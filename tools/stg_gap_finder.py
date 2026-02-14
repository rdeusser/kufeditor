#!/usr/bin/env python3
"""Find the correct gap/padding between events by brute-forcing event 1 start."""

import struct
from pathlib import Path

def u32(data, off):
    return struct.unpack_from('<I', data, off)[0]

# Use the same corrected param tables from stg_format_discovery.py
COND_PARAMS = {
    0: 2, 1: 3, 2: 2, 3: 2, 4: 2, 5: 3, 6: 4, 7: 2, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 1, 13: 3, 14: 3, 15: 2, 17: 3, 18: 1, 19: 3,
    20: 2, 22: 2, 23: 2, 24: 2, 25: 2, 26: 2, 27: 0, 28: 1, 29: 1,
    30: 1, 31: 2, 32: 3, 33: 1, 34: 3, 35: 1, 36: 1, 37: 0, 38: 2,
    39: 2, 40: 1, 41: 1, 42: 0, 43: 1, 44: 2, 45: 4, 46: 4, 47: 1,
    48: 1, 49: 2, 50: 1, 51: 1, 52: 2, 53: 1, 54: 3, 55: 2, 56: 2,
    57: 1, 58: 1, 59: 2, 60: 1,
    300: 1, 303: 0,
    402: 1, 403: 2, 404: 1, 405: 0, 406: 2, 407: 0, 408: 1, 409: 3,
    410: 1, 411: 1, 412: 0, 413: 0, 414: 0, 415: 2, 416: 2, 417: 1,
    418: 0, 419: 0, 420: 0, 421: 1,
}

STRING_ACT_INT_PARAMS = {33: 3, 110: 2, 165: 1, 180: 0, 729: 2}

ACT_PARAMS = {
    0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 2, 7: 3, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 3, 13: 3, 14: 1, 15: 2, 16: 2, 17: 2, 18: 2,
    19: 2, 20: 1, 21: 1, 22: 0, 23: 2, 24: 0, 26: 3, 27: 3, 28: 2,
    29: 0, 32: 2, 33: -1, 34: 1, 35: 1, 38: 1, 39: 1, 47: 0, 49: 0,
    50: 0, 51: 1, 52: 0, 53: 0, 54: 1, 55: 2, 56: 2, 57: 0, 58: 0,
    59: 0, 60: 2, 61: 1, 62: 2, 63: 1, 64: 1, 65: 1, 66: 2, 67: 1,
    68: 1, 70: 4, 71: 1, 72: 0, 73: 1, 74: 0, 75: 1, 76: 3, 77: 4,
    78: 1, 79: 0, 80: 1, 81: 0, 82: 2, 83: 1, 84: 1, 85: 0, 86: 2,
    87: 3, 88: 0, 89: 3, 90: 1, 91: 1, 92: 1, 93: 2, 94: 1, 95: 3,
    96: 1, 97: 1, 98: 1, 99: 3, 100: 2, 101: 1, 102: 1, 103: 0,
    104: 0, 105: 2, 106: 2, 107: 0, 109: 1, 110: -1, 111: 1, 112: 1,
    113: 1, 114: 1, 115: 2, 116: 2, 117: 2, 118: 1, 119: 0, 120: 0,
    121: 1, 122: 1, 123: 1, 124: 1, 125: 1, 126: 1, 127: 1, 129: 2,
    130: 1, 131: 3, 132: 0, 133: 2, 135: 2, 136: 2, 138: 1, 139: 1,
    140: 1, 141: 2, 142: 3, 143: 3, 144: 2, 145: 1, 146: 1, 147: 1,
    148: 0, 149: 0, 152: 4, 153: 3, 154: 3, 155: 0, 157: 0, 158: 0,
    159: 0, 161: 2, 162: 1, 163: 0, 164: 0, 165: -1, 166: 1, 168: 2,
    169: 1, 170: 1, 171: 2, 172: -2, 173: 1, 174: -2, 175: 2, 176: 2,
    177: 2, 178: 2, 179: 0, 180: -1, 181: 0, 182: 0,
    300: 1,
    500: 2, 501: 3, 502: 3, 503: 3, 504: 3, 505: 0, 506: 0,
    507: 1, 508: 1, 509: 1, 510: 0, 511: 4, 512: 1, 513: 3, 514: 4,
    700: 2, 701: 1, 702: 1, 703: 2, 704: 2, 706: 0, 707: 0, 708: 0,
    709: -2, 710: 0, 711: 2, 712: 2, 713: 0, 714: 0, 715: 1, 716: 1,
    717: 1, 718: 0, 719: 0, 720: 1, 721: 2, 723: 1, 724: 0,
    725: 2, 726: 3, 727: 1, 728: 3, 729: -1, 730: 1, 731: 2, 732: 3,
    733: 2, 734: 2, 735: 2, 736: 1, 737: 3, 738: 3, 739: 2,
}


