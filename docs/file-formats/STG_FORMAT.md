# STG Format Documentation

STG files define unit placement and configuration for missions in Kingdom Under Fire: Crusaders (PC port).

## File Locations

- `Mission/` - Main mission STG files (e.g., `E1001.stg`)
- `Mission/Briefing/` - Pre-battle briefing STG files
- `Mission/Worldmap/` - World map unit data

## Related Files

- `K2JobDef.h` - Unit/job type enum definitions (maps to Animation ID)
- `K2AbilityDef.h` - Ability type enum definitions
- `KufParticle_Def.h` - Particle effect IDs
- `CruMission.h` - Mission scripting conditions and actions
- `Text/KufReportMessage_Def.h` - Battle report message types
- `kuf_crusaders_stg.hexpat` - ImHex pattern for parsing STG files
- `MISSION_SCRIPTING.md` - Documentation for event conditions and actions

---

## Data Flow

STG files work together with external data files:

1. **TroopInfo.sox** provides base/default stats for all troop types (attack, defense, resistances, etc.)
2. **STG Troop Blocks** define unit positions and can override specific stats per-mission
3. **Mission Part Data** (after last troop block) handles runtime events: cutscenes, spawn timing, respawn, AI IDs

### Stat Priority

**STG takes priority** over TroopInfo.sox for most stats:
- NumX/NumY (formation dimensions)
- HP, Defense (e.g., tanky Hugh vs Regnier fight)
- Attack range and other combat stats

**TroopInfo.sox takes priority** only when:
- Changing jobs - TroopInfo.sox becomes the primary source for NumX/NumY

Stats not overridden in the STG file (set to -1.0) fall back to TroopInfo.sox defaults.

---

## File Structure Overview

| Section | Size | Description |
|---------|------|-------------|
| Header | 628 bytes (0x274) | Mission metadata and filenames |
| Units | 544 bytes × unit_count | Unit definitions and stat overrides |
| Mission Part Data | variable | Cutscenes, spawn timing, respawn, AI IDs |
| Events | 4 + (448 × event_count) | Event/trigger data |
| Footer | 42 bytes | Mission configuration flags |

---

## Header Structure (628 bytes = 0x274)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x000 | 4 | uint32 | Mission ID (e.g., 1001 for E1001) |
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
| 0x270 | 4 | uint32 | **Unit count** |

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
| 0x55 | 1 | uint8 | Model ID variant |
| 0x56 | 1 | uint8 | Worldmap ID (0xFF = new unit, prevents crash) |
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
| 0x1C0 | 4 | uint32 | **TroopInfo index** (references TroopInfo.sox) |
| 0x1C4 | 4 | uint32 | Formation type |
| 0x1C8 | 88 | float[22] | Stat overrides (all -1.0 = use defaults) |

---

## Mission Part Data

Located after the last troop block. Handles runtime mission behavior that isn't defined externally.

| Data Type | Description |
|-----------|-------------|
| Cutscenes | In-mission cinematics and camera events |
| Spawn Data | When to spawn units (timing), not where (positions are in Troop Blocks) |
| Respawn | Unit respawn configuration |
| AI IDs | AI behavior assignments for units |

**Note:** This section is distinct from stat data (TroopInfo.sox) and position data (Troop Blocks).

---

## Event Section

Follows after Mission Part Data. Events use condition/action IDs defined in `CruMission.h` (see `MISSION_SCRIPTING.md` for full documentation).

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Event count |
| +0x04 | 448 × count | EventEntry[] | Event entries |

### Event Entry (448 bytes)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 64 | char[64] | Event description (editable label for identification) |
| 0x40 | 4 | uint32 | Unique block ID (must be unique per mission) |
| 0x44 | 4 | uint32 | NumConditions |
| 0x48 | varies | Condition[] | Condition entries (ID + parameters each) |
| ... | 4 | uint32 | NumActions |
| ... | varies | Action[] | Action entries (ID + parameters each) |

**Note:** The first event block in many missions is a leftover debug block. Use text linking to find the actual starting block.

### Condition Entry Format

Each condition consists of:
1. **Condition ID** (uint32) - Maps to `CON_*` enum in CruMission.h (stored as hex, convert to decimal)
2. **Parameters** - Variable count based on condition type

Example: `CON_VAR_INT_COMPARE` (ID 19 = 0x13)
- Parameters: `[VariableID, Int, Compare]`
- Checks: if variable equals/compares to int value

### Action Entry Format

Each action consists of:
1. **Action ID** (uint32) - Maps to `ACT_*` enum in CruMission.h (stored as hex, convert to decimal)
2. **Parameters** - Variable count based on action type

Example: `ACT_VAR_INT_SET` (ID 55 = 0x37)
- Parameters: `[VariableID, Int]`
- Effect: Sets variable to int value

### Text Linking

Events can reference text strings by ID from `Data/Text/{LANG}/` files (e.g., `Data/Text/ENG/0000.txt` for mission H0000). To find a specific event block:

1. Start the mission and note the first dialog text
2. Find that text's ID in the corresponding text file
3. Convert the text ID to hex (e.g., 2042 decimal = 0x07FA)
4. Search for the hex value (little-endian: `FA070000`) in the STG file
5. Navigate backwards to find the event block's description field

---

## Footer (42 bytes)

Mission configuration flags at the end of the file.

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

From `K2JobDef.h` - maps to Animation ID / unit type fields.

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

- **Worldmap ID**: Use 0xFF for new units. Reusing existing Worldmap IDs can cause post-mission crashes.
- **Stat Overrides**: 22 float values at end of unit block can override TroopInfo stats. Set all to -1.0 (0xBF800000) to use defaults.
- **1088-byte blocks**: Player-controlled units have embedded sub-unit (M1/M2) entries for linked infantry.
- **Hero Limitation**: Only Gerald, Lucretia, Regnier, and Kendal have working player-controllable animations.
- **ImHex Pattern**: Use `kuf_crusaders_stg.hexpat` for interactive analysis with full enum support.
