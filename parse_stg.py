#!/usr/bin/env python3
"""
STG file parser for Kingdom Under Fire: The Crusaders (PC port).

Format verified against Ghidra decompilation of ReadSTGFile (0x00489bc0)
in Kuf2Main.exe and validated against all 178 STG files in the game data.

Usage:
    python3 parse_stg.py <file.stg>              # Parse single file
    python3 parse_stg.py <directory>              # Parse all STG files in directory tree
    python3 parse_stg.py <file.stg> --verbose     # Detailed output
    python3 parse_stg.py <file.stg> --json        # JSON output
    python3 parse_stg.py <dir> --sox-dir SOX/     # Resolve display names from SOX data
    python3 parse_stg.py <dir> --sox-dir SOX/ --lang JAP  # Use Japanese localization
"""

import argparse
import json
import os
import struct
import sys

STG_MAGIC = 0x3E9  # 1001 — format marker, NOT a mission ID
HEADER_BODY_SIZE = 0x26C  # 620 bytes
UNIT_BLOCK_SIZE = 0x220  # 544 bytes
AREA_ENTRY_SIZE = 0x54  # 84 bytes
FOOTER_ENTRY_SIZE = 8

PARAM_TYPE_INT = 0
PARAM_TYPE_FLOAT = 1
PARAM_TYPE_STRING = 2
PARAM_TYPE_ENUM = 3

PARAM_TYPE_NAMES = {0: "int", 1: "float", 2: "string", 3: "enum"}

UCD_NAMES = {0: "PLAYER", 1: "AI_ENEMY", 2: "AI_ALLY", 3: "AI_NEUTRAL"}
DIR_NAMES = {0: "E", 1: "NE", 2: "N", 3: "NW", 4: "W", 5: "SW", 6: "S", 7: "SE"}

# K2_JOB_TYPE enum from K2JobDef.h (values 0-42)
JOB_TYPE_NAMES = {
    0: "H_ARCHER",
    1: "H_LONGBOW_MAN",
    2: "H_INFANTRY",
    3: "H_SPEARMAN",
    4: "H_H_INFANTRY",
    5: "H_KNIGHT",
    6: "H_PALADIN",
    7: "H_CAVALRY",
    8: "H_H_CAVALRY",
    9: "H_STORM_RIDER",
    10: "H_SAPPER",
    11: "H_PYRO_TECHNICIAN",
    12: "H_BOMBER_WING",
    13: "H_MORTAR",
    14: "H_BALLISTA",
    15: "H_HARPOON",
    16: "H_CATAPULT",
    17: "H_BATTALOON",
    18: "DE_ARCHER",
    19: "DE_CAVALRY_ARCHER",
    20: "DE_FIGHTER",
    21: "DE_KNIGHT",
    22: "DE_LIGHT_CAVALRY",
    23: "DO_INFANTRY",
    24: "DO_RIDER",
    25: "DO_H_A_RIDERS",
    26: "DO_AXE_MAN",
    27: "DO_H_A_INFANTRY",
    28: "DO_SAPPER",
    29: "D_SCORPION",
    30: "D_SWAMP_MAMMOTH",
    31: "D_DIRIGIBLE",
    32: "D_BLACK_WYVERN",
    33: "DO_GHOUL",
    34: "D_BONE_DRAGON",
    35: "WALL",
    36: "SCOUT",
    37: "SELFDESTRUCTION",
    38: "ENCABLOSA_MONSTER",
    39: "ENCABLOSA_FLYING_MONSTER",
    40: "ENCABLOSA_RANGED",
    41: "ELF_WALL",
    42: "ENCABLOSA_LARGE",
}

FACTION_NAMES = {
    0: "Human",
    1: "Dark Orc",
    2: "Dark Elf",
    3: "Dark Special",
    4: "Encablossa",
    5: "Undead",
    6: "Ogre",
}

# Fallback TroopInfo index when troop_info_index == -1 (GetDefaultTroopInfoIndex at 0x004b4520)
_DEFAULT_TROOP_INFO = {
    1: 0, 2: 3, 3: 7, 4: 9, 5: 16, 6: 26, 7: 12, 8: 35,
    9: 13, 10: 19, 11: 10, 12: 37, 13: 18, 14: 6, 15: 29,
    0x20: 17, 0x21: 14, 0x23: 30, 0x24: 40, 0x25: 41, 0x26: 42, 0x27: 42,
}

# CharInfo job types from GetUnitDisplayName switch (0x005597a0)
_CHAR_INFO_JOB_TYPES = {32, 33, 34, 35, 36, 37, 38, 43, 44, 46, 47}


