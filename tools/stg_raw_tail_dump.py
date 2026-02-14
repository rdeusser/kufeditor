#!/usr/bin/env python3
"""Raw hex dump of the entire tail section of E1140.stg."""

import struct
from pathlib import Path

def u32(data, off):
    return struct.unpack_from('<I', data, off)[0]

def f32(data, off):
    return struct.unpack_from('<f', data, off)[0]

def hexdump(data, offset, length, prefix=""):
    for i in range(0, length, 16):
        off = offset + i
        n = min(16, length - i, len(data) - off)
        if n <= 0: break
        hx = ' '.join(f'{data[off+j]:02X}' for j in range(n))
        asc = ''.join(chr(data[off+j]) if 32 <= data[off+j] < 127 else '.' for j in range(n))
        print(f'{prefix}0x{off:04X}: {hx:<48} {asc}')

mission_dir = Path.home() / "Downloads" / "KUF Crusaders" / "Mission"

for name in ["E1140.stg"]:
    path = mission_dir / name
    data = path.read_bytes()
    file_size = len(data)

    unit_count = u32(data, 0x270)
    tail_offset = 628 + unit_count * 544

    print(f"# {name}: {file_size} bytes, {unit_count} units, tail at 0x{tail_offset:X}")
    print(f"# Tail size: {file_size - tail_offset} bytes")
    print()

    # Dump the ENTIRE tail
    print("=== ENTIRE TAIL SECTION ===")
    hexdump(data, tail_offset, file_size - tail_offset)

    print()

    # Also annotate known sections
    print("=== ANNOTATED PARSE ===")
    off = tail_offset

    # AreaIDs
    area_count = u32(data, off)
    area_section = 4 + area_count * 84
    print(f"AreaID count at 0x{off:X}: {area_count}")
    print(f"AreaID section: {area_section} bytes (0x{off:X} to 0x{off+area_section:X})")

    # Dump area names
    for i in range(area_count):
        area_base = off + 4 + i * 84
        desc = data[area_base:area_base+32]
        desc_end = desc.index(0) if 0 in desc else 32
        try:
            name_str = desc[:desc_end].decode('cp949', errors='replace')
        except:
            name_str = desc[:desc_end].decode('ascii', errors='replace')
        area_id = u32(data, area_base + 0x40)
        print(f"  area[{i}]: name='{name_str}' id={area_id}")
    off += area_section

    print(f"\nAfter areas: 0x{off:X}")
    print(f"Remaining: {file_size - off} bytes")
    print()

    # Now dump everything from here to end as annotated hex
    print("=== RAW AFTER AREAS ===")
    hexdump(data, off, file_size - off)

    # Try interpreting with fixed 76-byte variables
    print("\n=== TRYING 76-BYTE FIXED VARIABLES ===")
    var_count = u32(data, off)
    print(f"Variable count at 0x{off:X}: {var_count}")

    if var_count < 100:
        var_off = off + 4
        for i in range(var_count):
            vbase = var_off + i * 76
            if vbase + 76 > file_size:
                print(f"  var[{i}]: OUT OF BOUNDS at 0x{vbase:X}")
                break
            vname_raw = data[vbase:vbase+64]
            vname_end = vname_raw.index(0) if 0 in vname_raw else 64
            try:
                vname = vname_raw[:vname_end].decode('cp949', errors='replace')
            except:
                vname = vname_raw[:vname_end].decode('ascii', errors='replace')
            vid = u32(data, vbase + 64)
            vpad = u32(data, vbase + 68)
            vinit = u32(data, vbase + 72)
            print(f"  var[{i}]: name='{vname}' id={vid} pad={vpad} init={vinit}")
            hexdump(data, vbase, 76, "    ")

        var_section = 4 + var_count * 76
        after_vars = off + var_section
        print(f"  Variable section: {var_section} bytes")
        print(f"  After vars: 0x{after_vars:X}")
        print(f"  Remaining: {file_size - after_vars} bytes")
        print()
        print("  === AFTER VARIABLES ===")
        hexdump(data, after_vars, min(128, file_size - after_vars), "    ")

    # Also try variable-length null-terminated name + 12 bytes
    print("\n=== TRYING NULL-TERMINATED VARIABLES ===")
    var_count2 = u32(data, off)
    print(f"Variable count at 0x{off:X}: {var_count2}")

    if var_count2 < 100:
        voff = off + 4
        for i in range(var_count2):
            vstart = voff
            while voff < file_size and data[voff] != 0:
                voff += 1
            if voff >= file_size: break
            try:
                vname = data[vstart:voff].decode('cp949', errors='replace')
            except:
                vname = data[vstart:voff].decode('ascii', errors='replace')
            voff += 1  # skip null
            if voff + 12 > file_size: break
            # Try reading as: padding_to_align + id + init
            vid = u32(data, voff)
            vpad = u32(data, voff + 4)
            vinit = u32(data, voff + 8)
            print(f"  var[{i}]: name='{vname}' id={vid} pad={vpad} init={vinit}")
            hexdump(data, vstart, voff + 12 - vstart, "    ")
            voff += 12

        print(f"  After null-term vars: 0x{voff:X}")
        print(f"  Remaining: {file_size - voff} bytes")
        print()
        print("  === AFTER NULL-TERM VARIABLES ===")
        hexdump(data, voff, min(128, file_size - voff), "    ")
