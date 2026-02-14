# STG Format Documentation

STG files define unit placement and configuration for missions in Kingdom Under Fire: Crusaders (PC port).

## File Locations

- `Mission/` - Main mission STG files (e.g., `E1001.stg`)
- `Mission/Briefing/` - Pre-battle briefing STG files (unit selection screens)
- `Mission/Worldmap/` - World map unit data

## Mission File Naming

### Crusaders

Crusaders missions use a single-letter prefix identifying the campaign, followed by a 4-digit mission number:

| Prefix | Campaign | Protagonist | Mission Numbers | Unlock |
|--------|----------|-------------|-----------------|--------|
| `H####.stg` | Hironeiden | Gerald | H0000-H0200 | Available from start |
| `V####.stg` | Vellond | Lucretia | V2000-V2200 | Available from start |
| `E####.stg` | Ecclesia | Kendal | E1001-E1140 | Unlocked after Lucretia's campaign |
| `X####.stg` | Dark Legion | Regnier | X3001-X3140 | Unlocked after Lucretia's campaign |

The first digit of the mission number is campaign-specific (0=Gerald, 1=Kendal, 2=Lucretia, 3=Regnier), redundant with the letter prefix.

### Text File Association

Text files are linked by mission number: e.g., mission E1100 uses `UserTextTable1100_ENG.txt` from `Data/Text/ENG/`.

## Related Files

- `K2JobDef.h` - Unit/job type enum definitions (maps to Animation ID)
- `K2AbilityDef.h` - Ability type enum definitions
- `KufParticle_Def.h` - Particle effect IDs
- `CruMission.h` - Mission scripting conditions and actions
- `Text/KufReportMessage_Def.h` - Battle report message types
- `MISSION_SCRIPTING.md` - Documentation for event conditions and actions

---

## File Structure Overview

> **✓ VERIFIED FROM BINARY:** The structure below has been confirmed by Ghidra decompilation of `ReadSTGFile` at `0x00489bc0` in `Kuf2Main.exe`, and validated against all 178 STG files in the game data (0 bytes remaining in every file).

The full STG layout is: **Magic → Header Body → Unit Count → Units → AreaIDs → Variables → Event Blocks → Footer**.

| Section | Size | Description |
|---------|------|-------------|
| Magic | 4 bytes | Format marker `0x3E9` (1001) — read and validated first |
| Header Body | 620 bytes (0x26C) | Mission metadata, filenames, configuration |
| Unit Count | 4 bytes | Number of unit blocks to follow |
| Units | 544 bytes × unit_count | Unit definitions (0x220 each) |
| AreaIDs | 4 + (84 × area_count) | Named map regions for scripting |
| Variables | 4 + variable data | Variable-length mission state variables |
| Event Blocks | 4 + variable data | Variable-length event/trigger data (NOT fixed-size) |
| Footer | 4 + (8 × footer_count) | Unit selection slots (briefing files) or empty |

---

## Header Structure (628 bytes = 0x274)

The binary reads the header in three parts: 4-byte magic, 620-byte body (`_fread(dest, 0x26C, 1, file)`), then 4-byte unit count. The combined total is 628 bytes.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x000 | 4 | uint32 | **Format magic** `0x3E9` (1001) — NOT a mission ID. All STG files share this value. The game validates this before reading. |
| 0x004 | 4 | int32 | Reserved (-1) |
| 0x008 | 4 | uint32 | Unknown (typically 1) |
| 0x00C | 4 | uint32 | Unknown (typically 3) |
| 0x010 | 4 | uint32 | Unknown (typically 3) |
| 0x014 | 24 | - | Reserved zeros |
| 0x02C | 4 | uint32 | Unknown (typically 1) |
| 0x030 | 24 | - | Reserved zeros |
| 0x048 | 64 | string | Map filename (null-terminated, e.g., "E1001.map") |
| 0x088 | 64 | string | Bitmap filename (e.g., "E1001.bmp") |
| 0x0C8 | 64 | string | Default camera file (e.g., "Default.cam") |
| 0x108 | 64 | string | User camera file (e.g., "User.cam") |
| 0x148 | 64 | string | Settings file (e.g., "(08)000114.set") |
| 0x188 | 64 | string | Sky/cloud effects (e.g., "cloud08.smfx") |
| 0x1C8 | 64 | string | AI script file (e.g., "AI1001.txt") |
| 0x208 | 4 | - | Padding |
| 0x20C | 64 | string | Cubemap texture (e.g., "cubemap.dds") |
| 0x24C | 36 | - | Configuration data |
| 0x270 | 4 | uint32 | **Unit count** (numTroops) |