def read_u32(data, pos):
    return struct.unpack_from("<I", data, pos)[0], pos + 4


def read_i32(data, pos):
    return struct.unpack_from("<i", data, pos)[0], pos + 4


def read_f32(data, pos):
    return struct.unpack_from("<f", data, pos)[0], pos + 4


def read_string(data, pos, length):
    raw = data[pos : pos + length]
    return raw.split(b"\x00")[0].decode("ascii", errors="replace"), pos + length


def read_param_value(data, pos):
    ptype, pos = read_u32(data, pos)
    if ptype == PARAM_TYPE_STRING:
        slen, pos = read_u32(data, pos)
        sval = data[pos : pos + slen].decode("ascii", errors="replace")
        pos += slen
        return {"type": "string", "value": sval}, pos
    elif ptype == PARAM_TYPE_FLOAT:
        fval, pos = read_f32(data, pos)
        return {"type": "float", "value": fval}, pos
    elif ptype == PARAM_TYPE_ENUM:
        ival, pos = read_i32(data, pos)
        return {"type": "enum", "value": ival}, pos
    else:
        ival, pos = read_i32(data, pos)
        return {"type": "int", "value": ival}, pos


def load_indexed_name_sox(filepath):
    """Load a SOX file with uint32 index + uint16 len + string records."""
    try:
        with open(filepath, "rb") as f:
            data = f.read()
    except OSError:
        return {}

    if len(data) < 8:
        return {}

    marker, count = struct.unpack_from("<II", data, 0)
    if marker != 100:
        return {}

    names = {}
    pos = 8
    for _ in range(count):
        if pos + 6 > len(data) or data[pos : pos + 5] == b"THEND":
            break
        idx = struct.unpack_from("<I", data, pos)[0]
        slen = struct.unpack_from("<H", data, pos + 4)[0]
        pos += 6
        if pos + slen > len(data):
            break
        names[idx] = data[pos : pos + slen].decode("utf-8", errors="replace")
        pos += slen

    return names


def load_special_names(sox_path, localized_path):
    """Load SpecialNames.sox keys paired with localized display names."""
    try:
        with open(sox_path, "rb") as f:
            sox_data = f.read()
    except OSError:
        return []

    if len(sox_data) < 8:
        return []

    marker, count = struct.unpack_from("<II", sox_data, 0)
    if marker != 100:
        return []

    keys = []
    pos = 8
    for _ in range(count):
        if pos + 2 > len(sox_data) or sox_data[pos : pos + 5] == b"THEND":
            break
        key_len = struct.unpack_from("<H", sox_data, pos)[0]
        pos += 2
        if pos + key_len > len(sox_data):
            break
        key_bytes = sox_data[pos : pos + key_len]
        pos += key_len

        if pos + 2 > len(sox_data):
            break
        default_len = struct.unpack_from("<H", sox_data, pos)[0]
        pos += 2
        default_str = ""
        if default_len > 0 and pos + default_len <= len(sox_data):
            default_str = sox_data[pos : pos + default_len].decode(
                "utf-8", errors="replace"
            )
        pos += default_len

        keys.append((key_bytes, default_str))

    try:
        with open(localized_path, "rb") as f:
            loc_data = f.read()
    except OSError:
        return keys

    if len(loc_data) < 8:
        return keys

    loc_marker, loc_count = struct.unpack_from("<II", loc_data, 0)
    if loc_marker != 100:
        return keys

    display_names = []
    pos = 8
    for _ in range(loc_count):
        if pos + 2 > len(loc_data) or loc_data[pos : pos + 5] == b"THEND":
            break
        slen = struct.unpack_from("<H", loc_data, pos)[0]
        pos += 2
        if pos + slen > len(loc_data):
            break
        display_names.append(
            loc_data[pos : pos + slen].decode("utf-8", errors="replace")
        )
        pos += slen

    result = []
    for i, (key_bytes, default_name) in enumerate(keys):
        if i < len(display_names) and display_names[i]:
            result.append((key_bytes, display_names[i]))
        else:
            result.append((key_bytes, default_name))

    return result


