~FGHB
# Kingdom Under Fire File Formats

Technical specifications for file formats used in Kingdom Under Fire: The Crusaders and Heroes.

## Overview

| Format | Location | Description |
|--------|----------|-------------|
| `.sox` (binary) | `Data/SOX/` | Game data (troops, skills, items, exp tables) |
| `.sox` (text) | `Data/SOX/ENG/` | Localized text with fixed-width fields |
| `.stg` | `Data/Mission/` | Mission configuration and unit deployment |
| `.nav` | `Data/Mission/` | AI navigation/pathfinding data |
| `.K2A` | Various | 3D model archives (poorly documented) |
| Save files | `Documents/KUF2 {Game}/` | Player save data |

All numeric values are **little-endian**. Floats are **32-bit IEEE 754**.

### Directory Structure

| Directory | Contents |
|-----------|----------|
| `Data/AI/` | AI behavior files (.txt) |
| `Data/Camera/` | Camera angles (.cam) |
| `Data/FX/` | Visual effects (fire, explosions, etc.) |
| `Data/Interface/` | Menus, icons, UI elements |
| `Data/Map/` | Tilemaps, worldmap city/mission placement, props (trees, walls)

| `Data/Set/` | Mission lighting and color settings |
| `Data/Sky/` | Sky textures and settings |
| `Data/SOX/` | Game stats and data (.sox) |
| `Data/Text/` | Text/localization (except officer names) |
| `Data/Units/` | Unit assets: `.saf` (animations), `.smfx` (models), `.k2a` (animation tables), `.dds` (textures) |

---

## Binary SOX Files

Located in `Data/SOX/`. Contains game configuration data.

### General Structure

```
Header (8 bytes)
├── Version: int32 (typically 100)
└── Count: int32 (number of records)

Records (Count × RecordSize bytes)
└── Fixed-size structures, format varies by file

Footer
└── Optional padding (e.g., 64 bytes for TroopInfo.sox)
```

### TroopInfo.sox

**Note**: The structure below is verified for Crusaders. Heroes may have different troop entries.

**Header**: Version=100, Count=43

**TroopInfo Structure** (148 bytes per troop):

| Offset | Type | Field | Description |
|--------|------|-------|-------------|
| 0x00 | int32 | Job | Troop job type (K2JobDef.h) |
| 0x04 | int32 | TypeID | Troop type ID (K2TroopDef.h) |
| 0x08 | float | MoveSpeed | Max move speed |
| 0x0C | float | RotateRate | Max rotate rate |
| 0x10 | float | MoveAcceleration | Move acceleration |
| 0x14 | float | MoveDeceleration | Move deceleration |
| 0x18 | float | SightRange | Visible range |
| 0x1C | float | AttackRangeMax | Maximum attack range |
| 0x20 | float | AttackRangeMin | Ranged attack range (0 if no ranged) |
| 0x24 | float | AttackFrontRange | Frontal attack range (0 if no frontal) |
| 0x28 | float | DirectAttack | Direct attack strength (melee/frontal) |
| 0x2C | float | IndirectAttack | Indirect attack strength (ranged) |
| 0x30 | float | Defense | Defense strength |
| 0x34 | float | BaseWidth | Base troop size |
| 0x38 | float | ResistMelee | Melee resistance |
| 0x3C | float | ResistRanged | Ranged resistance |
| 0x40 | float | ResistFrontal | Frontal resistance |
| 0x44 | float | ResistExplosion | Explosion resistance |
| 0x48 | float | ResistFire | Fire resistance |
| 0x4C | float | ResistIce | Ice resistance |
| 0x50 | float | ResistLightning | Lightning resistance |
| 0x54 | float | ResistHoly | Holy resistance |
| 0x58 | float | ResistCurse | Curse resistance |
| 0x5C | float | ResistPoison | Poison resistance |
| 0x60 | float | MaxUnitSpeedMultiplier | Unit speed multiplier |
| 0x64 | float | DefaultUnitHP | Default HP per unit |
| 0x68 | int32 | FormationRandom | Formation randomization |
| 0x6C | int32 | DefaultUnitNumX | Units per row |
| 0x70 | int32 | DefaultUnitNumY | Number of rows |
| 0x74 | float | UnitHPLevUp | HP gain per level |
| 0x78 | LevelUpData[3] | LevelUpData | 3 skill slots (8 bytes each) |
| 0x90 | float | DamageDistribution | Damage distribution factor |

**LevelUpData Structure** (8 bytes):