---

## Unit Block Structure (544 bytes = 0x220)

Base unit blocks are 544 bytes. Units with sub-units (player-controlled troops) may use 1088 bytes (0x440), containing two 544-byte entries.

### Core Unit Data (84 bytes = 0x54)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 32 | char[32] | Unit name + padding (null-terminated) |
| 0x20 | 4 | uint32 | Unique ID (must be unique per mission) |
| 0x24 | 1 | UCD | Unit Control Disposition (see enum below) |
| 0x25 | 1 | uint8 | IsHero: 0=No, 1=Yes |
| 0x26 | 1 | uint8 | IsEnabled: 0=Disabled, 1=Enabled |
| 0x27 | 1 | uint8 | Reserved |
| 0x28 | 4 | float | Leader HP override (-1.0 = use default) |
| 0x2C | 4 | float | Unit HP override (-1.0 = use default) |
| 0x30 | 4 | float | Unknown (typically 0.5) |
| 0x34 | 16 | - | Reserved |
| 0x44 | 4 | float | Position X (world coordinates) |
| 0x48 | 4 | float | Position Y (world coordinates) |
| 0x4C | 1 | Direction | Facing direction (see enum below) |
| 0x4D | 1 | uint8 | Extra flags |
| 0x4E | 1 | uint8 | Extra flags (0xFF for wildcards) |
| 0x4F | 1 | uint8 | Category (typically 3) |
| 0x50 | 4 | int32 | Reserved (-1) |

### Leader Configuration (108 bytes = 0x6C)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x54 | 1 | JobType | Unit type (see K2JobDef.h) |
| 0x55 | 1 | uint8 | Model ID variant (see Hero Model IDs below) |
| 0x56 | 1 | uint8 | Worldmap ID (see Worldmap ID Behavior) |
| 0x57 | 1 | uint8 | Level (1-99) |
| 0x58 | 8 | SkillSlot[4] | Leader skill slots (SkillID + Level per slot) |
| 0x60 | 92 | AbilityType[23] | Ability/equipment slots (see K2AbilityDef.h, -1 = empty) |
| 0xBC | 4 | uint32 | **Officer count** (0-2) |

### Officer 1 Data (104 bytes)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0xC0 | 1 | JobType | Officer 1 unit type |
| 0xC1 | 1 | uint8 | Officer 1 Model ID |
| 0xC2 | 1 | uint8 | Officer 1 Worldmap ID |
| 0xC3 | 1 | uint8 | Officer 1 Level |
| 0xC4 | 100 | - | Officer 1 skills/abilities |

### Officer 2 Data (88 bytes)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x128 | 1 | JobType | Officer 2 unit type |
| 0x129 | 1 | uint8 | Officer 2 Model ID |
| 0x12A | 1 | uint8 | Officer 2 Worldmap ID |
| 0x12B | 1 | uint8 | Officer 2 Level |
| 0x12C | 84 | - | Officer 2 skills/abilities |

### Unit Configuration (160 bytes = 0xA0)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x180 | 12 | - | Padding |
| 0x18C | 4 | uint32 | Unit animation/grid config |
| 0x190 | 4 | uint32 | Grid X dimension |
| 0x194 | 4 | uint32 | Grid Y dimension |
| 0x198 | 40 | - | Reserved |
| 0x1C0 | 4 | int32 | **TroopInfo index** (references TroopInfo.sox). If negative, computed from formation type at runtime |
| 0x1C4 | 4 | uint32 | Formation type |
| 0x1C8 | 88 | float[22] | Stat overrides (all -1.0 = use defaults) |