class SoxData:
    """Loaded SOX data for unit name resolution."""

    def __init__(self, sox_dir, lang="ENG"):
        lang_dir = os.path.join(sox_dir, lang)
        self.troop_names = load_indexed_name_sox(
            os.path.join(lang_dir, f"TroopInfo_{lang}.sox")
        )
        self.char_names = load_indexed_name_sox(
            os.path.join(lang_dir, f"CharInfo_{lang}.sox")
        )
        self.special_names = load_special_names(
            os.path.join(sox_dir, "SpecialNames.sox"),
            os.path.join(lang_dir, f"SpecialNames_{lang}.sox"),
        )
        self.loaded = bool(self.troop_names or self.char_names or self.special_names)


def get_faction(job_type):
    """Map job_type to faction (replicates GetFactionFromJobType at 0x004b4660)."""
    if 23 <= job_type <= 28:
        fid = 1
    elif job_type in (18, 19, 20, 21, 22, 41):
        fid = 2
    elif job_type in (33, 34):
        fid = 3
    elif job_type in (38, 39, 40, 42):
        fid = 4
    elif job_type in (9, 12, 17):
        fid = 5
    elif 29 <= job_type <= 32:
        fid = 6
    else:
        fid = 0
    return fid, FACTION_NAMES[fid]


def get_default_troop_info_index(formation_type):
    """Fallback TroopInfo index when troop_info_index is -1."""
    return _DEFAULT_TROOP_INFO.get(formation_type, 2)


def _ascii_lower(b):
    """ASCII-only lowercase, matching MSVC strlwr behavior."""
    return bytes(c | 0x20 if 0x41 <= c <= 0x5A else c for c in b)


def resolve_special_name(name_bytes, special_names):
    """Prefix-match name bytes against SpecialNames.sox keys."""
    if not special_names:
        return None
    lowered = _ascii_lower(name_bytes)
    for key_bytes, display_name in special_names:
        key_lowered = _ascii_lower(key_bytes)
        klen = len(key_lowered)
        if len(lowered) >= klen and lowered[:klen] == key_lowered:
            return display_name
    return None


def get_unit_display_name(name_raw, job_type, sub_type, sox):
    """Resolve display name following the game's priority chain (0x005597a0)."""
    if sox is None or not sox.loaded:
        return None

    name_bytes = name_raw.split(b"\x00")[0]

    try_special = False
    if name_bytes and name_bytes[0] == 0x2D:
        try_special = True
    elif job_type == 6 and sub_type > 12:
        try_special = True
    elif job_type == 19 and sub_type > 6:
        try_special = True

    if try_special:
        result = resolve_special_name(name_bytes, sox.special_names)
        if result:
            return result

    if job_type == 26 and sub_type < 1:
        name = sox.char_names.get(job_type)
        if name:
            return name
    elif job_type in _CHAR_INFO_JOB_TYPES:
        name = sox.char_names.get(job_type)
        if name:
            return name

    if 0 <= job_type <= 42:
        return sox.troop_names.get(job_type)

    return None


def find_sox_dir(start_path):
    """Walk up from start_path looking for a sibling SOX/ directory."""
    path = os.path.abspath(start_path)
    if os.path.isfile(path):
        path = os.path.dirname(path)
    for _ in range(5):
        candidate = os.path.join(path, "SOX")
        if os.path.isdir(candidate):
            return candidate
        parent = os.path.dirname(path)
        if parent == path:
            break
        path = parent
    return None


def parse_header(data, pos):
    header = {}
    header["magic"], pos = read_u32(data, pos)
    if header["magic"] != STG_MAGIC:
        raise ValueError(
            f"Bad STG magic: 0x{header['magic']:08X} (expected 0x{STG_MAGIC:08X})"
        )

    body_start = pos

    header["reserved"], pos = read_i32(data, pos)
    header["unknown_08"], pos = read_u32(data, pos)
    header["unknown_0c"], pos = read_u32(data, pos)
    header["unknown_10"], pos = read_u32(data, pos)

    header["map_file"], _ = read_string(data, body_start + 0x44, 64)
    header["bitmap_file"], _ = read_string(data, body_start + 0x84, 64)
    header["default_cam"], _ = read_string(data, body_start + 0xC4, 64)
    header["user_cam"], _ = read_string(data, body_start + 0x104, 64)
    header["settings_file"], _ = read_string(data, body_start + 0x144, 64)
    header["sky_effects"], _ = read_string(data, body_start + 0x184, 64)
    header["ai_script"], _ = read_string(data, body_start + 0x1C4, 64)
    header["cubemap"], _ = read_string(data, body_start + 0x208, 64)

    pos = body_start + HEADER_BODY_SIZE
    header["unit_count"], pos = read_i32(data, pos)

    return header, pos