| Offset | Type | Field |
|--------|------|-------|
| 0x00 | int32 | SkillID |
| 0x04 | float | SkillPerLevel |

**Resistance Values**: 1.0 = baseline (0% resistance), 0.0 = 100% resistance, 2.0 = -100% resistance (vulnerability)

**Footer**: 64 bytes padding

#### Troop Names (Index Order) - Crusaders

| Index | Name |
|-------|------|
| 0 | Archer |
| 1 | Longbows |
| 2 | Infantry |
| 3 | Spearman |
| 4 | Heavy Infantry |
| 5 | Knight |
| 6 | Paladin |
| 7 | Cavalry |
| 8 | Heavy Cavalry |
| 9 | Storm Riders |
| 10 | Sappers |
| 11 | Pyro Techs |
| 12 | Bomber Wings |
| 13 | Mortar |
| 14 | Ballista |
| 15 | Harpoon |
| 16 | Catapult |
| 17 | Battaloon |
| 18 | Dark Elves Archer |
| 19 | Dark Elves Cavalry Archers |
| 20 | Dark Elves Infantry |
| 21 | Dark Elves Knights |
| 22 | Dark Elves Cavalry |
| 23 | Orc Infantry |
| 24 | Orc Riders |
| 25 | Orc Heavy Riders |
| 26 | Orc Axe Man |
| 27 | Orc Heavy Infantry |
| 28 | Orc Sappers |
| 29 | Orc Scorpion |
| 30 | Orc Swamp Mammoth |
| 31 | Orc Dirigible |
| 32 | Orc Black Wyverns |
| 33 | Orc Ghouls |
| 34 | Orc Bone Dragon |
| 35 | Wall Archers (Humans) |
| 36 | Scouts |
| 37 | Ghoul Selfdestruct |
| 38 | Encablossa Monster (Melee) |
| 39 | Encablossa Flying Monster |
| 40 | Encablossa Monster (Ranged) |
| 41 | Wall Archers (Elves) |
| 42 | Encablossa Main |

#### Default Range Values (AttackRangeMax)

| Unit | Hex (LE) | Decimal |
|------|----------|---------|
| Battaloon | 0x00000BB8 | 3000 |
| Dark Elf Archers | 0x00001770 | 6000 |
| Dark Elf Cavalry Archers | 0x00001770 | 6000 |
| Bone Dragon | 0x00001770 | 6000 |
| Archer | 0x00001B58 | 7000 |
| Swamp Mammoth | 0x00001D4C | 7500 |
| Longbow | 0x00001F40 | 8000 |
| Mortar | 0x00002710 | 10000 |
| Ballista | 0x00002EE0 | 12000 |
| Catapult | 0x00002EE0 | 12000 |

### Known Binary SOX Files

| File | Description | Status |
|------|-------------|--------|
| `TroopInfo.sox` | Unit stats, resistances, formations | Implemented |
| `SkillInfo.sox` | Skill definitions and level caps | Not implemented |
| `ExpInfo.sox` | Experience tables (205,194 total XP for 1-50) | Not implemented |
| `UnitUVID.sox` | Unit UV mapping IDs | Not implemented |
| `UnitUVInfo.sox` | Unit UV mapping info | Not implemented |

---

## Text SOX Files

Located in `Data/SOX/ENG/` (or other language codes). Fixed-width text format.

### Structure

Each text entry has a **byte-length prefix** indicating maximum characters:
- `0x0B` (11) = 11 characters max
- `0x0C` (12) = 12 characters max
- `0x0F` (15) = 15 characters max

### Editing Rules

1. Text length must match the defined field width exactly
2. Pad shorter strings with spaces or null bytes (`0x00`)
3. If length doesn't match, game enters infinite error loop
4. Use hex editor to modify length prefix when needed

### Known Text SOX Files

| File | Description |
|------|-------------|
| `ItemTypeInfo_ENG.sox` | Item/equipment names and descriptions |

---

## Mission Files (.stg)

Located in `Data/Mission/` (missions) and `Data/Mission/Briefing/` (briefings).

### File Structure