**Stat Override Logic** (confirmed from `ReadSTGFile` decompilation): The game reads the full 544-byte unit block, copies the first 448 bytes (0x1C0) as core unit data, then conditionally applies each of the 22 float overrides. Each override is checked: if `value >= 0.0`, it replaces the corresponding field from TroopInfo defaults. Values of `-1.0` (0xBF800000) are skipped, preserving the default.

---

## AreaID Section

Follows immediately after the units array. AreaIDs define named rectangular regions on the map used by mission scripting (e.g., spawn points, trigger zones, objective markers).

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | **AreaID count** (numAreaIDs) |
| +0x04 | 84 x count | AreaEntry[] | Area entries |

### AreaID Entry - Crusaders (84 bytes = 0x54)

Verified against E1100.stg (Crusaders).

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 32 | char[32] | Area description (null-terminated, often Korean + label) |
| 0x20 | 4 | uint32 | Unknown constant (observed: 15) |
| 0x24 | 4 | uint32 | Unknown |
| 0x28 | 24 | - | Unknown fields |
| 0x40 | 4 | uint32 | **Area ID** (sequential, referenced by event actions) |
| 0x44 | 4 | float | Bound X1 (world coordinates) |
| 0x48 | 4 | float | Bound Y1 (world coordinates) |
| 0x4C | 4 | float | Bound X2 (world coordinates) |
| 0x50 | 4 | float | Bound Y2 (world coordinates) |

Area IDs at +0x40 are sequential (0, 1, 2, 4, 13, 14...), and bounding floats contain realistic map coordinates (e.g., 6375, 7625, 12000, 36625).

### Area Behavior

- Large AreaIDs cause randomized positioning: if an AreaID covers a large region, any unit spawned or moved to that area will be placed at a random position within it.
- Areas can be visualized in-game using the `ACT_SHOW_AREA_ON_MINIMAP` action (ID 97 = 0x61, parameters: `AreaID`).
- Respawn actions (`ACT_RESPAWN_*`) reference both a TroopID and an AreaID. If the target AreaID covers a large area, the spawn position is random within it.

---

## Variable Section

Follows immediately after the AreaID section. Variables store mission state that can be read and written by event conditions and actions.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | **Variable count** (numVariables) |
| +0x04 | varies | Variable[] | Variable entries (variable-length) |

### Variable Entry Structure (confirmed from `ReadSTGFile` decompilation)

The game allocates a 76-byte (0x4C) object per variable, then reads the fields sequentially:

| Step | Size | Type | Description |
|------|------|------|-------------|
| 1 | 64 (0x40) | char[64] | Variable name (null-terminated, padded) |
| 2 | 4 | uint32 | Variable ID (used by conditions/actions to reference this variable) |
| 3 | varies | ParamValue | Initial value — read via `ReadSTGParamValue` (see Parameter Type System below) |

Observed variable names include `stage`, `cnt`, `att_castle`, `nCurStage`, `nDebug`, `_TROOP_INDICATE_IN_MINIMAP`, `LF_WALL`, and `COUT`.

### Variable Usage in Events

- **Read:** `CON_VAR_INT_COMPARE` (condition ID 19 = 0x13) checks a variable's value. Parameters: `[VariableID, Int, Compare]`.
- **Write:** `ACT_VAR_INT_SET` (action ID 55 = 0x37) sets a variable's value. Parameters: `[VariableID, Int]`.

See `MISSION_SCRIPTING.md` for full condition/action documentation.

---

## Event Section

Follows immediately after the variable section. Events use condition/action IDs defined in `CruMission.h` (see `MISSION_SCRIPTING.md` for full documentation).

