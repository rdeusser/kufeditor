#!/usr/bin/env python3
"""Brute-force find event 2 start position in E1140.stg."""

import struct
from pathlib import Path

def u32(data, off):
    return struct.unpack_from('<I', data, off)[0]

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
    29: 0, 32: 2, 34: 1, 35: 1, 38: 1, 39: 1, 47: 0, 49: 0, 50: 0,
    51: 1, 52: 0, 53: 0, 54: 1, 55: 2, 56: 2, 57: 0, 58: 0, 59: 0,
    60: 2, 61: 1, 62: 2, 63: 1, 64: 1, 65: 1, 66: 2, 67: 1, 68: 1,
    70: 4, 71: 1, 72: 0, 73: 1, 74: 0, 75: 1, 76: 3, 77: 4, 78: 1,
    79: 0, 80: 1, 81: 0, 82: 2, 83: 1, 84: 1, 85: 0, 86: 2, 87: 3,
    88: 0, 89: 3, 90: 1, 91: 1, 92: 1, 93: 2, 94: 1, 95: 3, 96: 1,
    97: 1, 98: 1, 99: 3, 100: 2, 101: 1, 102: 1, 103: 0, 104: 0,
    105: 2, 106: 2, 107: 0, 109: 1, 111: 1, 112: 1, 113: 1, 114: 1,
    115: 2, 116: 2, 117: 2, 118: 1, 119: 0, 120: 0, 121: 1, 122: 1,
    123: 1, 124: 1, 125: 1, 126: 1, 127: 1, 129: 2, 130: 1, 131: 3,
    132: 0, 133: 2, 135: 2, 136: 2,
    138: 1, 139: 1, 140: 1, 141: 2,
    142: 3, 143: 3, 144: 2, 145: 1, 146: 1, 147: 1, 148: 0, 149: 0,
    152: 4, 153: 3, 154: 3, 155: 0, 157: 0, 158: 0, 159: 0,
    161: 2, 162: 1, 163: 0, 164: 0, 166: 1,
    168: 2, 169: 1, 170: 1, 171: 2,
    173: 1, 175: 2, 176: 2, 177: 2, 178: 2, 179: 0,
    181: 0, 182: 0,
    300: 1,
    500: 2, 501: 3, 502: 3, 503: 3, 504: 3, 505: 0, 506: 0, 507: 1,
    508: 1, 509: 1, 510: 0, 511: 4, 512: 1, 513: 3, 514: 4,
    700: 2, 701: 1, 702: 1, 703: 2, 704: 2, 706: 0, 707: 0, 708: 0,
    710: 0, 711: 2, 712: 2, 713: 0, 714: 0, 715: 1, 716: 1,
    717: 1, 718: 0, 719: 0, 720: 1, 721: 2, 723: 1, 724: 0, 725: 2,
    726: 3, 727: 1, 728: 3, 730: 1, 731: 2, 732: 3,
    733: 2, 734: 2, 735: 2, 736: 1, 737: 3, 738: 3, 739: 2,
}


def try_parse_event(data, start, file_end):
    """Try to parse an event at the given position. Returns (success, end_offset, info)."""
    if start + 72 > file_end:
        return False, 0, "too short for header"

    desc_raw = data[start:start+64]
    if 0 not in desc_raw:
        return False, 0, "no null in description"

    block_id = u32(data, start + 64)
    if block_id > 100:
        return False, 0, f"blockId={block_id} too large"

    num_cond = u32(data, start + 68)
    if num_cond > 50:
        return False, 0, f"numCond={num_cond} too large"

    pos = start + 72

    # Parse conditions
    for c in range(num_cond):
        if pos + 4 > file_end:
            return False, 0, f"cond[{c}] OOB"
        cid = u32(data, pos)
        pc = COND_PARAMS.get(cid)
        if pc is None:
            return False, 0, f"cond[{c}] unknown id={cid}"
        entry_size = 4 + pc * 4
        if pos + entry_size > file_end:
            return False, 0, f"cond[{c}] params OOB"
        pos += entry_size

    # Parse numAct
    if pos + 4 > file_end:
        return False, 0, "numAct OOB"
    num_act = u32(data, pos)
    if num_act > 100:
        return False, 0, f"numAct={num_act} too large"
    pos += 4

    # Parse actions
    for a in range(num_act):
        if pos + 4 > file_end:
            return False, 0, f"act[{a}] OOB"
        aid = u32(data, pos)
        pc = ACT_PARAMS.get(aid)
        if pc is None:
            return False, 0, f"act[{a}] unknown id={aid}"
        entry_size = 4 + pc * 4
        if pos + entry_size > file_end:
            return False, 0, f"act[{a}] params OOB"
        pos += entry_size

    desc_end = desc_raw.index(0) if 0 in desc_raw else 64
    try:
        desc = desc_raw[:desc_end].decode('cp949', errors='replace')
    except:
        desc = desc_raw[:desc_end].decode('ascii', errors='replace')

    info = f"desc='{desc}' blockId={block_id} numCond={num_cond} numAct={num_act}"
    return True, pos, info


def main():
    path = Path.home() / "Downloads" / "KUF Crusaders" / "Mission" / "E1140.stg"
    data = path.read_bytes()
    file_size = len(data)

    events_start = 0x644
    footer_start = file_size - 42

    print(f"E1140.stg: events at 0x{events_start:X}, footer at 0x{footer_start:X}")
    print(f"Event data: {footer_start - events_start} bytes for 2 events")
    print()

    # Event 1 starts at 0x644
    # numAct=1, action[0]=110 (ACT_PLAY_BGM, string action)
    # We know event 1 header uses 72 bytes + action
    # The action is at 0x690 (id=110)
    # We need to find where event 2 starts

    # Brute force: try every 4-byte-aligned position for event 2
    print("Scanning for valid event 2 starting positions...")
    print(f"(searching 0x{events_start + 72:X} to 0x{footer_start:X})")
    print()

    found = []
    for ev2_start in range(events_start + 72, footer_start, 4):
        ok, ev2_end, info = try_parse_event(data, ev2_start, footer_start)
        if ok:
            ev1_size = ev2_start - events_start
            ev2_size = ev2_end - ev2_start
            remaining_after = footer_start - ev2_end

            if remaining_after == 0:
                # Perfect match!
                found.append((ev2_start, ev1_size, ev2_size, remaining_after, info))
                print(f"  *** EXACT MATCH *** ev2 at 0x{ev2_start:X}")
                print(f"      event1: {ev1_size} bytes, event2: {ev2_size} bytes")
                print(f"      event2: {info}")
            elif remaining_after >= 0 and remaining_after < 20:
                found.append((ev2_start, ev1_size, ev2_size, remaining_after, info))
                print(f"  CLOSE at 0x{ev2_start:X} (remaining={remaining_after})")
                print(f"      event1: {ev1_size} bytes, event2: {ev2_size} bytes")
                print(f"      event2: {info}")

    if not found:
        print("  No valid positions found!")
        # Try unaligned too
        print("\n  Trying unaligned positions...")
        for ev2_start in range(events_start + 72, footer_start):
            ok, ev2_end, info = try_parse_event(data, ev2_start, footer_start)
            if ok and footer_start - ev2_end == 0:
                ev1_size = ev2_start - events_start
                ev2_size = ev2_end - ev2_start
                print(f"  UNALIGNED MATCH at 0x{ev2_start:X}")
                print(f"      event1: {ev1_size} bytes, event2: {ev2_size} bytes")
                print(f"      event2: {info}")


if __name__ == '__main__':
    main()