def try_parse_chain(data, start, end_limit, max_events):
    """Parse a chain of events. Returns (count, end_offset)."""
    pos = start
    count = 0
    for _ in range(max_events):
        ok, next_pos = try_parse_one(data, pos, end_limit)
        if not ok:
            break
        count += 1
        pos = next_pos
    return count, pos


def try_parse_one(data, start, end_limit):
    """Try to parse one event. Returns (ok, end_offset)."""
    if start + 76 > end_limit:
        return False, 0

    desc_raw = data[start:start+64]
    if 0 not in desc_raw:
        return False, 0

    block_id = u32(data, start + 64)
    if block_id > 200:
        return False, 0

    num_cond = u32(data, start + 68)
    if num_cond > 30:
        return False, 0

    pos = start + 72

    for c in range(num_cond):
        if pos + 4 > end_limit:
            return False, 0
        cid = u32(data, pos)
        pc = COND_PARAMS.get(cid)
        if pc is None:
            return False, 0
        if pos + 4 + pc * 4 > end_limit:
            return False, 0
        pos += 4 + pc * 4

    if pos + 4 > end_limit:
        return False, 0
    num_act = u32(data, pos)
    if num_act > 50:
        return False, 0
    pos += 4

    for a in range(num_act):
        if pos + 4 > end_limit:
            return False, 0
        aid = u32(data, pos)
        pc = ACT_PARAMS.get(aid)
        if pc is None:
            return False, 0
        if pc >= 0:
            if pos + 4 + pc * 4 > end_limit:
                return False, 0
            pos += 4 + pc * 4
        elif pc == -1:
            int_pc = STRING_ACT_INT_PARAMS.get(aid, 0)
            if pos + 4 + int_pc * 4 + 4 > end_limit:
                return False, 0
            str_len = u32(data, pos + 4 + int_pc * 4)
            if str_len > 512 or str_len == 0:
                return False, 0
            if pos + 4 + int_pc * 4 + 4 + str_len > end_limit:
                return False, 0
            pos = pos + 4 + int_pc * 4 + 4 + str_len
        elif pc == -2:
            if pos + 8 > end_limit:
                return False, 0
            str1_len = u32(data, pos + 4)
            if str1_len > 512 or str1_len == 0:
                return False, 0
            if pos + 8 + str1_len + 4 > end_limit:
                return False, 0
            str2_len = u32(data, pos + 8 + str1_len)
            if str2_len > 512 or str2_len == 0:
                return False, 0
            if pos + 8 + str1_len + 4 + str2_len > end_limit:
                return False, 0
            pos = pos + 8 + str1_len + 4 + str2_len

    return True, pos


