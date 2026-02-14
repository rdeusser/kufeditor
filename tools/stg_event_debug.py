#!/usr/bin/env python3
"""Detailed debug of STG event parsing for TUTO_D1.stg."""

import struct
from pathlib import Path

def read_u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

def hexdump(data, offset, length, prefix="    "):
    for i in range(0, length, 16):
        off = offset + i
        nbytes = min(16, length - i, len(data) - off)
        if nbytes <= 0: break
        hexb = ' '.join(f'{data[off+j]:02X}' for j in range(nbytes))
        asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.' for j in range(nbytes))
        print(f'{prefix}0x{off:04X}: {hexb:<48} {asc}')

# Import param counts from v2 script (copy key ones)
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
    152: 4, 153: 3, 154: 3, 155: 0, 157: 0, 158: 0, 159: 0, 161: 2,
    162: 1, 163: 0, 164: 0, 166: 1, 168: 2, 169: 1, 170: 1, 171: 2,
    173: 1, 175: 2, 176: 2, 177: 2, 178: 2, 179: 0, 181: 0, 182: 0,
}

COND_NAMES = {
    0: "CON_TIME_ELAPSED", 2: "CON_TROOP_IN_AREA", 6: "CON_TROOP_CLOSE",
    8: "CON_TROOP_ATTACKED", 13: "CON_STATE_HP", 14: "CON_LEADER_HP",
    19: "CON_VAR", 23: "CON_TROOP_NOT_IN_AREA", 27: "CON_ALWAYS_TRUE",
}

ACT_NAMES = {
    0: "ACT_TRIGGER_ACTIVATE", 1: "ACT_TRIGGER_DEACTIVATE",
    2: "ACT_MARK_ON_TIME", 8: "ACT_TROOP_ENABLE", 9: "ACT_TROOP_DISABLE",
    10: "ACT_TROOP_WALK_TO", 11: "ACT_TROOP_RUN_TO", 14: "ACT_TROOP_STOP",
    55: "ACT_VAR_INT_SET", 58: "ACT_LETTER_BOX_ENABLE",
    59: "ACT_LETTER_BOX_DISABLE", 66: "ACT_SET_AI", 67: "ACT_ENABLE_AI",
    68: "ACT_DISABLE_AI", 90: "ACT_LEADER_INVULNERABLE",
    95: "ACT_TROOP_WARP", 110: "ACT_PLAY_BGM", 138: "ACT_ENABLE_ABILITY",
    139: "ACT_DISABLE_ABILITY", 145: "ACT_TROOP_SET_INVULN",
}


def parse_events_verbose(data, events_offset, event_count, file_size):
    """Parse with full trace of every field."""
    offset = events_offset

    for ev in range(min(event_count, 10)):
        print(f"\n{'='*60}")
        print(f"EVENT {ev} at offset 0x{offset:X}")
        print(f"{'='*60}")

        if offset + 72 > file_size:
            print("  OUT OF BOUNDS")
            break

        # Description
        desc_bytes = data[offset:offset+64]
        desc_end = desc_bytes.index(0) if 0 in desc_bytes else 64
        desc = desc_bytes[:desc_end].decode('ascii', errors='replace')
        print(f"  desc (64 bytes): '{desc}'")

        blockId = read_u32(data, offset + 64)
        numCond = read_u32(data, offset + 68)
        print(f"  blockId: {blockId}")
        print(f"  numCond: {numCond}")

        if numCond > 50:
            print(f"  INVALID numCond!")
            print(f"  Hex context:")
            hexdump(data, offset + 64, min(32, file_size - offset - 64))
            break

        pos = offset + 72
        for c in range(numCond):
            cid = read_u32(data, pos)
            pc = COND_PARAMS.get(cid)
            name = COND_NAMES.get(cid, f"CON_{cid}")
            if pc is None:
                print(f"  cond[{c}] at 0x{pos:X}: id={cid} ({name}) — UNKNOWN CONDITION!")
                hexdump(data, pos, min(32, file_size - pos))
                return
            params = [read_u32(data, pos + 4 + p*4) for p in range(pc)]
            entry_size = 4 + pc * 4
            print(f"  cond[{c}] at 0x{pos:X}: {name}(id={cid}) params={params} [{entry_size}B]")
            pos += entry_size

        numAct = read_u32(data, pos)
        print(f"  numAct at 0x{pos:X}: {numAct}")
        pos += 4

        if numAct > 100:
            print(f"  INVALID numAct!")
            hexdump(data, pos - 8, min(48, file_size - pos + 8))
            break

        for a in range(numAct):
            aid = read_u32(data, pos)
            pc = ACT_PARAMS.get(aid)
            name = ACT_NAMES.get(aid, f"ACT_{aid}")
            if pc is None:
                print(f"  act[{a}] at 0x{pos:X}: id={aid} ({name}) — UNKNOWN ACTION!")
                hexdump(data, pos, min(48, file_size - pos))
                # Try to continue by guessing 0 params
                print(f"  Trying to skip with 0 params...")
                pos += 4
                continue
            params = [read_u32(data, pos + 4 + p*4) for p in range(pc)]
            entry_size = 4 + pc * 4
            print(f"  act[{a}] at 0x{pos:X}: {name}(id={aid}) params={params} [{entry_size}B]")
            pos += entry_size

        event_size = pos - offset
        print(f"\n  Total event size: {event_size} bytes")
        print(f"  Next event at: 0x{pos:X}")

        # Show what's at the next event offset
        if pos < file_size:
            print(f"  Peek at next 80 bytes:")
            hexdump(data, pos, min(80, file_size - pos))

        offset = pos


def main():
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    for name in ["TUTO_D1.stg", "E1001.stg"]:
        path = mission_dir / name
        if not path.exists():
            print(f"NOT FOUND: {path}")
            continue

        data = path.read_bytes()
        print(f"\n{'#'*70}")
        print(f"# {name} ({len(data)} bytes)")
        print(f"{'#'*70}")

        # Parse tail
        header_size = 628
        unit_size = 544
        unit_count = read_u32(data, 0x270)
        tail = header_size + unit_count * unit_size

        area_count = read_u32(data, tail)
        area_section = 4 + area_count * 84

        var_offset = tail + area_section
        var_count = read_u32(data, var_offset)
        var_section = 4 + var_count * 76

        after_vars = var_offset + var_section
        m1 = read_u32(data, after_vars)
        m2 = read_u32(data, after_vars + 4)
        event_count = read_u32(data, after_vars + 8)
        events_offset = after_vars + 12

        print(f"  units={unit_count} areas={area_count} vars={var_count}")
        print(f"  mystery=({m1},{m2}) events={event_count} at 0x{events_offset:X}")

        parse_events_verbose(data, events_offset, event_count, len(data))


if __name__ == '__main__':
    main()
