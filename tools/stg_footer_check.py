#!/usr/bin/env python3
"""Verify footer size and event boundaries across multiple STG files."""

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
        asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.' for j in range(n)
        )
        print(f'{prefix}0x{off:04X}: {hx:<48} {asc}')


def analyze_footer(path_str, expected_events):
    path = Path.home() / "Downloads" / "KUF Crusaders" / "Mission" / path_str
    data = path.read_bytes()
    file_size = len(data)

    print(f"\n{'='*60}")
    print(f"{path_str}: {file_size} bytes, {expected_events} events")
    print(f"{'='*60}")

    # Dump last 60 bytes
    print("\nLast 60 bytes:")
    hexdump(data, file_size - 60, 60, "  ")

    # Check footer at different sizes
    for footer_size in [38, 39, 40, 41, 42, 43, 44, 45, 46]:
        footer_start = file_size - footer_size
        # Read potential winFlag
        if footer_start + 4 <= file_size:
            win_flag = u32(data, footer_start)
            # Read potential map names
            win_map = data[footer_start+4:footer_start+4+19]
            lose_map = data[footer_start+4+19:footer_start+4+38]

            win_map_str = ''
            lose_map_str = ''
            try:
                null_idx = win_map.index(0) if 0 in win_map else len(win_map)
                win_map_str = win_map[:null_idx].decode('ascii', errors='replace')
            except:
                pass
            try:
                null_idx = lose_map.index(0) if 0 in lose_map else len(lose_map)
                lose_map_str = lose_map[:null_idx].decode('ascii', errors='replace')
            except:
                pass

            # Check if these look like valid footer data
            # winFlag should be 0 or 1
            # map names should be ASCII strings or empty
            valid_win = win_flag in [0, 1]
            valid_maps = all(
                (32 <= b < 127 or b == 0) for b in win_map
            ) and all(
                (32 <= b < 127 or b == 0) for b in lose_map
            )

            marker = " <<<" if (valid_win and valid_maps and
                               (win_map_str or lose_map_str or win_flag == 0)) else ""

            print(f"\n  footer_size={footer_size}: winFlag={win_flag}, "
                  f"winMap='{win_map_str}', loseMap='{lose_map_str}'{marker}")

    # Calculate event data size for each footer size
    print("\n  Event data alignment check:")

    # First, find the variables/events boundary
    unit_count = u32(data, 0x270)
    tail_offset = 628 + unit_count * 544
    area_count = u32(data, tail_offset)
    after_areas = tail_offset + 4 + area_count * 84
    var_count = u32(data, after_areas)
    after_vars = after_areas + 4 + var_count * 76
    gap = 8  # Known [1, 0] gap
    ec_off = after_vars + gap
    ec = u32(data, ec_off)
    events_start = ec_off + 4

    print(f"  events_start=0x{events_start:X}, eventCount={ec}")

    for footer_size in [40, 41, 42, 43, 44, 45, 46]:
        footer_start = file_size - footer_size
        ev_data = footer_start - events_start
        avg = ev_data / expected_events if expected_events else 0
        mod4 = ev_data % 4
        print(f"    footer={footer_size}: ev_data={ev_data} ({ev_data:#x}), "
              f"avg={avg:.1f}, mod4={mod4}")


def main():
    files = [
        ("E1140.stg", 2),
        ("TUTO_D1.stg", 47),
        ("E1001.stg", 16),
        ("X3001.stg", 46),
    ]

    for name, ec in files:
        analyze_footer(name, ec)


if __name__ == '__main__':
    main()