```
Header
├── Mission metadata
└── Troop count (1 byte, precedes first troop block)

Troop Blocks (repeated for each unit)
├── Internal name (32 bytes, ASCII, null-terminated)
├── Unique ID (1 byte) - must be unique in file
├── UCD - Unit Category Data (4 bytes Crusaders, 1 byte Heroes)
├── UAD - Unit Allegiance Data
├── Flags
│   ├── IsHero (1 byte, 0x00=No, other=Yes)
│   └── IsEnabled (1 byte)
├── HP Overrides
│   ├── LeaderHP (float, -1 = no override)
│   └── UnitHP (float)
├── Leader Data
│   ├── AnimationID (1 byte)
│   ├── ModelID (1 byte)
│   ├── WorldmapID (1 byte, 0xFF for non-story units)
│   └── Level (1 byte, hex; 0x63 = level 99)
├── Skills (4 slots)
│   ├── SkillID (1 byte)
│   └── SkillLevel (1 byte)
├── Officers (up to 2, same structure as Leader)
├── Unit Troop Data
│   ├── AnimationID (1 byte)
│   ├── ModelID (1 byte)
│   ├── UnitX (1 byte)
│   ├── UnitY (1 byte) - total units = X × Y
│   ├── TroopInfo/Job (1 byte)
│   └── Formation (1 byte)
├── Position
│   ├── X (float)
│   ├── Y (float)
│   └── Facing (1 byte, 0x00=Right, counter-clockwise)
├── Flag Visuals
│   ├── FlagBearerModel (1 byte)
│   └── FlagModel (1 byte)
└── SP - Skill Points (float)

Extra Stats Block (optional, 88 bytes = 22 floats, identified by 0x80BF marker)
├── 1. MovementSpeed
├── 2. RotationRate
├── 3. MoveAcceleration
├── 4. MoveDeceleration
├── 5. SightRange
├── 6. AttackRangeMax
├── 7. AttackRangeMin
├── 8. FrontRange (spearman, axe thrower frontal)
├── 9. DirectDamage (melee combat)
├── 10. IndirectDamage (ranged)
├── 11. Defense
├── 12. Width
├── 13. MeleeResistance
├── 14. RangedResistance
├── 15. ExplosionResistance
├── 16. FrontalResistance
├── 17. FireResistance
├── 18. IceResistance
├── 19. LightningResistance
├── 20. HolyResistance
├── 21. CurseResistance
└── 22. PoisonResistance

Briefing Section (at file end, for briefing files)
├── Unit count (1 byte)
└── Per-unit entries (8 bytes each)
    ├── Byte 0: Unique ID
    ├── Byte 2: Special ID
    ├── Byte 3: Special Icon
    ├── Byte 6: Support flag (0x00/0x01)
    └── Byte 7: Locked flag (0x00=locked, 0x01=unlocked)
```

### Reference Tables

#### Unit Category Data (UCD)

| Value | Meaning |
|-------|---------|
| 0x00 | Not Used |
| 0x01 | Local (Player-controlled) |
| 0x02 | Remote (Multiplayer) |
| 0x03 | AI (Enemy) |
| 0x04 | AI Friendly |
| 0x05 | AI Neutral (unused) |

#### Unit Allegiance Data (UAD)

Relative to player (assuming player UAD=0):

| Value | Meaning |
|-------|---------|
| 0x00 | Ally |
| 0x01 | Enemy |
| 0x02 | Enemy of everyone |

#### Animation IDs (0x00-0x44)

| ID | Unit Type |
|----|-----------|
| 0x01-0x0A | Infantry types |
| 0x19-0x1F | Orc types |
| 0x20 | Gerald |
| 0x22 | Regnier |
| 0x2B | Lucretia |
| 0x2C | Leinhart |

#### Skill IDs (0x00-0x0D)

| ID | Skill |
|----|-------|
| 0x00 | Melee |
| 0x01 | Range |
| 0x02 | Frontal |
| 0x03 | Riding |
| 0x04 | Teamwork |
| 0x05 | Scouting |
| 0x06 | Gunpowder |
| 0x07 | Taming |
| 0x08 | Fire |
| 0x09 | Ice |
| 0x0A | Lightning |
| 0x0B | Holy |
| 0x0C | Earth |
| 0x0D | Curse |

#### TroopInfo/Job IDs (0x00-0x37)

Unit class types (Archer, Infantry, Cavalry, etc.)

#### Formation IDs (0x00-0x28)

| Range | Type |
|-------|------|
| Infantry | Standard infantry formations |
| Archer | Ranged unit formations |
| Mounted | Cavalry formations |
| Siege | Siege weapon formations |
| Dummy | Placeholder/special formations |

#### Flag Bearer Models

| ID | Race |
|----|------|
| 0x00 | Human |
| 0x01 | Orc |
| 0x02 | Dark Elves |

#### Flag Models

| ID | Faction |
|----|---------|
| 0x00 | Hironeiden |
| 0x01 | Hexter |
| 0x02 | Vellond |
| 0x03 | Ecclesia |