def locate_tail_sections(data, unit_count):
    """Fixed variable parsing with 76-byte entries."""
    tail_offset = 628 + unit_count * 544
    area_count = u32(data, tail_offset)
    after_areas = tail_offset + 4 + area_count * 84
    var_count = u32(data, after_areas)
    var_end = after_areas + 4 + var_count * 76
    # 8-byte gap [1, 0]
    ec_off = var_end + 8
    event_count = u32(data, ec_off)
    events_start = ec_off + 4
    footer_start = len(data) - 42
    return events_start, event_count, footer_start


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
        if not path.exists():
            continue

        data = path.read_bytes()
        unit_count = u32(data, 0x270)
        events_start, event_count, footer_start = locate_tail_sections(data, unit_count)
        event_data_size = footer_start - events_start

        print(f"\n{'='*60}")
        print(f"{name}: {event_count} events, {event_data_size}B event area")
        print(f"  events: 0x{events_start:X}-0x{footer_start:X}")

        # Parse event 0
        ok, ev0_end = try_parse_one(data, events_start, footer_start)
        if not ok:
            print(f"  Event 0 fails to parse!")
            continue

        ev0_size = ev0_end - events_start
        print(f"  Event 0: {ev0_size}B, ends at 0x{ev0_end:X}")

        # Try EVERY possible event 1 start (gap from 0 to 32 bytes)
        print(f"\n  Trying event 1 at different gaps:")
        best_chain = 0
        best_gap = -1
        best_remaining = 999999

        for gap in range(0, 33):
            ev1_start = ev0_end + gap
            if ev1_start + 76 > footer_start:
                break

            count, chain_end = try_parse_chain(data, ev1_start, footer_start, event_count - 1)
            remaining = footer_start - chain_end

            if count > 0:
                total_events = 1 + count  # event 0 + chain
                marker = ""
                if remaining == 0 and total_events == event_count:
                    marker = " *** PERFECT ***"
                elif remaining == 0:
                    marker = f" (all data consumed, {total_events} events)"
                elif count >= best_chain:
                    marker = ""

                if count >= 3 or remaining == 0 or total_events == event_count:
                    print(f"    gap={gap:2d}: ev1@0x{ev1_start:X}, chain={count} events, "
                          f"remaining={remaining}B{marker}")

                if (remaining == 0 and total_events == event_count) or count > best_chain:
                    best_chain = count
                    best_gap = gap
                    best_remaining = remaining

        if best_gap >= 0:
            print(f"\n  Best: gap={best_gap}, chain={best_chain}, remaining={best_remaining}")

            # Parse the best chain with details
            ev1_start = ev0_end + best_gap
            print(f"\n  Detailed parse with gap={best_gap}:")
            pos = events_start
            for i in range(event_count):
                ok, next_pos = try_parse_one(data, pos, footer_start)
                if not ok:
                    print(f"    Event {i} FAILS at 0x{pos:X}")
                    break

                size = next_pos - pos
                # Decode description
                desc_raw = data[pos:pos+64]
                null_idx = desc_raw.index(0) if 0 in desc_raw else 64
                try:
                    desc = desc_raw[:null_idx].decode('cp949', errors='replace')
                except:
                    desc = desc_raw[:null_idx].decode('ascii', errors='replace')

                block_id = u32(data, pos + 64)
                num_cond = u32(data, pos + 68)

                print(f"    Event {i} at 0x{pos:X}: size={size}B desc='{desc}' "
                      f"blk={block_id} nc={num_cond}")

                pos = next_pos
                if i == 0:
                    pos = ev1_start  # Apply gap after event 0

            remaining = footer_start - pos
            print(f"    Remaining: {remaining}B")

        # Also: try different CONDITION param counts for id=0 (CON_TIME_ELAPSED)
        # Try 0, 1, 3, 4, 5 params instead of standard 2
        print(f"\n  Trying different CON_TIME_ELAPSED param counts:")
        for cte_params in [0, 1, 3, 4, 5]:
            saved = COND_PARAMS[0]
            COND_PARAMS[0] = cte_params

            for gap in range(0, 17):
                ev1_start = ev0_end + gap
                if ev1_start + 76 > footer_start:
                    break
                count, chain_end = try_parse_chain(data, ev1_start, footer_start, event_count - 1)
                remaining = footer_start - chain_end
                total = 1 + count
                if remaining == 0 and total == event_count:
                    print(f"    CTE={cte_params} params, gap={gap}: PERFECT! "
                          f"{total} events, 0 remaining")
                elif count >= event_count // 2:
                    print(f"    CTE={cte_params} params, gap={gap}: chain={count}, "
                          f"remaining={remaining}")

            COND_PARAMS[0] = saved

        # Try different CON_VAR param counts (id=19)
        print(f"\n  Trying different CON_VAR param counts:")
        for cv_params in [2, 4, 5]:
            saved = COND_PARAMS[19]
            COND_PARAMS[19] = cv_params

            for gap in range(0, 17):
                ev1_start = ev0_end + gap
                if ev1_start + 76 > footer_start:
                    break
                count, chain_end = try_parse_chain(data, ev1_start, footer_start, event_count - 1)
                remaining = footer_start - chain_end
                total = 1 + count
                if remaining == 0 and total == event_count:
                    print(f"    CON_VAR={cv_params} params, gap={gap}: PERFECT! "
                          f"{total} events, 0 remaining")
                elif count >= event_count // 2:
                    print(f"    CON_VAR={cv_params} params, gap={gap}: chain={count}, "
                          f"remaining={remaining}")

            COND_PARAMS[19] = saved


if __name__ == '__main__':
    main()