> **✓ VERIFIED FROM BINARY:** Event entries are **variable-length**, NOT fixed 448 bytes as previously documented. The structure has been confirmed by decompilation of `ReadSTGFile` (0x00489bc0) and validated against all 178 STG files.

### Event Block Container

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | **Event block count** (typically 1) |

Each event block contains:

| Field | Size | Type | Description |
|-------|------|------|-------------|
| Block header | 4 | uint32 | Block identifier/flags (often 0) |
| Event count | 4 | uint32 | Number of events in this block |
| Events | varies | Event[] | Variable-length event entries |

### Event Entry (variable-length)

Each event is read sequentially — NOT fixed-size:

| Step | Size | Type | Description |
|------|------|------|-------------|
| 1 | 64 (0x40) | char[64] | Event description (editable label for identification) |
| 2 | 4 | uint32 | Unique event ID (must be unique per mission) |
| 3 | 4 | uint32 | **Condition count** |
| 4 | varies | Condition[] | Condition entries (see below) |
| 5 | 4 | uint32 | **Action count** |
| 6 | varies | Action[] | Action entries (see below) |

The game allocates a 100-byte (0x64) object per event, then reads the description (64 bytes) and ID (4 bytes) with `_fread`, followed by the variable-length condition and action arrays.

### Condition Entry Format

Each condition consists of:
1. **Condition type ID** (uint32) — read via `_fread`, maps to `CON_*` enum in CruMission.h
2. **Parameter count** (uint32) — number of parameters that follow
3. **Parameters** — each read via `ReadSTGParamValue` (see Parameter Type System below)

Example: `CON_VAR_INT_COMPARE` (ID 19 = 0x13)
- Parameters: `[VariableID, Int, Compare]`
- Checks: if variable equals/compares to int value

### Action Entry Format

Each action consists of:
1. **Action type ID** (uint32) — read via `_fread`, maps to `ACT_*` enum in CruMission.h
2. **Parameter count** (uint32) — number of parameters that follow
3. **Parameters** — each read via `ReadSTGParamValue` (see Parameter Type System below)

Example: `ACT_VAR_INT_SET` (ID 55 = 0x37)
- Parameters: `[VariableID, Int]`
- Effect: Sets variable to int value

### Parameter Type System (`ReadSTGParamValue` at 0x004847b0)

All condition/action parameters and variable initial values are read using a common typed-value reader. Each parameter consists of a 4-byte type tag followed by type-specific data:

| Type | Name | Data | Total Size | Description |
|------|------|------|------------|-------------|
| 0 | Int | 4 bytes (int32) | 8 bytes | Integer value |
| 1 | Float | 4 bytes (float32) | 8 bytes | Floating-point value |
| 2 | String | 4 bytes (length) + N bytes (chars) | 8+N bytes | Length-prefixed string |
| 3 | Enum | 4 bytes (int32) | 8 bytes | Enumerated value (same binary layout as int) |

This type system means event data is inherently variable-length — string parameters make entries different sizes.

### Respawn Actions

Respawn actions reference both a TroopID and an AreaID. If the target AreaID covers a large area, the spawn position is randomized within it.

### Text Linking

Events can reference text strings by ID from `Data/Text/{LANG}/` files (e.g., `Data/Text/ENG/UserTextTable1100_ENG.txt` for mission E1100). To find a specific event block:

1. Start the mission and note the first dialog text
2. Find that text's ID in the corresponding text file
3. Convert the text ID to hex (e.g., 2042 decimal = 0x07FA)
4. Search for the hex value (little-endian: `FA070000`) in the STG file
5. Navigate backwards to find the event block's description field

---

## Briefing File Structure

Briefing STG files (in `Mission/Briefing/`) control the pre-battle unit selection screen. They share the base STG format with some distinctive characteristics:

- **0 unit blocks** — units are defined in the corresponding mission file
- **Area definitions** for unit placement
- **1 event block** with 1 event (typically briefing configuration actions)
- **Footer entries** containing unit selection slot data (3-4 entries)