### AI Files

Each mission references an AI file (e.g., `AI2020.txt` for Lucretia's second mission). The reference is at the beginning of the .stg file. AI file names are in Korean - use Notepad++ to view properly.

To add AI behavior for a new troop, you must add it to the Mission Part data.

### Mission Parts (Scripting/Triggers)

Located after the last Troop Block. Defines mission events, conditions, and actions.

```
Mission Parts Section
├── Num Mission Parts (1 byte) - first byte after last troop
└── Mission Part Blocks (repeated)
    ├── Name + Description (64 bytes, ASCII)
    ├── Num Block (4 bytes, int32)
    ├── Num Conditions (4 bytes, int32)
    ├── Conditions (variable length)
    ├── Num Acts (4 bytes, int32)
    └── Acts (variable length)
```

**Handles**: Cutscenes, spawn timing (when to spawn, not position), respawn, AI IDs. Generally everything that happens in a mission not defined externally (stats are in TroopInfo.sox, positions in Troop Block).

#### Condition/Action Length

The byte length of each Condition or Action depends on its type, as defined in `Mission.h`. For example:
- Compare int: `[ConditionType][VarID][CompareValue]`
- Add SP: `[ActionType=0x11][NumSubs=0x02][0x00][PlayerID][0x00][SP as int32]`

Action types reference specific operations (add SP, spawn unit, trigger dialogue, etc.). The parameter structure varies per action.

### Known Issues

**Crash-prone units**: Dark elf archers, orc axemen, and pyrotechs can crash the game if they are in combat immediately after a cutscene. Likely an animation or engine issue.

**80BF stat overflow**: The Extra Stats Block values can overflow to negative, causing instant unit death. Be careful when editing.

---

## Navigation Files (.nav)

Located in `Data/Mission/` (e.g., `1050.nav`).

Contains AI pathfinding and navigation mesh data. Format not well documented.

---

## 3D Model Files (.K2A)

Proprietary archive format containing:
- Vertices and mesh data
- Texture references
- LOD (Level of Detail) variants

**Status**: Poorly documented. 3D Object Converter may support some variants but Kingdom Under Fire K2A files often fail to load. Models may appear degraded after extraction due to LOD optimization.

---

## Save Game Files

Located in `Documents/KUF2 Crusaders/` or `Documents/KUF2 Heroes/`.

### Troop Data Structure (per troop)

| Field | Description |
|-------|-------------|
| Hero Barracks UI | Display in barracks menu |
| Hero In-Game | In-game model reference |
| Leader Model Variation | Leader appearance variant |
| Troop Job | Unit class/job type |
| Troop Model Variation(s) | Unit appearance variants |
| UnitX | Units per row |
| UnitY | Number of rows |

### Job Tree

**Job Tree Unlocked**: Boolean byte
- `0x00` = Locked
- `0x01` = Unlocked

Savegame job tree unlocking is easier than via .stg files.

### Known Offsets (Gerald example)

| Offset | Data |
|--------|------|
| 0x05B0 | Troop data start, Unit item ID |
| 0x05C0 | Unit job |
| 0x05D0 | Troop level |
| 0x05E0 | Skill data |
| 0x05F0 | Troop data end |

### Troop Type Hex Values

Range 0x01-0x3F for different unit types:
- 0x0D = Paladin
- 0x18 = Bone Dragon

---

## Game Differences: Crusaders vs Heroes game differences:

| Feature | Crusaders | Heroes |
|---------|-----------|--------|
| UCD size | 4 bytes | 1 byte |
| Barracks SP | N/A | Overrides .stg SP |
| Skill caps | 50 (25 for elementals) | Same |

---

## Sources

- [Steam: .stg Guide - Advanced (Heroes)](https://steamcommunity.com/sharedfiles/filedetails/?id=2154228946)
- [Steam: .stg Guide - Beginner (Crusaders)](https://steamcommunity.com/sharedfiles/filedetails/?id=3033606561)
- [Steam: Mod Collection - Crusaders](https://steamcommunity.com/sharedfiles/filedetails/?id=2063179503)
- [Steam: Mod Collection - Heroes](https://steamcommunity.com/sharedfiles/filedetails/?id=2146867909)
- [Steam: Savegame Modding Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2089344971)
- [SteamAH: Text Editing Guide](https://steamah.com/kingdom-under-fire-the-crusaders-editing-text-guide/)
- [Steam: Mod Support Discussion](https://steamcommunity.com/app/1121420/discussions/0/1746772087800224057/?ctp=7)
