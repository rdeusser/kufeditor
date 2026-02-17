#!/usr/bin/env python3
"""Parse and print save files from Kingdom Under Fire: Crusaders.

Outputs human-readable troop listings with decoded equipment names.
Pass --json for raw JSON output instead.
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "generated"))

from kuf_save import EquipmentSlot, File, SkillType, ResistType, UCD


def load_weapon_names(base_dir: Path) -> dict[int, list[dict]]:
    """Load WeaponNames_ENG.txt into {item_type_id: [{id, short, long}, ...]}."""
    path = base_dir / "Text" / "ENG" / "WeaponNames_ENG.txt"
    if not path.exists():
        return {}

    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    idx = 0
    total_types = int(lines[idx].strip())
    idx += 1

    result = {}
    item_type_id = 0
    for _ in range(total_types):
        # skip blank lines
        while idx < len(lines) and lines[idx].strip() == "":
            idx += 1
        if idx >= len(lines):
            break

        # first line: variant_count + optional comment with item type name
        header = lines[idx].strip()
        idx += 1
        variant_count = int(header.split("\t")[0].split("//")[0].strip())

        variants = []
        for _ in range(variant_count):
            variant_id = int(lines[idx].strip())
            idx += 1
            short_name = lines[idx].strip()
            idx += 1
            long_name = lines[idx].strip()
            idx += 1
            variants.append({"id": variant_id, "short": short_name, "long": long_name})

        result[item_type_id] = variants
        item_type_id += 1

    return result


def load_item_att_info(base_dir: Path) -> dict[int, dict]:
    """Load ItemAttInfo_ENG.sox into {record_index: {name, description}}."""
    path = base_dir / "SOX" / "ENG" / "ItemAttInfo_ENG.sox"
    if not path.exists():
        return {}

    data = path.read_bytes()
    offset = 0
    marker = struct.unpack_from("<I", data, offset)[0]
    offset += 4
    count = struct.unpack_from("<I", data, offset)[0]
    offset += 4

    result = {}
    for i in range(count):
        rec_id = struct.unpack_from("<I", data, offset)[0]
        offset += 4
        name_len = struct.unpack_from("<H", data, offset)[0]
        offset += 2
        name = data[offset:offset + name_len].decode("ascii", errors="replace")
        offset += name_len
        desc_len = struct.unpack_from("<H", data, offset)[0]
        offset += 2
        desc = data[offset:offset + desc_len].decode("ascii", errors="replace")
        offset += desc_len
        result[i] = {"name": name, "description": desc}

    return result


def load_item_type_prefixes(base_dir: Path) -> dict[int, list[str]]:
    """Load ItemTypeInfo_ENG.sox tier prefix strings into {item_type_id: [tier0, tier1, tier2]}."""
    path = base_dir / "SOX" / "ENG" / "ItemTypeInfo_ENG.sox"
    if not path.exists():
        return {}

    data = path.read_bytes()
    offset = 0
    marker = struct.unpack_from("<I", data, offset)[0]
    offset += 4
    count = struct.unpack_from("<I", data, offset)[0]
    offset += 4

    result = {}
    for i in range(count):
        rec_id = struct.unpack_from("<I", data, offset)[0]
        offset += 4
        tiers = []
        for _ in range(3):
            str_len = struct.unpack_from("<H", data, offset)[0]
            offset += 2
            s = data[offset:offset + str_len].decode("ascii", errors="replace")
            offset += str_len
            tiers.append(s)
        result[rec_id] = tiers

    return result


class EquipmentDecoder:
    def __init__(self, base_dir: Path):
        self.weapon_names = load_weapon_names(base_dir)
        self.item_att_info = load_item_att_info(base_dir)
        self.item_type_prefixes = load_item_type_prefixes(base_dir)

    def decode_slot(self, slot: EquipmentSlot) -> str:
        if slot.item_type_id == -1:
            return "(empty)"

        parts = []

        # base name from WeaponNames
        name = self._get_item_name(slot)
        parts.append(name)

        # attribute suffixes
        attrs = []
        if slot.attribute1_index >= 0 and slot.attribute1_index in self.item_att_info:
            attrs.append(self.item_att_info[slot.attribute1_index]["name"])
        if slot.attribute2_index >= 0 and slot.attribute2_index in self.item_att_info:
            attrs.append(self.item_att_info[slot.attribute2_index]["name"])
        if attrs:
            parts[0] += " of " + " and ".join(attrs)

        # level
        parts.append(f"Lv{slot.level}")

        # skills
        skill_strs = []
        if slot.skill_type_1 != SkillType.NONE:
            skill_strs.append(f"{slot.skill_type_1.name}+{slot.skill_bonus_1}")
        if slot.skill_type_2 != SkillType.NONE:
            skill_strs.append(f"{slot.skill_type_2.name}+{slot.skill_bonus_2}")
        if skill_strs:
            parts.append("[" + ", ".join(skill_strs) + "]")

        # resistances
        resist_strs = []
        if slot.resist_type_1 != ResistType.NONE:
            resist_strs.append(f"{slot.resist_type_1.name}({slot.resist_bonus_1})")
        if slot.resist_type_2 != ResistType.NONE:
            resist_strs.append(f"{slot.resist_type_2.name}({slot.resist_bonus_2})")
        if resist_strs:
            parts.append("[Resist: " + ", ".join(resist_strs) + "]")

        # attribute descriptions
        att_descs = []
        if slot.attribute1_index >= 0 and slot.attribute1_index in self.item_att_info:
            att_descs.append(self.item_att_info[slot.attribute1_index]["description"])
        if slot.attribute2_index >= 0 and slot.attribute2_index in self.item_att_info:
            att_descs.append(self.item_att_info[slot.attribute2_index]["description"])
        if att_descs:
            parts.append("[" + ", ".join(att_descs) + "]")

        return ", ".join(parts)

    def _get_item_name(self, slot: EquipmentSlot) -> str:
        type_id = slot.item_type_id
        variants = self.weapon_names.get(type_id)

        if variants is None:
            return f"item_{type_id}"

        # find variant by index
        variant = None
        if slot.variant_index < len(variants):
            variant = variants[slot.variant_index]
        elif variants:
            # try matching by variant_id
            for v in variants:
                if v["id"] == slot.variant_index + 1:
                    variant = v
                    break
            if variant is None:
                variant = variants[0]

        if variant is None:
            return f"item_{type_id}"

        # enhanced items use tier prefix + short name
        if slot.enhancement_tier >= 0:
            prefixes = self.item_type_prefixes.get(type_id, [])
            if 0 <= slot.enhancement_tier < len(prefixes):
                prefix = prefixes[slot.enhancement_tier]
                if prefix and not prefix.endswith(" "):
                    prefix += " "
                return f"{prefix}{variant['short']}"
            return variant["short"]

        return variant["long"]


def group_troops(units):
    troops = []
    i = 0
    while i < len(units):
        unit = units[i]
        if unit.ucd == UCD.Player:
            troop = {"leader": unit, "officers": []}
            i += 1
            while i < len(units) and units[i].ucd in (UCD.Enemy, UCD.Ally):
                troop["officers"].append(units[i])
                i += 1
            troops.append(troop)
        else:
            troops.append({"leader": unit, "officers": []})
            i += 1
    return troops


SLOT_LABELS = ["Weapon", "Accessory", "Armor"]


def leader_slots(unit):
    return [unit.leader_weapon, unit.leader_accessory, unit.leader_armor]


def troop_slots(unit):
    return [unit.troop_weapon, unit.troop_accessory, unit.troop_armor]


def print_unit(unit, decoder: EquipmentDecoder, indent: str = "", role: str = ""):
    if not role:
        role = unit.ucd.name
    print(f"{indent}[{role}] char_id={unit.char_id}, job_type={unit.job_type}, skill_level={unit.skill_level}")

    for i, slot in enumerate(leader_slots(unit)):
        if slot.item_type_id != -1:
            label = SLOT_LABELS[i] if i < len(SLOT_LABELS) else f"Slot{i}"
            decoded = decoder.decode_slot(slot)
            print(f"{indent}  {label:12s}: {decoded}")

    troop_labels = ["Troop Wpn", "Troop Acc", "Troop Armor"]
    for i, slot in enumerate(troop_slots(unit)):
        if slot.item_type_id != -1:
            label = troop_labels[i] if i < len(troop_labels) else f"TroopSlot{i}"
            decoded = decoder.decode_slot(slot)
            print(f"{indent}  {label:12s}: {decoded}")


def main():
    base_dir = Path(__file__).parent
    sav_path = base_dir / "Saves" / "KUF2 Crusader" / "SAVE GAME 1" / "kuf.sav"
    json_mode = False

    args = sys.argv[1:]
    if "--json" in args:
        json_mode = True
        args.remove("--json")
    if args:
        sav_path = Path(args[0])

    if not sav_path.exists():
        print(f"Error: {sav_path} not found", file=sys.stderr)
        sys.exit(1)

    data = sav_path.read_bytes()
    parsed, _ = File.parse(data)

    if json_mode:
        print(parsed.to_json())
        return

    decoder = EquipmentDecoder(base_dir)

    campaign_names = {
        0: "Hironeiden (Gerald)",
        1: "Vellond (Lucretia)",
        2: "Ecclesia (Kendal)",
        3: "Dark Legion (Regnier)",
    }
    print(f"Campaign: {campaign_names.get(parsed.campaign_index, f'Unknown({parsed.campaign_index})')}")
    print(f"Units: {parsed.unit_count}")
    print(f"Mission slot: {parsed.current_mission_slot}")
    print()

    troops = group_troops(parsed.units)
    for troop_idx, troop in enumerate(troops):
        leader = troop["leader"]
        officers = troop["officers"]

        print(f"--- Troop {troop_idx + 1} ---")
        print_unit(leader, decoder)

        for i, officer in enumerate(officers):
            print_unit(officer, decoder, indent="  ", role=f"Officer {i + 1}")

        print()

    # round-trip validation
    rebuilt = parsed.to_bytes()
    if rebuilt == data:
        print("(round-trip: OK)")
    else:
        mismatch = sum(1 for a, b in zip(rebuilt, data) if a != b)
        print(f"(round-trip: {mismatch} byte differences out of {len(data)})")


if __name__ == "__main__":
    main()
