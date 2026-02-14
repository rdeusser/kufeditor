#!/usr/bin/env python3
"""Find the exact variable entry size and event section layout by brute force."""

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


def find_event_count(data, search_start, file_size, expected_event_count=None):
    """Search for a plausible event count u32 in the data."""
    results = []
    for off in range(search_start, min(search_start + 200, file_size - 4), 4):
        val = u32(data, off)
        if 1 <= val <= 100:
            # Check if what follows looks like events (64-byte description starting with text or null)
            ev_start = off + 4
            if ev_start + 72 <= file_size:
                # The first 64 bytes should be a description (text or zeros)
                desc = data[ev_start:ev_start+64]
                # Check that it looks like a null-terminated string (first null within 64 bytes)
                has_null = 0 in desc
                # After desc (64B), next 4 bytes should be a small blockId
                block_id = u32(data, ev_start + 64)
                # Then numCond should be small
                num_cond = u32(data, ev_start + 68)

                is_plausible = (has_null and block_id < 100 and num_cond < 50)
                results.append((off, val, block_id, num_cond, is_plausible))

                if expected_event_count and val == expected_event_count and is_plausible:
                    return off, val

    return results


def analyze_file(path, expected_events=None):
    data = path.read_bytes()
    file_size = len(data)

    print(f"\n{'='*70}")
    print(f"# {path.name} ({file_size} bytes)")
    print(f"{'='*70}")

    unit_count = u32(data, 0x270)
    tail_offset = 628 + unit_count * 544

    # AreaIDs
    area_count = u32(data, tail_offset)
    area_section = 4 + area_count * 84
    after_areas = tail_offset + area_section

    # Variable count
    var_count = u32(data, after_areas)
    print(f"  units={unit_count}, areas={area_count}, vars={var_count}")
    print(f"  tail=0x{tail_offset:X}, after_areas=0x{after_areas:X}")

    # Try different variable entry sizes
    for var_size in [68, 72, 76, 80, 84]:
        after_vars = after_areas + 4 + var_count * var_size
        if after_vars > file_size:
            continue

        # Check what comes after vars
        remaining = file_size - after_vars
        if remaining < 8:
            continue

        # Look for event count in the next few bytes
        for gap in [0, 4, 8, 12]:
            ec_off = after_vars + gap
            if ec_off + 4 > file_size:
                continue
            ec = u32(data, ec_off)

            # Check if ec is reasonable
            if ec == 0 or ec > 200:
                continue

            # Check events_start
            ev_start = ec_off + 4
            if ev_start + 72 > file_size:
                continue

            # Check event 1 structure
            desc = data[ev_start:ev_start+64]
            has_null = 0 in desc
            block_id = u32(data, ev_start + 64)
            num_cond = u32(data, ev_start + 68)

            if has_null and block_id < 100 and num_cond < 50:
                # Check if this variable name is readable
                var_start = after_areas + 4
                var_name = data[var_start:var_start+64]
                null_pos = var_name.index(0) if 0 in var_name else 64
                try:
                    name = var_name[:null_pos].decode('cp949', errors='replace')
                except:
                    name = var_name[:null_pos].decode('ascii', errors='replace')

                # For this var_size, check the last bytes of the entry
                entry_end = var_start + var_size
                if entry_end <= file_size:
                    last12 = data[entry_end-12:entry_end]
                    last_vals = [u32(last12, i) for i in range(0, 12, 4)]
                else:
                    last_vals = []

                # Check gap values
                gap_vals = [u32(data, after_vars + i) for i in range(0, gap, 4)] if gap > 0 else []

                # Score this combination
                desc_str = data[ev_start:ev_start+64]
                null_idx = desc_str.index(0) if 0 in desc_str else 64
                try:
                    ev_desc = desc_str[:null_idx].decode('cp949', errors='replace')
                except:
                    ev_desc = desc_str[:null_idx].decode('ascii', errors='replace')

                print(f"\n  var_size={var_size} gap={gap}: ec={ec} at 0x{ec_off:X}")
                print(f"    var[0] name='{name}', last12={last_vals}")
                print(f"    gap_vals={gap_vals}")
                print(f"    event[0]: desc='{ev_desc}' blockId={block_id} numCond={num_cond}")

                # Validate: try to parse first event
                if expected_events and ec == expected_events:
                    print(f"    *** MATCHES EXPECTED EVENT COUNT ***")


def main():
    mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

    files = [
        ("E1140.stg", 2),
        ("TUTO_D1.stg", 47),
        ("E1001.stg", 16),
        ("X3001.stg", 46),
    ]

    for name, expected_events in files:
        path = mission_dir / name
        if path.exists():
            analyze_file(path, expected_events)


if __name__ == '__main__':
    main()
