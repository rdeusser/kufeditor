#!/usr/bin/env python3
"""Find event 2 start by looking for valid event headers and partial parses."""

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


def main():
    path = Path.home() / "Downloads" / "KUF Crusaders" / "Mission" / "E1140.stg"
    data = path.read_bytes()
    file_size = len(data)
    events_start = 0x644
    footer_start = file_size - 42

    print(f"E1140.stg: events at 0x{events_start:X}, footer at 0x{footer_start:X}")
    print(f"Event data: {footer_start - events_start} bytes for 2 events\n")

    # Approach 1: Find positions where (blockId, numCond, numAct) look reasonable
    print("=== Searching for event-header-like patterns ===")
    for pos in range(events_start + 72, footer_start - 72, 4):
        # At pos - 64 should be description start, pos is blockId
        desc_start = pos - 64
        if desc_start < events_start:
            continue

        block_id = u32(data, pos)
        if block_id > 20:
            continue

        num_cond = u32(data, pos + 4)
        if num_cond > 20:
            continue

        # Check that desc area has a null byte
        desc = data[desc_start:pos]
        if 0 not in desc:
            continue

        desc_end = desc.index(0)
        try:
            desc_str = desc[:desc_end].decode('cp949', errors='replace')
        except:
            desc_str = desc[:desc_end].decode('ascii', errors='replace')

        ev2_start = desc_start
        ev1_size = ev2_start - events_start
        print(f"  Candidate at 0x{ev2_start:X} (ev1_size={ev1_size}): "
              f"desc='{desc_str}' blockId={block_id} numCond={num_cond}")

    # Approach 2: Just look at specific candidate offsets and dump
    print("\n=== Detailed analysis of event 1 action area ===")
    print("Event 1: desc at 0x644 ('시작'), blockId=0, numCond=0, numAct=1")
    print("Action 0 (ACT_PLAY_BGM) starts at 0x690")
    print()
    print("Hex dump from action start:")
    hexdump(data, 0x690, 128, "  ")

    # Let's look at what MUST be event 2 by finding its blockId
    # E1140 has 2 events. Event 0 blockId=0, event 1 blockId is likely 1
    print("\n=== Looking for u32 value 1 (probable event 2 blockId) ===")
    for off in range(0x690, footer_start, 4):
        if u32(data, off) == 1:
            # Check if 64 bytes before this could be a description
            desc_start = off - 64
            if desc_start >= events_start:
                # Check if desc looks valid (has null within 64 bytes)
                desc = data[desc_start:off]
                if 0 in desc:
                    num_cond = u32(data, off + 4)
                    desc_end = desc.index(0)
                    try:
                        d = desc[:desc_end].decode('cp949', errors='replace')
                    except:
                        d = desc[:desc_end].decode('ascii', errors='replace')
                    if num_cond < 50:
                        print(f"  blockId=1 at 0x{off:X}, ev2_start=0x{desc_start:X}: "
                              f"desc='{d}' numCond={num_cond}")
                        ev1_size = desc_start - events_start
                        ev2_size = footer_start - desc_start
                        print(f"    ev1={ev1_size}B ev2={ev2_size}B")

    # Approach 3: Focus on ACT_PLAY_BGM format
    # The action is at 0x690: 6E 00 00 00 (id=110)
    # Let's see what follows by dumping the full event area
    print("\n=== Complete event area hex dump ===")
    hexdump(data, events_start, footer_start - events_start)

    # Approach 4: Check if E1001 has better luck (no string actions early)
    print("\n\n=== Trying E1001.stg (non-string actions likely) ===")
    path2 = Path.home() / "Downloads" / "KUF Crusaders" / "Mission" / "E1001.stg"
    data2 = path2.read_bytes()
    fs2 = len(data2)

    # Known: events at offset with ec=16 at 0x7390
    ec_off = 0x7390
    ec = u32(data2, ec_off)
    ev_start = ec_off + 4
    footer2 = fs2 - 42

    print(f"E1001: {fs2} bytes, {ec} events, events_start=0x{ev_start:X}, footer=0x{footer2:X}")
    print(f"Event data: {footer2 - ev_start} bytes\n")

    # Known condition/action param counts (excluding string actions)
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
        132: 0, 133: 2, 135: 2, 136: 2, 138: 1, 139: 1, 140: 1, 141: 2,
        142: 3, 143: 3, 144: 2, 145: 1, 146: 1, 147: 1, 148: 0, 149: 0,
        152: 4, 153: 3, 154: 3, 155: 0, 157: 0, 158: 0, 159: 0,
        161: 2, 162: 1, 163: 0, 164: 0, 166: 1,
        168: 2, 169: 1, 170: 1, 171: 2,
        173: 1, 175: 2, 176: 2, 177: 2, 178: 2, 179: 0, 181: 0, 182: 0,
    }

    pos = ev_start
    for ev in range(min(ec, 5)):
        if pos + 72 > footer2:
            print(f"  Event {ev}: OUT OF BOUNDS at 0x{pos:X}")
            break

        desc = data2[pos:pos+64]
        desc_end = desc.index(0) if 0 in desc else 64
        try:
            desc_str = desc[:desc_end].decode('cp949', errors='replace')
        except:
            desc_str = desc[:desc_end].decode('ascii', errors='replace')

        block_id = u32(data2, pos + 64)
        num_cond = u32(data2, pos + 68)

        p = pos + 72
        cond_ok = True
        for c in range(num_cond):
            if p + 4 > footer2:
                cond_ok = False
                break
            cid = u32(data2, p)
            pc = COND_PARAMS.get(cid)
            if pc is None:
                print(f"  Event {ev} cond[{c}]: UNKNOWN id={cid} at 0x{p:X}")
                cond_ok = False
                break
            p += 4 + pc * 4

        if not cond_ok:
            print(f"  Event {ev} at 0x{pos:X}: desc='{desc_str}' COND PARSE FAILED")
            break

        num_act = u32(data2, p)
        p += 4

        act_ok = True
        for a in range(num_act):
            if p + 4 > footer2:
                act_ok = False
                break
            aid = u32(data2, p)
            pc = ACT_PARAMS.get(aid)
            if pc is None:
                print(f"  Event {ev} act[{a}]: UNKNOWN id={aid} at 0x{p:X}")
                hexdump(data2, p, min(32, footer2 - p), "    ")
                act_ok = False
                break
            p += 4 + pc * 4

        ev_size = p - pos
        status = "OK" if (cond_ok and act_ok) else "FAILED"
        print(f"  Event {ev} at 0x{pos:X}: desc='{desc_str}' blockId={block_id} "
              f"cond={num_cond} act={num_act} size={ev_size} [{status}]")

        if not (cond_ok and act_ok):
            break
        pos = p

    print(f"\n  Parsed through offset 0x{pos:X} ({pos})")
    print(f"  Footer at 0x{footer2:X} ({footer2})")
    print(f"  Remaining: {footer2 - pos} bytes")


if __name__ == '__main__':
    main()
