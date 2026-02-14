#!/usr/bin/env python3
"""Parse TUTO_D1 events using known description strings as anchors to determine
exact event boundaries, then derive correct param counts."""

import struct
from pathlib import Path

def u32(data, off):
    return struct.unpack_from('<I', data, off)[0]

def hexdump(data, offset, length, prefix=""):
    for i in range(0, length, 16):
        off = offset + i
        n = min(16, length - i, len(data) - off)
        if n <= 0: break
        hx = ' '.join(f'{data[off+j]:02X}' for j in range(n))
        asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.' for j in range(n))
        print(f'{prefix}0x{off:04X}: {hx:<48} {asc}')

# Known condition/action param counts from MISSION_SCRIPTING.md
COND_PARAMS = {
    0: 2, 1: 3, 2: 2, 3: 2, 4: 2, 5: 3, 6: 4, 7: 2, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 1, 13: 3, 14: 3, 15: 2, 17: 3, 18: 1, 19: 3,
    20: 2, 22: 2, 23: 2, 24: 2, 25: 2, 26: 2, 27: 0, 28: 1, 29: 1,
    30: 1, 31: 2, 32: 3, 33: 1, 34: 3, 35: 1, 36: 1, 37: 0, 38: 2,
    39: 2, 40: 1, 41: 1, 42: 0, 43: 1, 44: 2, 45: 4, 46: 4, 47: 1,
    48: 1, 49: 2, 50: 1, 51: 1, 52: 2, 53: 1, 54: 3, 55: 2, 56: 2,
    57: 1, 58: 1, 59: 2, 60: 1,
}

# String actions (these have a variable-length string param)
STRING_ACTIONS = {38, 110, 165, 172}  # ACT_CHAR_SAY, ACT_PLAY_BGM, ACT_LOAD_MISSION, ACT_PLAY_FMV

ACT_PARAMS = {
    0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 2, 7: 3, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 3, 13: 3, 14: 1, 15: 2, 16: 2, 17: 2, 18: 2,
    19: 2, 20: 1, 21: 1, 22: 0, 23: 2, 24: 0, 26: 3, 27: 3, 28: 2,
    29: 0, 32: 2, 34: 1, 35: 1, 38: -1, 39: 1, 47: 0, 49: 0, 50: 0,
    51: 1, 52: 0, 53: 0, 54: 1, 55: 2, 56: 2, 57: 0, 58: 0, 59: 0,
    60: 2, 61: 1, 62: 2, 63: 1, 64: 1, 65: 1, 66: 2, 67: 1, 68: 1,
    70: 4, 71: 1, 72: 0, 73: 1, 74: 0, 75: 1, 76: 3, 77: 4, 78: 1,
    79: 0, 80: 1, 81: 0, 82: 2, 83: 1, 84: 1, 85: 0, 86: 2, 87: 3,
    88: 0, 89: 3, 90: 1, 91: 1, 92: 1, 93: 2, 94: 1, 95: 3, 96: 1,
    97: 1, 98: 1, 99: 3, 100: 2, 101: 1, 102: 1, 103: 0, 104: 0,
    105: 2, 106: 2, 107: 0, 109: 1, 110: -1, 111: 1, 112: 1, 113: 1,
    114: 1, 115: 2, 116: 2, 117: 2, 118: 1, 119: 0, 120: 0, 121: 1,
    122: 1, 123: 1, 124: 1, 125: 1, 126: 1, 127: 1, 129: 2, 130: 1,
    131: 3, 132: 0, 133: 2, 135: 2, 136: 2, 138: 1, 139: 1, 140: 1,
    141: 2, 142: 3, 143: 3, 144: 2, 145: 1, 146: 1, 147: 1, 148: 0,
    149: 0, 152: 4, 153: 3, 154: 3, 155: 0, 157: 0, 158: 0, 159: 0,
    161: 2, 162: 1, 163: 0, 164: 0, 165: -1, 166: 1, 168: 2, 169: 1,
    170: 1, 171: 2, 172: -1, 173: 1, 175: 2, 176: 2, 177: 2, 178: 2,
    179: 0, 181: 0, 182: 0,
}

