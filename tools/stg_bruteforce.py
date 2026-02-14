#!/usr/bin/env python3
"""Brute-force find event 2 start in E1140 by trying EVERY byte offset."""

import struct
from pathlib import Path

def u32(data, off):
    return struct.unpack_from('<I', data, off)[0]

def u16(data, off):
    return struct.unpack_from('<H', data, off)[0]

COND_PARAMS = {
    0: 2, 1: 3, 2: 2, 3: 2, 4: 2, 5: 3, 6: 4, 7: 2, 8: 1, 9: 1,
    10: 2, 11: 2, 12: 1, 13: 3, 14: 3, 15: 2, 17: 3, 18: 1, 19: 3,
    20: 2, 22: 2, 23: 2, 24: 2, 25: 2, 26: 2, 27: 0, 28: 1, 29: 1,
    30: 1, 31: 2, 32: 3, 33: 1, 34: 3, 35: 1, 36: 1, 37: 0, 38: 2,
    39: 2, 40: 1, 41: 1, 42: 0, 43: 1, 44: 2, 45: 4, 46: 4, 47: 1,
    48: 1, 49: 2, 50: 1, 51: 1, 52: 2, 53: 1, 54: 3, 55: 2, 56: 2,
    57: 1, 58: 1, 59: 2, 60: 1,
}

STRING_ACT_INT_PARAMS = {38: 3, 110: 2, 165: 0, 172: 1}

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


def try_parse_event(data, start, end_limit):
    """Try to parse an event. Returns (ok, end_offset)."""
    if start + 76 > end_limit:
        return False, 0

    desc_raw = data[start:start+64]
    if 0 not in desc_raw:
        return False, 0

    block_id = u32(data, start + 64)
    if block_id > 100:
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
        else:
            int_pc = STRING_ACT_INT_PARAMS.get(aid, 0)
            if pos + 4 + int_pc * 4 + 4 > end_limit:
                return False, 0
            str_len_off = pos + 4 + int_pc * 4
            str_len = u32(data, str_len_off)
            if str_len > 256 or str_len == 0:
                return False, 0
            str_off = str_len_off + 4
            if str_off + str_len > end_limit:
                return False, 0
            pos = str_off + str_len

    return True, pos


def hexdump(data, offset, length, prefix=""):
    for i in range(0, length, 16):
        off = offset + i
        n = min(16, length - i, len(data) - off)
        if n <= 0: break
        hx = ' '.join(f'{data[off+j]:02X}' for j in range(n))
        asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.' for j in range(n))
        print(f'{prefix}0x{off:04X}: {hx:<48} {asc}')


