#!/usr/bin/env python3
"""Comprehensive STG event format discovery tool.

Empirically determines the binary event format by:
1. Finding description anchors (readable text) in event area
2. Computing actual event sizes from consecutive anchors
3. Trying variable-length parsing with corrected param tables
4. Reporting which parse hypotheses work across all files
"""

import struct
from pathlib import Path

def u32(data, off):
    return struct.unpack_from('<I', data, off)[0]

def u16(data, off):
    return struct.unpack_from('<H', data, off)[0]


# Complete condition param counts from MISSION_SCRIPTING.md (CruMission.h)
COND_PARAMS = {
    0: 2, 1: 3, 2: 2, 3: 2, 4: 2, 5: 3, 6: 4, 7: 2, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 1, 13: 3, 14: 3, 15: 2, 17: 3, 18: 1, 19: 3,
    20: 2, 22: 2, 23: 2, 24: 2, 25: 2, 26: 2, 27: 0, 28: 1, 29: 1,
    30: 1, 31: 2, 32: 3, 33: 1, 34: 3, 35: 1, 36: 1, 37: 0, 38: 2,
    39: 2, 40: 1, 41: 1, 42: 0, 43: 1, 44: 2, 45: 4, 46: 4, 47: 1,
    48: 1, 49: 2, 50: 1, 51: 1, 52: 2, 53: 1, 54: 3, 55: 2, 56: 2,
    57: 1, 58: 1, 59: 2, 60: 1,
    # Worldmap conditions
    300: 1, 303: 0,
    402: 1, 403: 2, 404: 1, 405: 0, 406: 2, 407: 0, 408: 1, 409: 3,
    410: 1, 411: 1, 412: 0, 413: 0, 414: 0, 415: 2, 416: 2, 417: 1,
    418: 0, 419: 0, 420: 0, 421: 1,
}

# Complete action param counts from MISSION_SCRIPTING.md
# -1 = string action (uses STRING_ACT_INT_PARAMS for int param count)
# -2 = double-string action (two consecutive strings)
ACT_PARAMS = {
    0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 2, 7: 3, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 3, 13: 3, 14: 1, 15: 2, 16: 2, 17: 2, 18: 2,
    19: 2, 20: 1, 21: 1, 22: 0, 23: 2, 24: 0, 26: 3, 27: 3, 28: 2,
    29: 0, 32: 2, 33: -1, 34: 1, 35: 1,
    38: 1,  # ACT_OPEN_SESAME: PropID (NOT string action!)
    39: 1, 47: 0, 49: 0, 50: 0,
    51: 1, 52: 0, 53: 0, 54: 1, 55: 2, 56: 2, 57: 0, 58: 0, 59: 0,
    60: 2, 61: 1, 62: 2, 63: 1, 64: 1, 65: 1, 66: 2, 67: 1, 68: 1,
    70: 4, 71: 1, 72: 0, 73: 1, 74: 0, 75: 1, 76: 3, 77: 4, 78: 1,
    79: 0, 80: 1, 81: 0, 82: 2, 83: 1, 84: 1, 85: 0, 86: 2, 87: 3,
    88: 0, 89: 3, 90: 1, 91: 1, 92: 1, 93: 2, 94: 1, 95: 3, 96: 1,
    97: 1, 98: 1, 99: 3, 100: 2, 101: 1, 102: 1, 103: 0, 104: 0,
    105: 2, 106: 2, 107: 0, 109: 1,
    110: -1,  # ACT_PLAY_BGM: string, int, int
    111: 1, 112: 1, 113: 1,
    114: 1, 115: 2, 116: 2, 117: 2, 118: 1, 119: 0, 120: 0, 121: 1,
    122: 1, 123: 1, 124: 1, 125: 1, 126: 1, 127: 1, 129: 2, 130: 1,
    131: 3, 132: 0, 133: 2, 135: 2, 136: 2, 138: 1, 139: 1, 140: 1,
    141: 2, 142: 3, 143: 3, 144: 2, 145: 1, 146: 1, 147: 1, 148: 0,
    149: 0, 152: 4, 153: 3, 154: 3, 155: 0, 157: 0, 158: 0, 159: 0,
    161: 2, 162: 1, 163: 0, 164: 0,
    165: -1,  # ACT_LOAD_MISSION: string, int
    166: 1,
    168: 2, 169: 1, 170: 1, 171: 2,
    172: -2,  # ACT_PLAY_FMV: string, string (DOUBLE STRING)
    173: 1,
    174: -2,  # ACT_CHANGE_SKYBOX_N_LIGHT_SET: string, string
    175: 2, 176: 2, 177: 2, 178: 2,
    179: 0,
    180: -1,  # ACT_PLAY_FMV_AND_GO_TO_WORLDMAP: string
    181: 0, 182: 0,
    # Briefing actions (500s)
    500: 2, 501: 3, 502: 3, 503: 3, 504: 3, 505: 0, 506: 0,
    507: 1, 508: 1, 509: 1, 510: 0, 511: 4, 512: 1, 513: 3, 514: 4,
    # Live action
    300: 1,
    # Worldmap actions (700s)
    700: 2, 701: 1, 702: 1, 703: 2, 704: 2, 706: 0, 707: 0, 708: 0,
    709: -2,  # ACT_WORLD_CHANGE_LIGHT_N_SKY: string, string
    710: 0, 711: 2, 712: 2, 713: 0, 714: 0, 715: 1, 716: 1,
    717: 1, 718: 0, 719: 0, 720: 1, 721: 2, 723: 1, 724: 0,
    725: 2, 726: 3, 727: 1, 728: 3,
    729: -1,  # ACT_WORLD_PLAY_BGM: string, int, int
    730: 1, 731: 2, 732: 3, 733: 2, 734: 2, 735: 2, 736: 1,
    737: 3, 738: 3, 739: 2,
}