def parse_unit(data, pos, sox=None):
    unit_start = pos
    unit = {}

    name_raw = data[pos : pos + 32]
    unit["name"], _ = read_string(data, pos, 32)
    unit["unique_id"], _ = read_u32(data, pos + 0x20)
    unit["ucd"] = data[pos + 0x24]
    unit["ucd_name"] = UCD_NAMES.get(unit["ucd"], f"UNKNOWN({unit['ucd']})")
    unit["is_hero"] = data[pos + 0x25]
    unit["is_enabled"] = data[pos + 0x26]

    unit["leader_hp_override"], _ = read_f32(data, pos + 0x28)
    unit["unit_hp_override"], _ = read_f32(data, pos + 0x2C)

    unit["position_x"], _ = read_f32(data, pos + 0x44)
    unit["position_y"], _ = read_f32(data, pos + 0x48)
    unit["direction"] = data[pos + 0x4C]
    unit["direction_name"] = DIR_NAMES.get(
        unit["direction"], f"UNKNOWN({unit['direction']})"
    )

    unit["job_type"] = data[pos + 0x54]
    jt = unit["job_type"]
    if jt in JOB_TYPE_NAMES:
        unit["job_type_name"] = JOB_TYPE_NAMES[jt]
    elif sox is not None and sox.loaded and jt in sox.char_names:
        unit["job_type_name"] = sox.char_names[jt]
    else:
        unit["job_type_name"] = f"UNKNOWN({jt})"
    unit["model_id"] = data[pos + 0x55]
    unit["worldmap_id"] = data[pos + 0x56]
    unit["level"] = data[pos + 0x57]

    unit["officer_count"], _ = read_u32(data, pos + 0xBC)

    unit["troop_info_index"], _ = read_i32(data, pos + 0x1C0)
    unit["formation_type"], _ = read_u32(data, pos + 0x1C4)

    if unit["troop_info_index"] >= 0:
        unit["effective_troop_info_index"] = unit["troop_info_index"]
    else:
        unit["effective_troop_info_index"] = get_default_troop_info_index(
            unit["formation_type"]
        )

    faction_id, faction_name = get_faction(jt)
    unit["faction_id"] = faction_id
    unit["faction"] = faction_name

    unit["display_name"] = get_unit_display_name(
        name_raw, jt, unit["model_id"], sox
    )
    unit["troop_display_name"] = (
        sox.troop_names.get(unit["effective_troop_info_index"])
        if sox is not None and sox.loaded
        else None
    )

    overrides = []
    for i in range(22):
        val, _ = read_f32(data, pos + 0x1C8 + i * 4)
        overrides.append(val)
    unit["stat_overrides"] = overrides
    unit["has_overrides"] = any(v >= 0.0 for v in overrides)

    pos = unit_start + UNIT_BLOCK_SIZE
    return unit, pos


def parse_area(data, pos):
    area = {}
    area["description"], _ = read_string(data, pos, 32)
    area["unknown_20"], _ = read_u32(data, pos + 0x20)
    area["area_id"], _ = read_u32(data, pos + 0x40)
    area["bound_x1"], _ = read_f32(data, pos + 0x44)
    area["bound_y1"], _ = read_f32(data, pos + 0x48)
    area["bound_x2"], _ = read_f32(data, pos + 0x4C)
    area["bound_y2"], _ = read_f32(data, pos + 0x50)
    return area, pos + AREA_ENTRY_SIZE


def parse_variable(data, pos):
    var = {}
    var["name"], pos = read_string(data, pos, 64)
    pos_after_name = pos - 64 + 64  # re-align
    var["id"], pos = read_u32(data, pos)
    var["initial_value"], pos = read_param_value(data, pos)
    return var, pos


def parse_condition(data, pos):
    cond = {}
    cond["type_id"], pos = read_u32(data, pos)
    param_count, pos = read_u32(data, pos)
    cond["params"] = []
    for _ in range(param_count):
        param, pos = read_param_value(data, pos)
        cond["params"].append(param)
    return cond, pos


def parse_action(data, pos):
    act = {}
    act["type_id"], pos = read_u32(data, pos)
    param_count, pos = read_u32(data, pos)
    act["params"] = []
    for _ in range(param_count):
        param, pos = read_param_value(data, pos)
        act["params"].append(param)
    return act, pos


def parse_event(data, pos):
    event = {}
    event["description"], pos = read_string(data, pos, 64)
    event["id"], pos = read_u32(data, pos)

    cond_count, pos = read_i32(data, pos)
    event["conditions"] = []
    for _ in range(cond_count):
        cond, pos = parse_condition(data, pos)
        event["conditions"].append(cond)

    act_count, pos = read_i32(data, pos)
    event["actions"] = []
    for _ in range(act_count):
        act, pos = parse_action(data, pos)
        event["actions"].append(act)

    return event, pos