### Unit Selection Slots (Footer Section)

Unit selection slots are stored as the **footer entries** (8 bytes each). Each entry defines one unit the player can select or that is locked into the mission.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Slot data 1 (UniqueID / flags) |
| 0x04 | 4 | uint32 | Slot data 2 (configuration) |

### Briefing-Only Actions and Conditions

- **Unit count action** (`ACT_FE`, 0xFE): Controls how many units the player can bring. Written as `FE 01 00 00` followed by `00 00 00 00` and then the unit count.
- **Briefing dummy condition** (0x1B): A placeholder condition that only exists in briefing files, not in mission files.

---

## Footer Section

The footer is a counted array of 8-byte entries, NOT a fixed-size block.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | **Footer entry count** |
| +0x04 | 8 × count | Entry[] | 8-byte entries (2 × uint32 each) |

**Observed patterns:**
- **Mission files** (`./ and Live/`): footer count = 0 (no entries)
- **Briefing files** (`Briefing/`): footer count = 3-4 (unit selection slot data)
- **Worldmap files** (`Worldmap/`): footer count = 0 (no entries)

In briefing files, the footer entries define the unit selection slots available to the player before battle. See the Briefing File Structure section for details.

---

## Enums

### UCD (Unit Control Disposition)

Defined in pattern, controls AI behavior.

| Value | Name | Description |
|-------|------|-------------|
| 0 | UCD_PLAYER | Player-controlled |
| 1 | UCD_AI_ENEMY | AI enemy |
| 2 | UCD_AI_ALLY | AI ally |
| 3 | UCD_AI_NEUTRAL | AI neutral |

### Direction

Counter-clockwise from East.

| Value | Name | Direction |
|-------|------|-----------|
| 0 | DIR_EAST | East (Right) |
| 1 | DIR_NE | Northeast |
| 2 | DIR_NORTH | North (Up) |
| 3 | DIR_NW | Northwest |
| 4 | DIR_WEST | West (Left) |
| 5 | DIR_SW | Southwest |
| 6 | DIR_SOUTH | South (Down) |
| 7 | DIR_SE | Southeast |

### JobType (Unit Types)

From `K2JobDef.h` - maps to Animation ID / unit type fields. Values 0-42 correspond to the `K2_JOB_TYPE` enum. Values >= 43 (`JOB_LAST`) are **CharInfo indices** referencing hero/named character records in `CharInfo.sox`. The game resolves their base combat type from the CharInfo record's internal job type field.