# String actions: number of int params BEFORE the string
STRING_ACT_INT_PARAMS = {
    33: 3,   # ACT_VAR_DISPLAY: VariableID, int, int, string
    110: 2,  # ACT_PLAY_BGM: string, int, int → in binary: 2 ints then string
    165: 1,  # ACT_LOAD_MISSION: string, int → in binary: 1 int then string
    180: 0,  # ACT_PLAY_FMV_AND_GO_TO_WORLDMAP: string
    729: 2,  # ACT_WORLD_PLAY_BGM: string, int, int
}

COND_NAMES = {
    0: "CON_TIME_ELAPSED", 1: "CON_TIME_ELAPSED_FROM_MARKED",
    2: "CON_TROOP_IN_AREA", 3: "CON_TROOP_SCOUTER_STOPPED_IN_AREA",
    4: "CON_TROOP_SCOUTER_IN_AREA", 5: "CON_TROOP_SCOUTER_CLOSE_TO_TROOP",
    6: "CON_TROOP_CLOSE_TO_TROOP", 7: "CON_TROOP_TARGETED",
    8: "CON_TROOP_ATTACKED", 9: "CON_TROOP_MELEE_ATTACKED",
    12: "CON_LEADER_HAS_BEEN_KILLED", 13: "CON_STATE_HP_PERCENT",
    19: "CON_VAR", 27: "CON_ALWAYS_TRUE",
}

ACT_NAMES = {
    0: "ACT_TRIGGER_ACTIVATE", 1: "ACT_TRIGGER_DEACTIVATE",
    6: "ACT_CHAR_SAY", 8: "ACT_TROOP_ENABLE", 9: "ACT_TROOP_DISABLE",
    14: "ACT_TROOP_STOP", 38: "ACT_OPEN_SESAME",
    49: "ACT_MISSION_COMPLETE", 50: "ACT_MISSION_FAIL",
    55: "ACT_VAR_INT_SET", 90: "ACT_LEADER_INVULNERABLE",
    95: "ACT_TROOP_WARP", 110: "ACT_PLAY_BGM",
    139: "ACT_DISABLE_ABILITY", 145: "ACT_TROOP_SET_INVULNERABLE",
    165: "ACT_LOAD_MISSION", 172: "ACT_PLAY_FMV",
}