def parse_stg(data, sox=None):
    result = {}
    pos = 0

    header, pos = parse_header(data, pos)
    result["header"] = header

    result["units"] = []
    for _ in range(header["unit_count"]):
        unit, pos = parse_unit(data, pos, sox=sox)
        result["units"].append(unit)

    area_count, pos = read_i32(data, pos)
    result["areas"] = []
    for _ in range(area_count):
        area, pos = parse_area(data, pos)
        result["areas"].append(area)

    var_count, pos = read_i32(data, pos)
    result["variables"] = []
    for _ in range(var_count):
        var, pos = parse_variable(data, pos)
        result["variables"].append(var)

    block_count, pos = read_i32(data, pos)
    result["event_blocks"] = []
    for _ in range(block_count):
        block = {}
        block["header"], pos = read_u32(data, pos)
        event_count, pos = read_i32(data, pos)
        block["events"] = []
        for _ in range(event_count):
            event, pos = parse_event(data, pos)
            block["events"].append(event)
        result["event_blocks"].append(block)

    footer_count, pos = read_i32(data, pos)
    result["footer"] = []
    for _ in range(footer_count):
        a, pos = read_u32(data, pos)
        b, pos = read_u32(data, pos)
        result["footer"].append({"field1": a, "field2": b})

    result["_file_size"] = len(data)
    result["_bytes_parsed"] = pos
    result["_remaining"] = len(data) - pos

    return result


def print_summary(path, result):
    fname = os.path.basename(path)
    h = result["header"]
    units = result["units"]
    areas = result["areas"]
    variables = result["variables"]
    blocks = result["event_blocks"]
    footer = result["footer"]
    total_events = sum(len(b["events"]) for b in blocks)
    remaining = result["_remaining"]

    status = "OK" if remaining == 0 else f"WARN: {remaining} bytes remaining"

    print(f"\n{'=' * 60}")
    print(f"  {fname} ({result['_file_size']} bytes) [{status}]")
    print(f"{'=' * 60}")
    print(f"  Map: {h['map_file']}")
    print(f"  Bitmap: {h['bitmap_file']}")
    print(f"  AI Script: {h['ai_script']}")
    print(f"  Sky: {h['sky_effects']}")
    print(f"  Settings: {h['settings_file']}")
    print(
        f"  Sections: {len(units)} units, {len(areas)} areas, "
        f"{len(variables)} vars, {len(blocks)} blocks ({total_events} events), "
        f"{len(footer)} footer entries"
    )


def print_verbose(path, result):
    print_summary(path, result)

    if result["units"]:
        print(f"\n  --- Units ({len(result['units'])}) ---")
        for i, u in enumerate(result["units"]):
            overrides = " [HAS OVERRIDES]" if u["has_overrides"] else ""
            troop_idx = u["troop_info_index"]
            eff_idx = u["effective_troop_info_index"]
            idx_str = (
                f"{troop_idx}->{eff_idx}" if troop_idx < 0 else str(troop_idx)
            )
            extras = f" Faction={u['faction']}"
            if u["display_name"] is not None:
                extras += f" Display=\"{u['display_name']}\""
            if u["troop_display_name"] is not None:
                extras += f" Troop=\"{u['troop_display_name']}\""
            print(
                f"    [{i:2d}] \"{u['name']}\" UID={u['unique_id']} "
                f"UCD={u['ucd_name']} Hero={u['is_hero']} Enabled={u['is_enabled']} "
                f"Pos=({u['position_x']:.0f},{u['position_y']:.0f}) "
                f"Dir={u['direction_name']} Job={u['job_type']}({u['job_type_name']}) Model={u['model_id']} "
                f"Lv={u['level']} TroopIdx={idx_str} "
                f"Officers={u['officer_count']}{extras}{overrides}"
            )

    if result["areas"]:
        print(f"\n  --- Areas ({len(result['areas'])}) ---")
        for i, a in enumerate(result["areas"]):
            print(
                f"    [{i:2d}] \"{a['description']}\" ID={a['area_id']} "
                f"Bounds=({a['bound_x1']:.0f},{a['bound_y1']:.0f})-"
                f"({a['bound_x2']:.0f},{a['bound_y2']:.0f})"
            )

    if result["variables"]:
        print(f"\n  --- Variables ({len(result['variables'])}) ---")
        for i, v in enumerate(result["variables"]):
            iv = v["initial_value"]
            print(
                f"    [{i}] \"{v['name']}\" id={v['id']} "
                f"type={iv['type']} val={iv['value']}"
            )

    for bi, block in enumerate(result["event_blocks"]):
        print(
            f"\n  --- Event Block {bi} (header={block['header']}, "
            f"{len(block['events'])} events) ---"
        )
        for ei, ev in enumerate(block["events"]):
            desc = ev["description"][:50]
            print(
                f"    [{ei:2d}] \"{desc}\" ID={ev['id']} "
                f"conds={len(ev['conditions'])} acts={len(ev['actions'])}"
            )
            for ci, c in enumerate(ev["conditions"]):
                params = ", ".join(
                    f"{p['type']}:{p['value']}" for p in c["params"]
                )
                print(f"          CON 0x{c['type_id']:02X} ({c['type_id']}) [{params}]")
            for ai, a in enumerate(ev["actions"]):
                params = ", ".join(
                    f"{p['type']}:{p['value']}" for p in a["params"]
                )
                print(f"          ACT 0x{a['type_id']:02X} ({a['type_id']}) [{params}]")

    if result["footer"]:
        print(f"\n  --- Footer ({len(result['footer'])} entries) ---")
        for i, f in enumerate(result["footer"]):
            print(f"    [{i}] {f['field1']}, {f['field2']}")