def main():
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    path = mission_dir / "E1140.stg"
    data = path.read_bytes()
    file_size = len(data)
    events_start = 0x644
    footer_start = file_size - 42

    print(f"E1140.stg: events 0x{events_start:X}-0x{footer_start:X} ({footer_start-events_start} bytes)")
    print(f"Brute-forcing event 2 start position...\n")

    # Try EVERY byte position for event 2
    exact_matches = []
    close_matches = []

    for ev2_start in range(events_start + 76, footer_start - 76):
        ok, ev2_end = try_parse_event(data, ev2_start, footer_start)
        if not ok:
            continue

        remaining = footer_start - ev2_end
        ev1_size = ev2_start - events_start
        ev2_size = ev2_end - ev2_start

        if remaining == 0:
            exact_matches.append((ev2_start, ev1_size, ev2_size))
        elif 0 < remaining <= 10:
            close_matches.append((ev2_start, ev1_size, ev2_size, remaining))

    print(f"EXACT matches (event 2 ends exactly at footer):")
    for ev2, s1, s2 in exact_matches:
        desc_raw = data[ev2:ev2+64]
        null_idx = desc_raw.index(0) if 0 in desc_raw else 64
        try:
            desc = desc_raw[:null_idx].decode('cp949', errors='replace')
        except:
            desc = desc_raw[:null_idx].decode('ascii', errors='replace')
        bid = u32(data, ev2 + 64)
        nc = u32(data, ev2 + 68)
        print(f"  ev2 at 0x{ev2:X}: ev1={s1}B ev2={s2}B desc='{desc}' blk={bid} ncond={nc}")

        # Parse event 2 details
        ok, end = try_parse_event(data, ev2, footer_start)
        if ok:
            pos = ev2 + 72
            for c in range(nc):
                cid = u32(data, pos)
                pc = COND_PARAMS.get(cid, 0)
                params = [u32(data, pos+4+i*4) for i in range(pc)]
                print(f"    cond[{c}]: id={cid} params={params}")
                pos += 4 + pc*4
            na = u32(data, pos)
            pos += 4
            for a in range(na):
                aid = u32(data, pos)
                pc = ACT_PARAMS.get(aid, 0)
                if pc >= 0:
                    params = [u32(data, pos+4+i*4) for i in range(pc)]
                    print(f"    act[{a}]: id={aid} params={params}")
                    pos += 4 + pc*4
                else:
                    ipc = STRING_ACT_INT_PARAMS.get(aid, 0)
                    iparams = [u32(data, pos+4+i*4) for i in range(ipc)]
                    slen = u32(data, pos+4+ipc*4)
                    sdata = data[pos+8+ipc*4:pos+8+ipc*4+slen].rstrip(b'\0').decode('ascii', errors='replace')
                    print(f"    act[{a}]: id={aid} iparams={iparams} str='{sdata}' slen={slen}")
                    pos = pos + 4 + ipc*4 + 4 + slen

    print(f"\nCLOSE matches (1-10 bytes remaining):")
    for ev2, s1, s2, rem in close_matches[:20]:
        desc_raw = data[ev2:ev2+64]
        null_idx = desc_raw.index(0) if 0 in desc_raw else 64
        try:
            desc = desc_raw[:null_idx].decode('cp949', errors='replace')
        except:
            desc = desc_raw[:null_idx].decode('ascii', errors='replace')
        bid = u32(data, ev2 + 64)
        nc = u32(data, ev2 + 68)
        print(f"  ev2 at 0x{ev2:X}: ev1={s1}B ev2={s2}B rem={rem} desc='{desc}' blk={bid} nc={nc}")

    # Also: try with DIFFERENT condition param counts for CON_TIME_ELAPSED
    print("\n=== Retry with CON_TIME_ELAPSED having 3 params ===")
    COND_PARAMS_ALT = dict(COND_PARAMS)
    COND_PARAMS_ALT[0] = 3  # Try 3 params instead of 2

    def try_parse_alt(data, start, end_limit):
        if start + 76 > end_limit:
            return False, 0
        desc_raw = data[start:start+64]
        if 0 not in desc_raw:
            return False, 0
        block_id = u32(data, start + 64)
        if block_id > 100:
            return False, 0
        num_cond = u32(data, start + 68)
        if num_cond > 30:
            return False, 0
        pos = start + 72
        for c in range(num_cond):
            if pos + 4 > end_limit:
                return False, 0
            cid = u32(data, pos)
            pc = COND_PARAMS_ALT.get(cid)
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
            else:
                int_pc = STRING_ACT_INT_PARAMS.get(aid, 0)
                if pos + 4 + int_pc * 4 + 4 > end_limit:
                    return False, 0
                str_len_off = pos + 4 + int_pc * 4
                str_len = u32(data, str_len_off)
                if str_len > 256 or str_len == 0:
                    return False, 0
                str_off = str_len_off + 4
                if str_off + str_len > end_limit:
                    return False, 0
                pos = str_off + str_len
        return True, pos

    exact_alt = []
    for ev2_start in range(events_start + 76, footer_start - 76):
        ok, ev2_end = try_parse_alt(data, ev2_start, footer_start)
        if ok and footer_start - ev2_end == 0:
            exact_alt.append(ev2_start)
            desc_raw = data[ev2_start:ev2_start+64]
            null_idx = desc_raw.index(0) if 0 in desc_raw else 64
            try:
                desc = desc_raw[:null_idx].decode('cp949', errors='replace')
            except:
                desc = desc_raw[:null_idx].decode('ascii', errors='replace')
            bid = u32(data, ev2_start + 64)
            nc = u32(data, ev2_start + 68)
            ev1_sz = ev2_start - events_start
            ev2_sz = ev2_end - ev2_start
            print(f"  EXACT at 0x{ev2_start:X}: ev1={ev1_sz}B ev2={ev2_sz}B "
                  f"desc='{desc}' blk={bid} ncond={nc}")


if __name__ == '__main__':
    main()