ACT_NAMES = {
    0: "ACT_TROOP_STOP", 1: "ACT_TROOP_MOVE_TO_AREA", 2: "ACT_TROOP_ATTACK_TROOP",
    3: "ACT_TROOP_MOVE_TO_AREA_ATTACK", 4: "ACT_TROOP_CHARGE_TROOP",
    5: "ACT_TROOP_GUARD_AREA", 6: "ACT_TROOP_PATROL",
    7: "ACT_TROOP_PATROL_AREA", 8: "ACT_TROOP_RETREAT",
    9: "ACT_TROOP_TAKE_POSITION", 10: "ACT_TROOP_CHANGE_TROOP_TYPE",
    11: "ACT_TROOP_SET_BEHAVIOR", 12: "ACT_TROOP_SET_SPEED",
    13: "ACT_TROOP_SET_SIGHT_AND_ATK_RANGE", 14: "ACT_TROOP_SET_AGGRESSIVE",
    15: "ACT_TROOP_ENABLE_ABILITY", 17: "ACT_TROOP_DISABLE_ABILITY",
    18: "ACT_TROOP_CHANGE_AFFILIATION", 22: "ACT_MARK_TIME",
    23: "ACT_SET_BLOCK", 38: "ACT_CHAR_SAY", 39: "ACT_CLEAR_CHAR_SAY",
    47: "ACT_STOP_LETTERBOX", 49: "ACT_STOP_ALL_CINEMATICS",
    51: "ACT_TROOP_REVIVE_LEADER", 55: "ACT_VAR_INT_SET",
    56: "ACT_VAR_INT_ADD", 59: "ACT_TROOP_DISABLE",
    60: "ACT_TROOP_ENABLE_ON_AREA", 62: "ACT_TROOP_ENABLE_IN_AREA",
    85: "ACT_SET_CURSOR_POS", 86: "ACT_PLAY_CINEMATIC_AT_AREA",
    87: "ACT_PLAY_CINEMATIC_AND_WAIT", 89: "ACT_SEND_TROOP_TO",
    90: "ACT_LEADER_INVULNERABLE", 93: "ACT_TROOP_REFILL",
    95: "ACT_START_TROOP_AI", 97: "ACT_SHOW_AREA_ON_MINIMAP",
    105: "ACT_TROOP_INDICATE_IN_MINIMAP",
    110: "ACT_PLAY_BGM", 118: "ACT_DELAY_TICK",
    131: "ACT_PLAY_CINEMATIC_AND_WAIT2", 139: "ACT_TROOP_SET_LEADER_HP",
    145: "ACT_TROOP_SET_INVULNERABLE",
}

COND_NAMES = {
    0: "CON_TIME_ELAPSED", 1: "CON_TIME_ELAPSED_FROM_MARKED",
    2: "CON_TROOP_IN_AREA", 8: "CON_TROOP_ATTACKED",
    13: "CON_STATE_HP_PERCENT", 19: "CON_VAR_INT_COMPARE",
    27: "CON_START", 33: "CON_TROOP_NOT_ENGAGED",
    42: "CON_NEVER",
}


def find_description_anchors(data, start, end):
    """Find all positions where readable text suggests an event description."""
    anchors = []
    pos = start
    while pos < end - 64:
        # Check if the bytes at this position look like a description start
        b = data[pos]
        if 32 < b < 127:  # Starts with printable ASCII (not space)
            # Read until null
            null_pos = data.index(0, pos) if 0 in data[pos:pos+64] else pos + 64
            length = null_pos - pos
            if length >= 3:
                try:
                    s = data[pos:null_pos].decode('ascii')
                    if s.isascii() and any(c.isalpha() for c in s):
                        anchors.append((pos, s))
                except:
                    pass
        elif b >= 0x80:  # Korean text (CP949)
            null_pos = data.index(0, pos) if 0 in data[pos:pos+64] else pos + 64
            length = null_pos - pos
            if length >= 2:
                try:
                    s = data[pos:null_pos].decode('cp949', errors='replace')
                    anchors.append((pos, s))
                except:
                    pass
        pos += 4  # Events are 4-byte aligned

    return anchors


