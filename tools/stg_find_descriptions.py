#!/usr/bin/env python3
"""Find all event descriptions in TUTO_D1.stg by scanning for valid 64-byte
description headers followed by reasonable blockId/numCond values."""

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
    path = Path.home() / "Downloads" / "KUF Crusaders" / "Mission" / "TUTO_D1.stg"
    data = path.read_bytes()
    file_size = len(data)

    # Event data range
    ev_start = 0x1584
    footer = file_size - 42

    print(f"TUTO_D1.stg: event data 0x{ev_start:X} to 0x{footer:X} ({footer-ev_start} bytes)")
    print(f"Expected: 47 events\n")

    # Search for event starts: positions where a 64-byte block followed by
    # reasonable blockId and numCond values.
    candidates = []
    for pos in range(ev_start, footer - 72, 4):
        # Read potential blockId and numCond
        block_id = u32(data, pos + 64)
        num_cond = u32(data, pos + 68)

        # Check constraints
        if block_id > 200:
            continue
        if num_cond > 20:
            continue

        # The 64-byte description must contain at least one null byte
        desc = data[pos:pos+64]
        if 0 not in desc:
            continue

        # Decode description
        null_idx = desc.index(0)
        try:
            desc_str = desc[:null_idx].decode('cp949', errors='replace')
        except:
            desc_str = desc[:null_idx].decode('ascii', errors='replace')

        # Check that everything between desc end and offset 64 is "reasonable"
        # (zeros or typical buffer garbage)

        candidates.append((pos, desc_str, block_id, num_cond))

    print(f"Found {len(candidates)} candidate event positions\n")

    # Show all candidates, highlighting ones that form a consistent sequence
    for i, (pos, desc, bid, ncond) in enumerate(candidates):
        rel = pos - ev_start
        if desc or bid < 50:  # Either has a description or reasonable blockId
            print(f"  0x{pos:X} (+{rel}): desc='{desc}' blockId={bid} numCond={ncond}")

    # Also extract all readable strings from the event area
    print(f"\n=== Readable strings in event area ===")
    strings_found = []
    i = ev_start
    while i < footer:
        if 32 <= data[i] < 127 or (data[i] >= 0x80 and data[i] <= 0xFE):
            start = i
            # Read until non-printable or null
            while i < footer and (32 <= data[i] < 127 or data[i] >= 0x80):
                i += 1
            if i - start >= 3:
                raw = data[start:i]
                try:
                    s = raw.decode('cp949', errors='replace')
                except:
                    s = raw.decode('ascii', errors='replace')
                strings_found.append((start, s))
        else:
            i += 1

    for off, s in strings_found[:50]:
        if len(s) >= 3 and any(c.isalpha() for c in s):
            print(f"  0x{off:X}: '{s}'")

    # Try: dump the first 200 bytes after event 0 ends (at 0x1648)
    print(f"\n=== Hex dump after event 0 (0x1648-0x1700) ===")
    hexdump(data, 0x1648, 0x100, "  ")

    # Try: look at what happens with numCond=1 for event 1
    # cond[0] at 0x1690: id=0 (CON_TIME_ELAPSED)
    # Let's see the raw bytes there
    print(f"\n=== Raw bytes at cond[0] of event 1 (0x1690) ===")
    hexdump(data, 0x1690, 48, "  ")

    # Try: what if CON_TIME_ELAPSED has 3 params?
    # cond[0]: id(4) + 3 params(12) = 16 bytes -> ends at 0x16A0
    # numAct at 0x16A0
    print(f"\n=== If CON_TIME_ELAPSED has 3 params ===")
    p3_numact = u32(data, 0x16A0)
    print(f"  numAct at 0x16A0 = {p3_numact}")
    if p3_numact < 50:
        p = 0x16A4
        for a in range(p3_numact):
            aid = u32(data, p)
            print(f"  act[{a}] at 0x{p:X}: id={aid}")
            p += 4

    # Try: what if numCond at event 1 is wrong?
    # What if blockId=21 is actually numCond, and there's an extra field?
    print(f"\n=== Alternative: extra padding between desc and blockId? ===")
    for extra in [0, 4, 8, 12]:
        bid = u32(data, 0x1648 + 64 + extra)
        nc = u32(data, 0x1648 + 68 + extra)
        na_off = 0x1648 + 72 + extra
        print(f"  extra={extra}: blockId={bid} numCond={nc} (header+extra ends at 0x{na_off:X})")


if __name__ == '__main__':
    main()