def try_parse_event(data, start, end_limit, verbose=False):
    """Try to parse an event at start. Returns (ok, end_offset, info_str)."""
    if start + 76 > end_limit:
        return False, 0, "too short for header"

    desc_raw = data[start:start+64]
    if 0 not in desc_raw:
        return False, 0, "no null in desc"

    null_idx = desc_raw.index(0)
    try:
        desc = desc_raw[:null_idx].decode('cp949', errors='replace')
    except:
        desc = desc_raw[:null_idx].decode('ascii', errors='replace')

    block_id = u32(data, start + 64)
    if block_id > 200:
        return False, 0, f"blockId={block_id} too high"

    num_cond = u32(data, start + 68)
    if num_cond > 30:
        return False, 0, f"numCond={num_cond} too high"

    pos = start + 72

    conds = []
    for c in range(num_cond):
        if pos + 4 > end_limit:
            return False, 0, f"cond[{c}] id overflows"
        cid = u32(data, pos)
        pc = COND_PARAMS.get(cid)
        if pc is None:
            return False, 0, f"cond[{c}] unknown id={cid}"
        if pos + 4 + pc * 4 > end_limit:
            return False, 0, f"cond[{c}] params overflow (id={cid}, pc={pc})"
        params = [u32(data, pos + 4 + i*4) for i in range(pc)]
        conds.append((cid, params))
        pos += 4 + pc * 4

    if pos + 4 > end_limit:
        return False, 0, "numAct overflows"
    num_act = u32(data, pos)
    if num_act > 50:
        return False, 0, f"numAct={num_act} too high"
    pos += 4

    acts = []
    for a in range(num_act):
        if pos + 4 > end_limit:
            return False, 0, f"act[{a}] id overflows"
        aid = u32(data, pos)
        pc = ACT_PARAMS.get(aid)
        if pc is None:
            return False, 0, f"act[{a}] unknown id={aid}"

        if pc >= 0:
            if pos + 4 + pc * 4 > end_limit:
                return False, 0, f"act[{a}] params overflow (id={aid}, pc={pc})"
            params = [u32(data, pos + 4 + i*4) for i in range(pc)]
            acts.append((aid, params, None))
            pos += 4 + pc * 4
        elif pc == -1:
            # Single string action
            int_pc = STRING_ACT_INT_PARAMS.get(aid, 0)
            if pos + 4 + int_pc * 4 + 4 > end_limit:
                return False, 0, f"act[{a}] string header overflow (id={aid})"
            int_params = [u32(data, pos + 4 + i*4) for i in range(int_pc)]
            str_len = u32(data, pos + 4 + int_pc * 4)
            if str_len > 512 or str_len == 0:
                return False, 0, f"act[{a}] bad strlen={str_len} (id={aid})"
            str_off = pos + 4 + int_pc * 4 + 4
            if str_off + str_len > end_limit:
                return False, 0, f"act[{a}] string data overflow"
            s = data[str_off:str_off+str_len].rstrip(b'\0').decode('ascii', errors='replace')
            acts.append((aid, int_params, s))
            pos = str_off + str_len
        elif pc == -2:
            # Double string action
            if pos + 8 > end_limit:
                return False, 0, f"act[{a}] double string header overflow (id={aid})"
            str1_len = u32(data, pos + 4)
            if str1_len > 512 or str1_len == 0:
                return False, 0, f"act[{a}] bad str1len={str1_len}"
            str1_off = pos + 8
            if str1_off + str1_len + 4 > end_limit:
                return False, 0, f"act[{a}] string1 overflow"
            s1 = data[str1_off:str1_off+str1_len].rstrip(b'\0').decode('ascii', errors='replace')
            str2_len = u32(data, str1_off + str1_len)
            if str2_len > 512 or str2_len == 0:
                return False, 0, f"act[{a}] bad str2len={str2_len}"
            str2_off = str1_off + str1_len + 4
            if str2_off + str2_len > end_limit:
                return False, 0, f"act[{a}] string2 overflow"
            s2 = data[str2_off:str2_off+str2_len].rstrip(b'\0').decode('ascii', errors='replace')
            acts.append((aid, [], f"{s1}|{s2}"))
            pos = str2_off + str2_len

    size = pos - start
    info = f"desc='{desc}' blk={block_id} c={num_cond} a={num_act} size={size}"
    if verbose:
        for cid, params in conds:
            name = COND_NAMES.get(cid, f"CON_{cid}")
            info += f"\n    cond: {name}({cid}) {params}"
        for aid, params, s in acts:
            name = ACT_NAMES.get(aid, f"ACT_{aid}")
            if s:
                info += f"\n    act: {name}({aid}) {params} str='{s}'"
            else:
                info += f"\n    act: {name}({aid}) {params}"

    return True, pos, info


