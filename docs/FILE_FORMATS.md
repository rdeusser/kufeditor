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

**Header**: Version=100, Count=43

**TroopInfo Structure** (per troop):

| Offset | Type | Field |
|--------|------|-------|
| 0x00 | int32 | Job |
| 0x04 | int32 | TypeID |
| 0x08 | float | MoveSpeed |
| 0x0C | float | RotateRate |
| 0x10 | float | Acceleration |
| 0x14 | float | Deceleration |
| 0x18 | float | DirectAttack |
| 0x1C | float | IndirectAttack |
| 0x20 | float | Defense |
| 0x24 | float | AttackRangeMin |
| 0x28 | float | AttackRangeMax |
| ... | float | Resistances (10 types) |
| ... | int32 | DefaultUnitHP |
| ... | int32 | Formation |
| ... | int32 | UnitCountX, UnitCountY |
| ... | LevelUpData[3] | Skill slots (SkillID + SkillPerLevel) |

**Resistance Types** (in order):
1. Melee
2. Ranged
3. Frontal
4. Explosion
5. Fire
6. Ice
7. Lightning
8. Holy
9. Curse
10. Poison

**Resistance Values**: 1.0 = baseline (0% resistance), 0.0 = 100% resistance, 2.0 = -100% resistance (vulnerability)

**Footer**: 64 bytes padding

### Known Binary SOX Files

| File | Description | Status |
|------|-------------|--------|
| `TroopInfo.sox` | Unit stats, resistances, formations | Implemented |
| `SkillInfo.sox` | Skill definitions and level caps | Not implemented |
| `ExpInfo.sox` | Experience tables (205,194 total XP for 1-50) | Not implemented |

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

Extra Stats Block (optional, 88 bytes = 22 floats)
├── MovementSpeed
├── RotationRate
├── MoveAcceleration
├── MoveDeceleration
├── SightRange
├── AttackRangeMax
├── AttackRangeMin
├── DirectDamage
├── IndirectDamage
├── Defense
├── Width
└── Resistances[11]

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

## Game Differences: Crusaders vs Heroes

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