def main():
    parser = argparse.ArgumentParser(
        description="Parse Kingdom Under Fire: Crusaders STG mission files"
    )
    parser.add_argument("path", help="STG file or directory to parse")
    parser.add_argument(
        "--verbose", "-v", action="store_true", help="Show detailed output"
    )
    parser.add_argument(
        "--json", "-j", action="store_true", help="Output as JSON"
    )
    parser.add_argument(
        "--quiet", "-q", action="store_true", help="Only show errors"
    )
    parser.add_argument(
        "--sox-dir", metavar="PATH", help="Path to SOX data directory"
    )
    parser.add_argument(
        "--lang", default="ENG", help="Language code for localized SOX files (default: ENG)"
    )
    args = parser.parse_args()

    sox = None
    sox_dir = args.sox_dir
    if sox_dir is None:
        sox_dir = find_sox_dir(args.path)
    if sox_dir is not None:
        sox = SoxData(sox_dir, args.lang)
        if not sox.loaded:
            print(
                f"Warning: no SOX data found in {sox_dir}",
                file=sys.stderr,
            )
            sox = None
        elif not args.quiet and not args.json:
            print(
                f"Loaded SOX data: {len(sox.troop_names)} troop names, "
                f"{len(sox.char_names)} char names, "
                f"{len(sox.special_names)} special names"
            )

    paths = []
    if os.path.isdir(args.path):
        for root, dirs, files in os.walk(args.path):
            for f in sorted(files):
                if f.lower().endswith(".stg"):
                    paths.append(os.path.join(root, f))
    elif os.path.isfile(args.path):
        paths.append(args.path)
    else:
        print(f"Error: {args.path} not found", file=sys.stderr)
        sys.exit(1)

    results = {}
    ok_count = 0
    fail_count = 0

    for path in paths:
        try:
            with open(path, "rb") as f:
                data = f.read()
            result = parse_stg(data, sox=sox)
            results[path] = result

            if result["_remaining"] != 0:
                print(
                    f"WARN: {path}: {result['_remaining']} bytes remaining",
                    file=sys.stderr,
                )

            if args.json:
                pass  # output all at end
            elif args.verbose:
                print_verbose(path, result)
            elif not args.quiet:
                print_summary(path, result)

            ok_count += 1
        except Exception as e:
            print(f"FAIL: {path}: {e}", file=sys.stderr)
            fail_count += 1

    if args.json:
        json_results = {}
        for path, result in results.items():
            json_results[os.path.basename(path)] = result
        print(json.dumps(json_results, indent=2, default=str))

    if not args.json and not args.quiet:
        print(f"\n--- Parsed {ok_count} files, {fail_count} failures ---")
        bad = [p for p, r in results.items() if r["_remaining"] != 0]
        if bad:
            print(f"*** {len(bad)} files with remaining bytes:")
            for p in bad:
                print(f"  {p}: {results[p]['_remaining']} remaining")
        elif ok_count > 0:
            print("All files parsed cleanly (0 remaining bytes).")


if __name__ == "__main__":
    main()