def find_description_anchors(data, start, end):
    """Find positions that look like event description starts."""
    anchors = []
    pos = start
    while pos + 64 <= end:
        # Check if bytes at pos look like a description field:
        # Either starts with printable ASCII/Korean text,
        # or is all-null (empty description)
        first_byte = data[pos]
        is_text = (32 < first_byte < 127) or first_byte >= 0x80
        is_null = first_byte == 0

        if is_text or is_null:
            # Verify there's a null terminator within 64 bytes
            desc_raw = data[pos:pos+64]
            if 0 in desc_raw:
                # Check if blockId (at +64) is reasonable
                if pos + 68 <= end:
                    block_id = u32(data, pos + 64)
                    num_cond_or_act = u32(data, pos + 68)
                    if block_id <= 200 and num_cond_or_act <= 50:
                        null_idx = desc_raw.index(0)
                        try:
                            desc = desc_raw[:null_idx].decode('cp949', errors='replace')
                        except:
                            desc = desc_raw[:null_idx].decode('ascii', errors='replace')
                        anchors.append((pos, desc, block_id, num_cond_or_act))
        pos += 1

    return anchors


def find_events_from_start(data, events_start, footer_start, max_events):
    """Try to parse consecutive events from events_start.
    Returns list of (offset, size, info_str) for successfully parsed events."""
    events = []
    pos = events_start
    for i in range(max_events):
        ok, next_pos, info = try_parse_event(data, pos, footer_start, verbose=True)
        if not ok:
            return events, pos, info
        size = next_pos - pos
        events.append((pos, size, info))
        pos = next_pos
    remaining = footer_start - pos
    return events, pos, f"remaining={remaining}"


def locate_tail_sections(data, unit_count):
    """Locate the tail sections (areas, variables, gap, event count, events start).
    Variables are 76 bytes FIXED (64-byte name + 4 varId + 4 padding + 4 initialValue).
    """
    tail_offset = 628 + unit_count * 544

    # AreaIDs: 4-byte count + count * 84 bytes
    area_count = u32(data, tail_offset)
    after_areas = tail_offset + 4 + area_count * 84

    # Variables: 4-byte count + count * 76 bytes (FIXED SIZE)
    var_count = u32(data, after_areas)
    var_section_size = 4 + var_count * 76
    var_end = after_areas + var_section_size

    # Try multiple gap/event-count detection strategies
    # Strategy 1: 8-byte gap [1, 0] then event count
    # Strategy 2: 8-byte gap [0, 0] then event count
    # Strategy 3: No gap, just event count
    footer_start = len(data) - 42

    best = None
    for gap_size in [8, 4, 0]:
        ec_off = var_end + gap_size
        if ec_off + 4 > len(data):
            continue
        event_count = u32(data, ec_off)
        events_start = ec_off + 4

        # Sanity check: event count should be reasonable
        if event_count > 500:
            continue
        # Events must fit between events_start and footer
        event_data_size = footer_start - events_start
        if event_data_size < 0:
            continue
        # Average event size should be reasonable (76-2000 bytes)
        if event_count > 0:
            avg = event_data_size / event_count
            if avg < 76 or avg > 2000:
                continue

        gap_bytes = [u32(data, var_end + i) for i in range(0, gap_size, 4)] if gap_size else []
        candidate = {
            'tail_offset': tail_offset,
            'area_count': area_count,
            'after_areas': after_areas,
            'var_count': var_count,
            'var_end': var_end,
            'gap_size': gap_size,
            'gap_values': gap_bytes,
            'ec_offset': ec_off,
            'event_count': event_count,
            'events_start': events_start,
            'footer_start': footer_start,
        }
        if best is None:
            best = candidate
        # Prefer the one with gap [1, 0]
        if gap_bytes == [1, 0]:
            best = candidate
            break

    return best


def hexdump(data, offset, length, prefix=""):
    for i in range(0, length, 16):
        off = offset + i
        n = min(16, length - i, len(data) - off)
        if n <= 0: break
        hx = ' '.join(f'{data[off+j]:02X}' for j in range(n))
        asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.' for j in range(n))
        print(f'{prefix}0x{off:04X}: {hx:<48} {asc}')