| Value | Enum | Unit Type |
|-------|------|-----------|
| 0 | JOB_H_ARCHER | Human Archer |
| 1 | JOB_H_LONGBOW_MAN | Human Longbows |
| 2 | JOB_H_INFANTRY | Human Infantry |
| 3 | JOB_H_SPEARMAN | Human Spearman |
| 4 | JOB_H_H_INFANTRY | Human Heavy Infantry |
| 5 | JOB_H_KNIGHT | Human Knight |
| 6 | JOB_H_PALADIN | Human Paladin |
| 7 | JOB_H_CAVALRY | Human Cavalry |
| 8 | JOB_H_H_CAVALRY | Human Heavy Cavalry |
| 9 | JOB_H_STORM_RIDER | Human Storm Riders |
| 10 | JOB_H_SAPPER | Human Sapper |
| 11 | JOB_H_PYRO_TECHNICIAN | Human Pyro Technician |
| 12 | JOB_H_BOMBER_WING | Human Bomber Wing |
| 13 | JOB_H_MORTAR | Human Mortar |
| 14 | JOB_H_BALLISTA | Human Ballista |
| 15 | JOB_H_HARPOON | Human Harpoon |
| 16 | JOB_H_CATAPULT | Human Catapult |
| 17 | JOB_H_BATTALOON | Human Battaloon |
| 18 | JOB_DE_ARCHER | Dark Elf Archer |
| 19 | JOB_DE_CAVALRY_ARCHER | Dark Elf Cavalry Archer |
| 20 | JOB_DE_FIGHTER | Dark Elf Fighter |
| 21 | JOB_DE_KNIGHT | Dark Elf Knight |
| 22 | JOB_DE_LIGHT_CAVALRY | Dark Elf Light Cavalry |
| 23 | JOB_DO_INFANTRY | Dark Orc Infantry |
| 24 | JOB_DO_RIDER | Dark Orc Rider |
| 25 | JOB_DO_H_A_RIDERS | Dark Orc Heavy Armored Riders |
| 26 | JOB_DO_AXE_MAN | Dark Orc Axe Man |
| 27 | JOB_DO_H_A_INFANTRY | Dark Orc Heavy Armored Infantry |
| 28 | JOB_DO_SAPPER | Dark Orc Sapper |
| 29 | JOB_D_SCORPION | Scorpion |
| 30 | JOB_D_SWAMP_MAMMOTH | Swamp Mammoth |
| 31 | JOB_D_DIRIGIBLE | Dirigible |
| 32 | JOB_D_BLACK_WYVERN | Black Wyvern |
| 33 | JOB_DO_GHOUL | Ghoul |
| 34 | JOB_D_BONE_DRAGON | Bone Dragon |
| 35 | JOB_WALL | Wall |
| 36 | JOB_SCOUT | Scout |
| 37 | JOB_SELFDESTRUCTION | Self-Destruction Unit |
| 38 | JOB_ENCABLOSA_MONSTER | Encablosa Monster |
| 39 | JOB_ENCABLOSA_FLYING_MONSTER | Encablosa Flying Monster |
| 40 | JOB_ENCABLOSA_RANGED | Encablosa Ranged |
| 41 | JOB_ELF_WALL | Elf Wall |
| 42 | JOB_ENCABLOSA_LARGE | Encablosa Large |

#### CharInfo Character Types (values >= 43)

Values at or above `JOB_LAST` (43) index directly into `CharInfo.sox` for hero and named character configurations. These entries define the character's base combat type, model, and stats internally. Names below are from `CharInfo_ENG.sox`.

| Value | CharInfo Name | Usage |
|-------|---------------|-------|
| 43 | Lucretia | Dark Elf hero |
| 44 | Leinhart | Dark Legion hero |
| 45 | Elf | Generic elf allied unit |
| 46 | Thomas | Named NPC |
| 47 | Duane | Named NPC |
| 48 | Wizard | Named wizard/caster |
| 49 | Dark Elf | Generic Dark Elf character |
| 50 | Flagman | Human flag bearer |
| 51 | Orc Flagman | Orc flag bearer |
| 52 | Dark Elf Flagman | Dark Elf flag bearer |
| 53 | Dwarf | Dwarf character |
| 54 | Lich | Lich boss/caster |
| 55 | Encablossa | Encablossa entity |
| 56 | Leader | Generic human leader |
| 57 | Orc Leader | Generic orc leader |
| 58 | Dark Elf Leader | Generic dark elf leader |
| 59 | Knight Leader | Knight faction leader |
| 60 | Castle Archer | Castle defense archer |
| 61 | Encablossa | Encablossa variant |
| 62 | Encablossa | Encablossa variant |

### Hero Model/Animation IDs

These byte values map to hero character models and animations in the Model ID field.

| Hex | Decimal | Character |
|-----|---------|-----------|
| 0x20 | 32 | Gerald |
| 0x21 | 33 | Ellen |
| 0x22 | 34 | Regnier |
| 0x23 | 35 | Morene |
| 0x24 | 36 | Rupert |
| 0x25 | 37 | Kendal |
| 0x26 | 38 | Cirith |
| 0x2B | 43 | Lucretia |
| 0x2C | 44 | Leinhart |
| 0x44 | 68 | Walter |

### AbilityType

From `K2AbilityDef.h` - maps to ability slot values. Value 0xFFFFFFFF (-1) means empty slot.