def main():
    path = Path.home() / "Downloads" / "KUF Crusaders" / "Mission" / "TUTO_D1.stg"
    data = path.read_bytes()
    file_size = len(data)
    ev_start = 0x1584
    footer = file_size - 42

    print(f"TUTO_D1.stg: events 0x{ev_start:X} to 0x{footer:X} ({footer-ev_start} bytes)")
    print(f"47 events expected\n")

    # Find description anchors
    anchors = find_description_anchors(data, ev_start, footer)
    print(f"Found {len(anchors)} description anchors:")
    for pos, desc in anchors:
        print(f"  0x{pos:X} (+{pos-ev_start}): '{desc}'")

    # Event 0 is confirmed: starts at 0x1584, description 'setup'
    # Let me manually trace event 0
    print("\n=== Manual trace of event 0 ===")
    pos = ev_start
    desc = data[pos:pos+64]
    null_idx = desc.index(0)
    desc_str = desc[:null_idx].decode('ascii')
    block_id = u32(data, pos + 64)
    num_cond = u32(data, pos + 68)
    print(f"  desc='{desc_str}' blockId={block_id} numCond={num_cond}")
    pos += 72  # Skip header

    # Parse conditions
    for c in range(num_cond):
        cid = u32(data, pos)
        pc = COND_PARAMS.get(cid, -1)
        name = COND_NAMES.get(cid, f"UNK_{cid}")
        params = [u32(data, pos + 4 + i*4) for i in range(min(pc, 4) if pc >= 0 else 0)]
        print(f"  cond[{c}] at 0x{pos:X}: {name}({cid}) params={params}")
        pos += 4 + pc * 4

    # numAct
    num_act = u32(data, pos)
    print(f"  numAct at 0x{pos:X} = {num_act}")
    pos += 4

    # Parse actions
    for a in range(num_act):
        aid = u32(data, pos)
        pc = ACT_PARAMS.get(aid, -1)
        name = ACT_NAMES.get(aid, f"ACT_{aid}")
        if pc == -1:
            # String action or unknown
            print(f"  act[{a}] at 0x{pos:X}: {name}({aid}) [STRING/UNKNOWN]")
            hexdump(data, pos, min(64, footer - pos), "    ")
            break
        params = [u32(data, pos + 4 + i*4) for i in range(pc)]
        print(f"  act[{a}] at 0x{pos:X}: {name}({aid}) params={params}")
        pos += 4 + pc * 4

    ev0_end = pos
    print(f"\n  Event 0 ends at 0x{ev0_end:X} (size={ev0_end - ev_start})")

    # Now dump what's between event 0 end and 'help_clear'
    help_clear_pos = 0x16E0  # Known from string search
    gap = help_clear_pos - ev0_end
    print(f"\n=== Gap between event 0 end (0x{ev0_end:X}) and 'help_clear' (0x{help_clear_pos:X}) = {gap} bytes ===")
    hexdump(data, ev0_end, gap, "  ")

    # Try parsing event 1 from ev0_end
    print(f"\n=== Parse event 1 from 0x{ev0_end:X} ===")
    pos = ev0_end
    desc = data[pos:pos+64]
    null_idx = desc.index(0) if 0 in desc else 64
    desc_str = desc[:null_idx].decode('cp949', errors='replace') if null_idx > 0 else ""
    block_id = u32(data, pos + 64)
    num_cond = u32(data, pos + 68)
    print(f"  desc='{desc_str}' blockId={block_id} numCond={num_cond}")

    if num_cond <= 10 and block_id <= 100:
        p = pos + 72
        print(f"  Conditions start at 0x{p:X}")

        for c in range(num_cond):
            cid = u32(data, p)
            name = COND_NAMES.get(cid, f"CON_{cid}")
            print(f"\n  cond[{c}] at 0x{p:X}: id={cid} ({name})")

            # Try different param counts for CON_TIME_ELAPSED
            if cid == 0:
                for nparams in [2, 3, 4]:
                    params = [u32(data, p + 4 + i*4) for i in range(nparams)]
                    next_pos = p + 4 + nparams * 4
                    next_val = u32(data, next_pos) if next_pos + 4 <= footer else -1
                    print(f"    If {nparams} params: {params} → next u32 at 0x{next_pos:X} = {next_val}")
                # Use doc value (2 params) for forward progress
                pc = COND_PARAMS.get(cid, 2)
            else:
                pc = COND_PARAMS.get(cid, 0)
                params = [u32(data, p + 4 + i*4) for i in range(pc)]
                print(f"    params({pc}): {params}")

            p += 4 + pc * 4

        # numAct
        numact_pos = p
        num_act = u32(data, p)
        print(f"\n  numAct at 0x{p:X} = {num_act}")
        p += 4

        if num_act <= 50:
            for a in range(num_act):
                aid = u32(data, p)
                pc = ACT_PARAMS.get(aid, -1)
                name = ACT_NAMES.get(aid, f"ACT_{aid}")
                if pc == -1 or pc < 0:
                    print(f"  act[{a}] at 0x{p:X}: {name}({aid}) [STRING/UNKNOWN]")
                    hexdump(data, p, min(64, footer - p), "    ")
                    break
                params = [u32(data, p + 4 + i*4) for i in range(pc)]
                print(f"  act[{a}] at 0x{p:X}: {name}({aid}) params={params}")
                p += 4 + pc * 4

            ev1_end = p
            print(f"\n  Event 1 parse ends at 0x{ev1_end:X}")
            print(f"  Expected next event ('help_clear') at 0x{help_clear_pos:X}")
            print(f"  Difference: {help_clear_pos - ev1_end} bytes")

    # CRUCIAL TEST: What if the param order is different?
    # Docs say CON_TIME_ELAPSED: "int seconds, compare"
    # But what if binary stores: "compare, int seconds"?
    print("\n=== CON_TIME_ELAPSED param interpretation ===")
    cond_off = ev0_end + 72  # After header of event 1
    raw = [u32(data, cond_off + i*4) for i in range(6)]
    print(f"  Raw u32s at cond start: {raw}")
    print(f"  id={raw[0]}")
    print(f"  If (seconds, compare): seconds={raw[1]}, compare={raw[2]}")
    print(f"  If (compare, seconds): compare={raw[1]}, seconds={raw[2]}")
    print(f"  If 3 params (seconds, ??, compare): {raw[1]}, {raw[2]}, {raw[3]}")
    print(f"  If 3 params (compare, seconds, ??): {raw[1]}, {raw[2]}, {raw[3]}")

    # ALSO: Investigate if there's a DIFFERENT structure — what if conditions have
    # an extra "enabled" flag or similar?
    print("\n=== What if each cond/act entry has an extra enabled/flag field? ===")
    print("  E.g., cond entry = id(4) + flag(4) + params[N*4]")
    print(f"  At cond start 0x{cond_off:X}: id={raw[0]}, flag={raw[1]}, then params starting at {raw[2:]}")

    # LARGE-SCALE: Parse help_clear event to see if it's self-consistent
    print(f"\n=== Parse 'help_clear' event at 0x{help_clear_pos:X} ===")
    pos = help_clear_pos
    desc = data[pos:pos+64]
    null_idx = desc.index(0) if 0 in desc else 64
    desc_str = desc[:null_idx].decode('ascii', errors='replace')
    block_id = u32(data, pos + 64)
    num_cond = u32(data, pos + 68)
    print(f"  desc='{desc_str}' blockId={block_id} numCond={num_cond}")

    if num_cond <= 20 and block_id <= 100:
        p = pos + 72
        for c in range(num_cond):
            cid = u32(data, p)
            name = COND_NAMES.get(cid, f"CON_{cid}")
            pc = COND_PARAMS.get(cid, 0)
            params = [u32(data, p + 4 + i*4) for i in range(min(pc, 6))]
            print(f"  cond[{c}] at 0x{p:X}: {name}({cid}) params={params}")
            p += 4 + pc * 4

        num_act = u32(data, p)
        print(f"  numAct at 0x{p:X} = {num_act}")
        p += 4

        if num_act <= 50:
            for a in range(num_act):
                if p + 4 > footer:
                    break
                aid = u32(data, p)
                pc = ACT_PARAMS.get(aid, -1)
                name = ACT_NAMES.get(aid, f"ACT_{aid}")
                if pc < 0:
                    print(f"  act[{a}] at 0x{p:X}: {name}({aid}) [STRING/UNKNOWN]")
                    hexdump(data, p, min(64, footer - p), "    ")
                    break
                params = [u32(data, p + 4 + i*4) for i in range(pc)]
                print(f"  act[{a}] at 0x{p:X}: {name}({aid}) params={params}")
                p += 4 + pc * 4

            ev_end = p
            print(f"  Event ends at 0x{ev_end:X}")

            # Check: what's the next anchor?
            next_anchors = [a for a in anchors if a[0] > help_clear_pos]
            if next_anchors:
                next_pos, next_desc = next_anchors[0]
                print(f"  Next anchor: '{next_desc}' at 0x{next_pos:X}")
                print(f"  Gap: {next_pos - ev_end} bytes")

    # Also: analyze the first few events of E1001 to cross-validate
    print("\n\n" + "=" * 70)
    print("=== E1001.stg cross-validation ===")
    print("=" * 70)

    path2 = Path.home() / "Downloads" / "KUF Crusaders" / "Mission" / "E1001.stg"
    data2 = path2.read_bytes()
    fs2 = len(data2)
    ev_start2 = 0x7394  # Known from format_finder
    footer2 = fs2 - 42

    # Find descriptions in E1001
    anchors2 = find_description_anchors(data2, ev_start2, footer2)
    print(f"E1001: {len(anchors2)} description anchors:")
    for pos, desc in anchors2[:20]:
        print(f"  0x{pos:X} (+{pos-ev_start2}): '{desc}'")

    # Parse event 0
    print(f"\n=== E1001 event 0 at 0x{ev_start2:X} ===")
    pos = ev_start2
    desc = data2[pos:pos+64]
    null_idx = desc.index(0) if 0 in desc else 64
    desc_str = desc[:null_idx].decode('cp949', errors='replace')
    block_id = u32(data2, pos + 64)
    num_cond = u32(data2, pos + 68)
    print(f"  desc='{desc_str}' blockId={block_id} numCond={num_cond}")

    p = pos + 72
    for c in range(num_cond):
        cid = u32(data2, p)
        name = COND_NAMES.get(cid, f"CON_{cid}")
        pc = COND_PARAMS.get(cid, 0)
        params = [u32(data2, p + 4 + i*4) for i in range(min(pc, 6))]
        print(f"  cond[{c}] at 0x{p:X}: {name}({cid}) params={params}")
        p += 4 + pc * 4

    num_act = u32(data2, p)
    print(f"  numAct at 0x{p:X} = {num_act}")
    p += 4

    for a in range(min(num_act, 20)):
        if p + 4 > footer2:
            break
        aid = u32(data2, p)
        pc = ACT_PARAMS.get(aid, -1)
        name = ACT_NAMES.get(aid, f"ACT_{aid}")
        if pc < 0:
            print(f"  act[{a}] at 0x{p:X}: {name}({aid}) [STRING/UNKNOWN]")
            hexdump(data2, p, min(64, footer2 - p), "    ")
            break
        params = [u32(data2, p + 4 + i*4) for i in range(pc)]
        print(f"  act[{a}] at 0x{p:X}: {name}({aid}) params={params}")
        p += 4 + pc * 4

    ev0_end2 = p
    print(f"  Event 0 ends at 0x{ev0_end2:X}")

    # Parse event 1
    print(f"\n=== E1001 event 1 at 0x{ev0_end2:X} ===")
    pos = ev0_end2
    desc = data2[pos:pos+64]
    null_idx = desc.index(0) if 0 in desc else 64
    desc_str = desc[:null_idx].decode('cp949', errors='replace')
    block_id = u32(data2, pos + 64)
    num_cond = u32(data2, pos + 68)
    print(f"  desc='{desc_str}' blockId={block_id} numCond={num_cond}")

    p = pos + 72
    for c in range(num_cond):
        cid = u32(data2, p)
        name = COND_NAMES.get(cid, f"CON_{cid}")
        print(f"\n  cond[{c}] at 0x{p:X}: id={cid} ({name})")

        if cid == 0:
            raw = [u32(data2, p + 4 + i*4) for i in range(5)]
            print(f"    Next 5 u32s: {raw}")
            for nparams in [2, 3]:
                params = raw[:nparams]
                next_val = raw[nparams] if nparams < 5 else -1
                print(f"    If {nparams} params: {params} → next={next_val}")
        else:
            pc = COND_PARAMS.get(cid, 0)
            params = [u32(data2, p + 4 + i*4) for i in range(pc)]
            print(f"    params({pc}): {params}")

        pc = COND_PARAMS.get(cid, 0)
        p += 4 + pc * 4

    num_act = u32(data2, p)
    print(f"\n  numAct at 0x{p:X} = {num_act}")

    # Next anchors
    if anchors2:
        next_anchor = [a for a in anchors2 if a[0] > ev_start2 + 72]
        if next_anchor:
            print(f"  Next description anchor: '{next_anchor[0][1]}' at 0x{next_anchor[0][0]:X}")
            print(f"  Distance from ev0_start: {next_anchor[0][0] - ev_start2}")

    # Hex dump between E1001 events 0 and 1
    print(f"\n=== E1001 hex dump from 0x{ev0_end2:X} to 0x{ev0_end2+200:X} ===")
    hexdump(data2, ev0_end2, 200, "  ")


if __name__ == '__main__':
    main()
