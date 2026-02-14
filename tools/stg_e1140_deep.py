#!/usr/bin/env python3
"""Deep analysis of E1140.stg event area (2 events, 699 bytes).
Try ALL possible entry sizes and formats to find the correct parse."""

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

    event_data_size = footer_start - events_start
    print(f"E1140.stg: {file_size} bytes")
    print(f"Events: 0x{events_start:X} to 0x{footer_start:X} = {event_data_size} bytes for 2 events")
    print(f"Average event: {event_data_size / 2} bytes")
    print()

    # Dump ENTIRE event area
    print("=== Complete event area hex dump ===")
    hexdump(data, events_start, event_data_size, "  ")

    # Dump as u32 array
    print("\n=== Event area as u32 values ===")
    for i in range(0, event_data_size, 4):
        off = events_start + i
        val = u32(data, off)
        asc = ''
        for j in range(4):
            b = data[off + j]
            asc += chr(b) if 32 <= b < 127 else '.'
        print(f"  0x{off:X} [+{i:3d}]: {val:10d} (0x{val:08X})  {asc}")

    # Find descriptions in event area
    print("\n=== Finding description-like text ===")
    pos = events_start
    while pos < footer_start:
        if data[pos] >= 0x80 or (32 < data[pos] < 127):
            end = pos
            while end < footer_start and (data[end] >= 0x80 or (32 <= data[end] < 127)):
                end += 1
            if end - pos >= 2:
                try:
                    s = data[pos:end].decode('cp949', errors='replace')
                    print(f"  0x{pos:X} (+{pos-events_start}): '{s}' ({end-pos} bytes)")
                except:
                    pass
            pos = end
        else:
            pos += 1

    # Try ALL possible event 2 start positions and see which gives a
    # valid parse for BOTH events simultaneously
    print("\n=== Brute-force: try every possible event split ===")
    print("(Each split divides 699 bytes into event1_size + event2_size)")

    # Event 1 starts at events_start, has description '시작'
    # Event 1 description: bytes 0x644 to 0x644+63 = 64-byte desc
    # After desc: blockId, numCond, etc.

    for ev2_start in range(events_start + 76, footer_start - 76, 4):
        ev1_size = ev2_start - events_start
        ev2_size = footer_start - ev2_start

        # Parse event 1
        ev1_ok = False
        desc1 = data[events_start:events_start+64]
        if 0 in desc1:
            block_id1 = u32(data, events_start + 64)
            num_cond1 = u32(data, events_start + 68)

            if block_id1 <= 100 and num_cond1 <= 20:
                # Try to reach ev2_start by parsing conditions + numAct + actions
                ev1_remaining = ev1_size - 72  # After header

                # For this analysis, just check if values are reasonable
                ev1_ok = True

        # Parse event 2
        ev2_ok = False
        desc2 = data[ev2_start:ev2_start+64]
        if 0 in desc2:
            block_id2 = u32(data, ev2_start + 64)
            num_cond2 = u32(data, ev2_start + 68)

            if block_id2 <= 100 and num_cond2 <= 20:
                ev2_ok = True

        if ev1_ok and ev2_ok:
            # Decode descriptions
            n1 = desc1.index(0) if 0 in desc1 else 64
            n2 = desc2.index(0) if 0 in desc2 else 64
            try:
                d1 = desc1[:n1].decode('cp949', errors='replace')
            except:
                d1 = desc1[:n1].decode('ascii', errors='replace')
            try:
                d2 = desc2[:n2].decode('cp949', errors='replace')
            except:
                d2 = desc2[:n2].decode('ascii', errors='replace')

            # Print interesting splits
            if d1 and d2:
                print(f"\n  Split at 0x{ev2_start:X}: ev1={ev1_size}B ev2={ev2_size}B")
                print(f"    ev1: desc='{d1}' blockId={block_id1} numCond={num_cond1}")
                print(f"    ev2: desc='{d2}' blockId={block_id2} numCond={num_cond2}")
            elif not d1 and not d2:
                pass  # Both empty - skip
            else:
                label = "ev1_desc" if d1 else "ev2_desc"
                desc_text = d1 or d2
                if block_id2 <= 10 and num_cond2 <= 5:
                    print(f"\n  Split at 0x{ev2_start:X}: ev1={ev1_size}B ev2={ev2_size}B")
                    print(f"    ev1: desc='{d1}' blockId={block_id1} numCond={num_cond1}")
                    print(f"    ev2: desc='{d2}' blockId={block_id2} numCond={num_cond2}")

    # ALSO: Check if the event area might contain FIXED-SIZE events
    print("\n\n=== Fixed-size event hypothesis ===")
    # 699 / 2 = 349.5 — not evenly divisible
    # But if there's padding, maybe events are padded to some boundary
    for ev_size in range(76, 400, 4):
        if event_data_size == ev_size * 2:
            print(f"  EXACT: 2 × {ev_size} = {event_data_size}")
        elif abs(event_data_size - ev_size * 2) <= 8:
            print(f"  CLOSE: 2 × {ev_size} = {ev_size*2} (diff={event_data_size - ev_size*2})")

    # Check for specific popular sizes
    for ev_size in [256, 288, 320, 348, 352, 384, 448]:
        total = ev_size * 2
        diff = event_data_size - total
        print(f"  {ev_size} × 2 = {total} (diff={diff})")

    # WHAT IF: event description + fixed header = 80 bytes?
    # (64 desc + 4 blockId + 4 numCond + 4 flags + 4 ?)
    print("\n=== Alternative header sizes ===")
    for hdr_size in [72, 76, 80, 84, 88]:
        remaining = event_data_size - 2 * hdr_size
        if remaining > 0:
            print(f"  header={hdr_size}: remaining for conditions/actions = {remaining} bytes")


if __name__ == '__main__':
    main()