| Value | Enum | Description |
|-------|------|-------------|
| 0 | ABILITY_SCOUT | Scout |
| 1 | ABILITY_LAY_TRAP | Lay Trap (Sapper) |
| 2 | ABILITY_SET_FIRE | Set Fire (Sapper) |
| 3 | ABILITY_LAY_MINE | Lay Mine (Sapper) |
| 4 | ABILITY_REMOVE | Remove (Sapper) |
| 5 | ABILITY_DAM_OPEN | Dam Open (Sapper) |
| 6 | ABILITY_E_TREE_HEAL | Tree Heal (Dark Elf, Earth Elemental) |
| 7 | ABILITY_ENRAGE_HEAL | Enrage Heal (Orc) |
| 8 | ABILITY_FIRE_ARROW | Fire Arrow (Human Archer) |
| 9 | ABILITY_DIRECT_ARROW | Direct Arrow (Human Archer) |
| 10 | ABILITY_POUR_OIL | Pour Oil (Human Sapper) |
| 11 | ABILITY_BLESS_HEAL | Bless Heal (Paladin) |
| 12 | ABILITY_WORSHIP_EXPLOSION | Worship Explosion (Paladin) |
| 13 | ABILITY_FIREPOT | Firepot (Catapult) |
| 14 | ABILITY_FIREPOT_DIRIGIBLE | Firepot (Dirigible) |
| 15 | ABILITY_SHOCKWAVE | Shockwave (Scorpion) |
| 16 | ABILITY_ELEMENTAL_BOOST | Elemental Boost (Dark Elf) |
| 17 | ABILITY_SELF_DESTRUCTION | Self-Destruction (Ghoul) |
| 18-20 | ABILITY_H_* | Holy elemental abilities |
| 21-23 | ABILITY_C_* | Curse elemental abilities |
| 24-26 | ABILITY_F_* | Fire elemental abilities |
| 27-29 | ABILITY_L_* | Lightning elemental abilities |
| 30-32 | ABILITY_I_* | Ice elemental abilities |
| 33-35 | ABILITY_E_* | Earth/Poison elemental abilities |
| 36-43 | ABILITY_[HERO] | Hero-specific abilities |
| 44-56 | ABILITY_[RACE]_* | Race-specific combat abilities |

---

## Important Notes

- **Worldmap ID**: Use 0xFF for new units. Reusing existing Worldmap IDs can cause post-mission crashes. See Worldmap ID Behavior below for details.
- **Stat Overrides**: 22 float values at end of unit block can override TroopInfo stats. Set all to -1.0 (0xBF800000) to use defaults.
- **1088-byte blocks**: Player-controlled units have embedded sub-unit (M1/M2) entries for linked infantry.
- **Hero Limitation**: Only Gerald, Lucretia, Regnier, and Kendal have working player-controllable animations.
- **Equipment does not persist**: Equipment set in STG troop blocks is mission-only. The game never carries STG equipment over to the barracks/worldmap. Any equipment changes made via STG modding will only apply during that mission.

### Worldmap ID Behavior

The Worldmap ID field (offset 0x56 in the leader configuration) controls how the game loads and saves unit state relative to the worldmap:

- **0xFF**: Prevents the mission from loading any worldmap state. The unit is treated as entirely new. This is the safe default for custom units.
- **Any other value**: The game loads the corresponding worldmap entry and overwrites the STG unit data with whatever the worldmap save contains. This means the unit's type, level, equipment, and officers will be replaced by the saved worldmap state.

After a mission completes, the game saves the unit back to the worldmap entry referenced by this ID. Using an existing Worldmap ID on a different unit type can cause crashes or data corruption.

### Worldmap File Limitations (Crusaders only)

- Worldmap STG files (`Mission/Worldmap/`) cannot use the action triggers that mission files use.
- Post-game triggers (on campaign completion + forced save) only fire once.
- There is no way to reset triggers or force the game to re-read worldmap files from the worldmap context.

