#!/usr/bin/env python3
"""Determine string action format by trying ALL possible string sizes
for ACT_PLAY_BGM in E1140 and validating against event 2 parse."""

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


# Condition param counts
COND_PARAMS = {
    0: 2, 1: 3, 2: 2, 3: 2, 4: 2, 5: 3, 6: 4, 7: 2, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 1, 13: 3, 14: 3, 15: 2, 17: 3, 18: 1, 19: 3,
    20: 2, 22: 2, 23: 2, 24: 2, 25: 2, 26: 2, 27: 0, 28: 1, 29: 1,
    30: 1, 31: 2, 32: 3, 33: 1, 34: 3, 35: 1, 36: 1, 37: 0, 38: 2,
    39: 2, 40: 1, 41: 1, 42: 0, 43: 1, 44: 2, 45: 4, 46: 4, 47: 1,
    48: 1, 49: 2, 50: 1, 51: 1, 52: 2, 53: 1, 54: 3, 55: 2, 56: 2,
    57: 1, 58: 1, 59: 2, 60: 1,
}

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

# String actions and their int param counts (the string is the remaining param)
# From MISSION_SCRIPTING.md:
# ACT_CHAR_SAY (38): TroopID, string, int, int → 3 ints + string
# ACT_PLAY_BGM (110): string, int, int → 2 ints + string
# ACT_LOAD_MISSION (165): string → 0 ints + string
# ACT_PLAY_FMV (172): string, int → 1 int + string
STRING_ACT_INT_PARAMS = {38: 3, 110: 2, 165: 0, 172: 1}


def try_parse_event(data, start, end_limit):
    """Try to parse an event at start. Returns (ok, end_offset, info_dict)."""
    if start + 76 > end_limit:
        return False, 0, {}

    desc_raw = data[start:start+64]
    if 0 not in desc_raw:
        return False, 0, {}

    block_id = u32(data, start + 64)
    if block_id > 100:
        return False, 0, {}

    num_cond = u32(data, start + 68)
    if num_cond > 30:
        return False, 0, {}

    pos = start + 72

    # Parse conditions
    conds = []
    for c in range(num_cond):
        if pos + 4 > end_limit:
            return False, 0, {}
        cid = u32(data, pos)
        pc = COND_PARAMS.get(cid)
        if pc is None:
            return False, 0, {}
        if pos + 4 + pc * 4 > end_limit:
            return False, 0, {}
        params = [u32(data, pos + 4 + i*4) for i in range(pc)]
        conds.append((cid, params))
        pos += 4 + pc * 4

    # numAct
    if pos + 4 > end_limit:
        return False, 0, {}
    num_act = u32(data, pos)
    if num_act > 50:
        return False, 0, {}
    pos += 4

    # Parse actions
    acts = []
    for a in range(num_act):
        if pos + 4 > end_limit:
            return False, 0, {}
        aid = u32(data, pos)
        pc = ACT_PARAMS.get(aid)
        if pc is None:
            return False, 0, {}

        if pc >= 0:
            # Regular action
            if pos + 4 + pc * 4 > end_limit:
                return False, 0, {}
            params = [u32(data, pos + 4 + i*4) for i in range(pc)]
            acts.append((aid, params, None))
            pos += 4 + pc * 4
        else:
            # String action
            int_pc = STRING_ACT_INT_PARAMS.get(aid, 0)
            if pos + 4 + int_pc * 4 + 4 > end_limit:
                return False, 0, {}

            int_params = [u32(data, pos + 4 + i*4) for i in range(int_pc)]
            str_len_off = pos + 4 + int_pc * 4
            str_len = u32(data, str_len_off)

            if str_len > 256 or str_len == 0:
                return False, 0, {}

            str_off = str_len_off + 4
            if str_off + str_len > end_limit:
                return False, 0, {}

            try:
                s = data[str_off:str_off+str_len].rstrip(b'\0').decode('ascii', errors='replace')
            except:
                s = '<binary>'

            acts.append((aid, int_params, s))
            pos = str_off + str_len

    null_idx = desc_raw.index(0)
    try:
        desc = desc_raw[:null_idx].decode('cp949', errors='replace')
    except:
        desc = desc_raw[:null_idx].decode('ascii', errors='replace')

    info = {
        'desc': desc, 'blockId': block_id, 'numCond': num_cond,
        'numAct': num_act, 'conds': conds, 'acts': acts
    }
    return True, pos, info