def analyze_file(name, path):
    data = path.read_bytes()
    file_size = len(data)
    unit_count = u32(data, 0x270)

    info = locate_tail_sections(data, unit_count)

    print(f"\n{'='*70}")
    print(f"{name}: {file_size} bytes, {unit_count} units")
    print(f"  Areas: {info['area_count']}, Variables: {info['var_count']}")
    print(f"  Gap: size={info['gap_size']}, values={info['gap_values']}")
    print(f"  Events: count={info['event_count']}, start=0x{info['events_start']:X}")
    print(f"  Footer: 0x{info['footer_start']:X}")
    event_data_size = info['footer_start'] - info['events_start']
    print(f"  Event data: {event_data_size} bytes")
    print(f"{'='*70}")

    events_start = info['events_start']
    footer_start = info['footer_start']
    event_count = info['event_count']

    # Phase 1: Try parsing consecutively
    print(f"\n--- Phase 1: Sequential parse ---")
    events, end_pos, fail_info = find_events_from_start(
        data, events_start, footer_start, event_count)

    for i, (off, size, inf) in enumerate(events):
        print(f"  Event {i} at 0x{off:X}: {inf}")

    remaining = footer_start - end_pos
    if len(events) == event_count and remaining == 0:
        print(f"  *** PERFECT PARSE: {event_count} events, 0 remaining ***")
        return True
    else:
        print(f"  Parsed {len(events)}/{event_count}, stopped at 0x{end_pos:X}")
        print(f"  Failure: {fail_info}")
        print(f"  Remaining: {remaining} bytes")

    # Phase 2: Find description anchors
    print(f"\n--- Phase 2: Description anchors ---")

    # Only look at anchors that have non-empty descriptions (more reliable)
    all_anchors = find_description_anchors(data, events_start, footer_start)
    text_anchors = [a for a in all_anchors if a[1]]  # non-empty desc

    print(f"  Total candidate anchors: {len(all_anchors)}")
    print(f"  Text anchors (non-empty desc): {len(text_anchors)}")

    if text_anchors:
        print(f"  First 30 text anchors:")
        for off, desc, blk, nc in text_anchors[:30]:
            rel = off - events_start
            print(f"    0x{off:X} (+{rel}): desc='{desc}' blk={blk} field={nc}")

    # Phase 3: Try parsing from each text anchor to find working events
    print(f"\n--- Phase 3: Parse from each text anchor ---")
    good_anchors = []
    for off, desc, blk, nc in text_anchors:
        ok, end, info = try_parse_event(data, off, footer_start)
        if ok:
            size = end - off
            good_anchors.append((off, size, desc, blk))
            if len(good_anchors) <= 30:
                print(f"  ✓ 0x{off:X}: {info}")

    print(f"  Successfully parsed: {len(good_anchors)}/{len(text_anchors)} text anchors")

    # Phase 4: For the FIRST failure, dump the data around it
    if len(events) < event_count:
        fail_off = end_pos
        print(f"\n--- Phase 4: Analysis at failure point 0x{fail_off:X} ---")
        dump_len = min(256, footer_start - fail_off)
        hexdump(data, fail_off, dump_len, "  ")

        # Show as u32 values
        print(f"\n  As u32 values:")
        for i in range(0, min(80, dump_len), 4):
            off = fail_off + i
            val = u32(data, off)
            asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.'
                          for j in range(4))
            rel = off - events_start
            print(f"    0x{off:X} (+{rel:4d}): {val:10d} (0x{val:08X})  {asc}")

        # Phase 5: Try to find event boundary by scanning forward
        print(f"\n--- Phase 5: Find next valid event after failure ---")
        for probe in range(fail_off + 4, min(fail_off + 400, footer_start - 72), 4):
            ok, end, info = try_parse_event(data, probe, footer_start)
            if ok:
                size = end - probe
                dist = probe - fail_off
                # Also try parsing the NEXT event from end
                ok2, end2, info2 = try_parse_event(data, end, footer_start)
                chain = "→✓" if ok2 else "→✗"
                # Try 3-chain
                if ok2:
                    ok3, end3, info3 = try_parse_event(data, end2, footer_start)
                    chain += "→✓" if ok3 else "→✗"
                print(f"  probe 0x{probe:X} (+{dist}): {info} {chain}")

    return False


def main():
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    files = [
        "E1140.stg",
        "TUTO_D1.stg",
        "E1001.stg",
        "X3001.stg",
    ]

    for name in files:
        path = mission_dir / name
        if path.exists():
            analyze_file(name, path)
        else:
            print(f"NOT FOUND: {path}")


if __name__ == '__main__':
    main()