def main():
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    # E1140: 2 events
    path = mission_dir / "E1140.stg"
    data = path.read_bytes()
    file_size = len(data)
    events_start = 0x644
    footer_start = file_size - 42

    print(f"E1140.stg: {file_size} bytes, events at 0x{events_start:X}-0x{footer_start:X}")
    print(f"Event data: {footer_start - events_start} bytes for 2 events")
    print()

    # Try parsing event 0 (with string action detection)
    ok, ev0_end, info = try_parse_event(data, events_start, footer_start)
    if ok:
        print(f"Event 0: desc='{info['desc']}' blockId={info['blockId']} "
              f"numCond={info['numCond']} numAct={info['numAct']}")
        for cid, params in info['conds']:
            print(f"  cond: id={cid} params={params}")
        for aid, params, s in info['acts']:
            if s:
                print(f"  act: id={aid} params={params} string='{s}'")
            else:
                print(f"  act: id={aid} params={params}")
        print(f"  Event 0 ends at 0x{ev0_end:X} (size={ev0_end - events_start})")
        print()

        # Now try parsing event 1 from ev0_end
        ok2, ev1_end, info2 = try_parse_event(data, ev0_end, footer_start)
        if ok2:
            print(f"Event 1: desc='{info2['desc']}' blockId={info2['blockId']}' "
                  f"numCond={info2['numCond']} numAct={info2['numAct']}")
            for cid, params in info2['conds']:
                print(f"  cond: id={cid} params={params}")
            for aid, params, s in info2['acts']:
                if s:
                    print(f"  act: id={aid} params={params} string='{s}'")
                else:
                    print(f"  act: id={aid} params={params}")
            print(f"  Event 1 ends at 0x{ev1_end:X} (size={ev1_end - ev0_end})")
            print(f"  Footer starts at 0x{footer_start:X}")
            print(f"  Remaining: {footer_start - ev1_end} bytes")

            if ev1_end == footer_start:
                print("  *** PERFECT MATCH ***")
        else:
            print(f"Event 1 parse FAILED from 0x{ev0_end:X}")
            print("Hex around event 1 start:")
            hexdump(data, ev0_end, min(128, footer_start - ev0_end), "  ")
    else:
        print("Event 0 parse FAILED!")

    # Now try ALL 4 files
    print("\n\n" + "=" * 70)
    print("=== Full parse test across all files ===")
    print("=" * 70)

    files = [
        ("E1140.stg", 2, 0x644),
        ("TUTO_D1.stg", 47, 0x1584),
        ("E1001.stg", 16, 0x7394),
        ("X3001.stg", 46, 0x6E5C),
    ]

    for name, expected_ec, ev_start in files:
        path = mission_dir / name
        data = path.read_bytes()
        file_size = len(data)
        footer = file_size - 42

        print(f"\n--- {name}: {expected_ec} events, events at 0x{ev_start:X} ---")

        pos = ev_start
        parsed = 0
        for i in range(expected_ec):
            ok, next_pos, info = try_parse_event(data, pos, footer)
            if not ok:
                print(f"  Event {i} FAILED at 0x{pos:X}")
                print(f"  Hex at failure point:")
                hexdump(data, pos, min(80, footer - pos), "    ")
                break

            ev_size = next_pos - pos
            act_strs = []
            for aid, params, s in info['acts']:
                if s:
                    act_strs.append(f"{aid}('{s}')")
                else:
                    act_strs.append(f"{aid}")
            acts_brief = ','.join(act_strs) if act_strs else 'none'

            print(f"  Event {i} at 0x{pos:X}: desc='{info['desc']}' blk={info['blockId']} "
                  f"c={info['numCond']} a={info['numAct']} size={ev_size} acts=[{acts_brief}]")
            pos = next_pos
            parsed += 1

        remaining = footer - pos
        print(f"  Parsed {parsed}/{expected_ec} events. Remaining: {remaining} bytes")
        if remaining == 0 and parsed == expected_ec:
            print(f"  *** PERFECT PARSE ***")


if __name__ == '__main__':
    main()
