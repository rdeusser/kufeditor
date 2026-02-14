# SOX Format Documentation

Data format used by Kingdom Under Fire: Crusaders (PC port) for game data files.

## Encoding

SOX files use **pure binary** little-endian encoding. Each field is stored directly as its native type (uint32, int32, uint16, etc.) with no text encoding layer.

Example: The uint32 value `100` (0x64) is stored as bytes `64 00 00 00` (4 bytes in file).

> **Note:** Earlier community documentation claimed SOX files use ASCII hex encoding (each byte stored as 2 hex characters). This has been **disproven** by Ghidra decompilation of `ReadTroopInfoSOX` (0x005e3a50) in `Kuf2Main.exe`, which calls `_fread(&buffer, 0x94, 1, file)` to read 148 raw bytes per record directly into a stack buffer. The file sizes confirm binary encoding: TroopInfo.sox = 6436 bytes = 8 (header) + 43×148 (records) + 64 (footer).

## Common Structure

- **Byte order**: Little-endian
- **Header**: `uint32` header_val (100) + `uint32` record_count
- **Strings**: `uint16` length prefix + raw bytes (no null terminator)
- **Footer**: `THEND` marker + space padding (0x20) to 64 bytes
- **Special value**: `-1` stored as `0xFFFFFFFF`

## File Variants

1. **Main files** (`AbilityInfo.sox`): Contains gameplay data with localization keys like `@(Scout)`
2. **Localized files** (`AbilityInfo_ENG.sox`): Contains display names and descriptions
3. **LIVE files** (`LIVE_AbilityInfo.sox`): Remnants from Xbox Live multiplayer (identical to main files in PC port)

## File Types Summary

| File | Records | Record Size | Description |
|------|---------|-------------|-------------|
| AbilityInfo.sox | varies | variable | Ability definitions (cooldowns, ranges, damage) |
| AbilityByJob.sox | 35 | 24 bytes | Maps job IDs to available abilities |
| JobInfo.sox | 35 | variable | Job/troop type definitions |
| ItemTypeInfo.sox | 96 | variable | Item/equipment type definitions |
| ItemAttInfo.sox | 23 | 12 bytes | Item attribute/enchantment definitions |
| LeaderGeneration.sox | 19 | 72 bytes | Random leader generation templates |
| TroopInfo.sox | 43 | 148 bytes | Troop/unit type definitions |
| SkillInfo.sox | 15 | variable | Skill definitions |
| SkillPointTable.sox | 100 | 8 bytes | XP progression table |
| CharInfo.sox | 63 | 136 bytes | Character/unit type configuration |
| ResistInfo.sox | 10 | variable | Resistance type definitions |
| UnitUVInfo.sox | 109 | 32 bytes | Unit UV texture mapping data |
| UnitUVID.sox | 61 | 72 bytes | Unit upgrade/variant ID mappings |
| WorldMap_CharInfo.sox | 63 | 28 bytes | World map character configuration |
| WorldMap_TroopInfo.sox | 35 | 28 bytes | World map troop configuration |
| LibraryInfo.sox | 100 | 6 bytes | Library/encyclopedia offset table |
| SpecialNames.sox | 12 | variable | Special character name translations |
| KUF2CustomRandomTable.sox | 10 | 120 bytes | Random generation configuration |
| FontType.sox | 34 | variable | Font style definitions |

---

## ItemTypeInfo.sox Format

**Note**: This file uses format version 2 (not the standard marker 100) and has no THEND footer.

**Header (8 decoded bytes):**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Format version (2) |
| 0x04 | 4 | uint32 | Record count (96) |

**Record Structure (~92 decoded bytes each):**
Records are tightly packed with item name strings followed by stat fields.

| Field | Size | Type | Description |
|-------|------|------|-------------|
| Name | 2+N | uint16 + char[] | Length-prefixed item name |
| Stats | ~84 | uint32[] | Item stat fields |

**Key Stat Fields (after name):**
| Index | Description | Example Values |
|-------|-------------|----------------|
| 0 | Item ID | 1-96 |
| 1 | Buy price | 5000-25000 |
| 2 | Sell price | 1500-12000 |
| 3+ | Combat/stat modifiers | varies |

**Item Types (96 items):**

| Prefix | Class | Faction | Examples |
|--------|-------|---------|----------|
| E_HU_* | Equipment | Human Unit | E_HU_1S (Sword), E_HU_SS (Short Sword) |
| E_HL_* | Equipment | Human Leader | E_HL_BW (Bow), E_HL_ST (Staff) |
| E_HP_* | Equipment | Human Paladin | E_HP_GR (Greatsword) |
| E_HS_* | Equipment | Human Special | E_HS_RP (Rapier), E_HS_IR (Iron) |
| A_HU_* | Armor | Human Unit | A_HU_NR (Normal), A_HU_LI (Light) |
| A_HL_* | Armor | Human Leader | A_HL_MG (Mage), A_HL_EL (Elite) |
| E_DU_* | Equipment | Dark Unit | E_DU_OA (Orc Axe), E_DU_E1S (Elf Sword) |
| E_DL_* | Equipment | Dark Leader | E_DL_SP (Spear), E_DL_RN (Rune) |
| A_DU_* | Armor | Dark Unit | A_DU_ON (Orc Normal), A_DU_EN (Elf Normal) |
| E_DO_* | Equipment | Dark Orc | E_DO_NL (Null) |
| E_DP_* | Equipment | Dark Paladin | E_DP_RN (Rune) |
| E_DS_* | Equipment | Dark Special | E_DS_LH (Light), E_DS_NZ (Nazgul) |
| A_DS_* | Armor | Dark Special | A_DS_UK (Unknown), A_DS_LH (Light) |

**Naming Convention:**
- Format: `X_YY_ZZZ` where:
  - `X` = Class (`E` = Equipment/Weapon, `A` = Armor/Accessory)
  - `YY` = Faction code (see table above)
  - `ZZZ` = Item type suffix (weapon/armor variant)

**Item Tier Names (from ItemTypeInfo_ENG.sox):**

| Tier | Weapons | Armor |
|------|---------|-------|
| 1 | Improved | Brass |
| 2 | Enhanced | Iron |
| 3 | Master's | Steel |

**Equipment Resistances:**

Resistance bonuses on equipment (shields, armor) are stored directly in the item's stat fields within ItemTypeInfo.sox, NOT through the ItemAttInfo attribute system. Currently, equipment only provides **elemental/magic resistances** (Fire, Lightning, Ice, Holy, Poison, Curse). Physical resistances (Melee, Ranged, Explosion, Frontal) appear to only exist on troops via TroopInfo.sox.

---

## ItemAttInfo.sox Format

Item attributes are enchantments/bonuses that can be applied to equipment. **Important:** These are offensive and recovery bonuses only - there are NO resistance attributes in this system.

**Main File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (23) |
| 0x08 | 12×N | records | Fixed 12-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (12 bytes):**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Attribute ID (0-22) |
| 0x04 | 4 | uint32 | Power value (0, 2, 5, 10) |
| 0x08 | 4 | uint32 | Category (0-7) |

**Power Values:**
| Value | Tier | Typical Bonus |
|-------|------|---------------|
| 0 | None/Special | Elemental (Holy) |
| 2 | Low | +20-25% |
| 5 | Medium | +10-15% |
| 10 | High | +10% base |

Note: Power values are **inversely related** to bonus strength. Higher power (10) yields lower bonuses (+10%), while lower power (2) yields higher bonuses (+20-25%). This likely represents rarity or cost rather than attribute strength.

**All 23 Attributes (from ItemAttInfo_ENG.sox):**

| ID | Name | Category | Description |
|----|------|----------|-------------|
| 0 | Iron Will | 0 | HP recovery rate +10% |
| 1 | Steadfast | 0 | HP recovery rate +20% |
| 2 | Plenty | 1 | EXP earned +10% |
| 3 | Abundance | 1 | EXP earned +15% |
| 4 | Wealth | 1 | EXP earned +20% |
| 5 | Desire | 2 | SP earned +10% |
| 6 | Greed | 2 | SP earned +15% |
| 7 | Obsession | 2 | SP earned +20% |
| 8 | Star | 3 | HP +10% |
| 9 | Moon | 3 | HP +15% |
| 10 | Sun | 3 | HP +25% |
| 11 | Iron Will | 4 | HP recovery rate +30% |
| 12 | Steadfast | 4 | HP recovery rate +50% |
| 13 | Durability | 5 | KO Recovery speed +30% |
| 14 | Indomitable | 5 | KO Recovery speed +50% |
| 15 | Defend | 6 | Autoblock rate +10% |
| 16 | Impenetrable | 6 | Autoblock rate +20% |
| 17 | Fire | 7 | Add Fire to Attack |
| 18 | Lightning | 7 | Add Lightning to Attack |
| 19 | Ice | 7 | Add Ice to Attack |
| 20 | Holy | 7 | Add Holy to Attack |
| 21 | Poison | 7 | Add Poison to Attack |
| 22 | Curse | 7 | Add Curse to Attack |

**Categories:**
| ID | Type | Description |
|----|------|-------------|
| 0 | HP Recovery (base) | Passive HP regeneration bonus |
| 1 | EXP Bonus | Experience gain multiplier |
| 2 | SP Bonus | Skill point gain multiplier |
| 3 | HP Bonus | Maximum HP increase |
| 4 | HP Recovery (enhanced) | Higher tier HP regeneration |
| 5 | KO Recovery | Speed of recovering from knockouts |
| 6 | Autoblock | Chance to automatically block attacks |
| 7 | Elemental Attack | **Adds elemental DAMAGE to attacks** (not resistance!) |

**Important Notes:**
- Category 7 (Elemental) adds damage TO your attacks, NOT resistance FROM enemy attacks
- There are NO resistance attributes - resistances come from equipment stats or troop stats
- Physical damage resistances (Melee, Ranged, Explosion, Frontal) are not available as equipment attributes

**Elemental Priority Order (from Discord):**
Fire > Lightning > Ice > Holy > Poison > Curse

When multiple elemental effects could apply, this determines which takes precedence. Vanilla files have a 15% chance to apply elemental effects on hit.

---

## LeaderGeneration.sox Format

Defines templates for randomly generating leader/hero characters.

**Main File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (19) |
| 0x08 | 72×N | records | Fixed 72-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (72 bytes = 18 uint32 fields):**
| Field | Type | Description |
|-------|------|-------------|
| 0 | uint32 | Leader template ID (references name pool) |
| 1 | uint32 | Faction type (0=Human, 1=Dark, 2=Mixed) |
| 2 | uint32 | Base stat value (0, 5, or 10) |
| 3 | uint32 | Stat modifier (0-10) |
| 4 | uint32 | Attribute 1 (1-6) |
| 5 | uint32 | Attribute 2 (1-5) |
| 6 | uint32 | Attribute 3 (6-12) |
| 7 | uint32 | Reserved (always 0) |
| 8-17 | uint32[10] | Available ability/equipment slot IDs (-1 = unused) |

**Localized File Structure:**
Each record contains an ID followed by a space-separated list of possible names for random selection.

**Base Stat Tiers:**
| Base | Tier | Characteristics |
|------|------|-----------------|
| 10 | High | More equipment slots, higher stat modifiers (mod 2-10) |
| 5 | Medium | Moderate equipment slots, lower modifiers (mod 0-1) |
| 0 | Low | Fewer slots, no stat modifier, fixed single names |

**Faction Distribution:**
| Faction | Count | Example Names |
|---------|-------|---------------|
| Human (0) | 10 | Mattew, Andrew, Gabriel, Edward, Ellen, Rupert |
| Dark (1) | 5 | Vrark, Parak, Leinhart, Urukubarr |
| Mixed (2) | 4 | Endith, Rothhaal, Morene, Cirith |

---

## TroopInfo.sox Format

Defines all troop/unit types with their combat statistics.

> **⚠️ CRITICAL BUG (PC Port Patch 4+):** Resistance values are INVERTED in patch 4 and later. Positive resistance values now INCREASE damage taken instead of reducing it. This is a known bug in the PC port. To get correct behavior, use Steam depot with Patch 3: `download_depot 1121420 1121421 4395673366137241035`

> **✓ VERIFIED FROM BINARY:** The field layout, types, divisor constants, and resistance field ordering below have been confirmed by Ghidra decompilation of `ReadTroopInfoSOX` at `0x005e3a50` in `Kuf2Main.exe`. The resistance field swap pattern (SOX file order → in-memory ResistInfo.sox ID order) is confirmed in the binary.

**Main File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (43) |
| 0x08 | 148×N | records | Fixed 148-byte records (37 × int32) |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Binary Reading Process** (from `ReadTroopInfoSOX` decompilation):

1. `OpenSOXFile` (0x005e4870) opens `Data\SOX\troopinfo.sox` with `fopen("rb")`
2. Reads 4-byte header marker (100), then 4-byte record count (43)
3. Allocates three separate in-memory arrays per record:
   - Array0: 0x60 (96) bytes — combat stats and resistances
   - Array1: 0x14 (20) bytes — unit properties and name pointer
   - Array2: 0x1c (28) bytes — level-up data
4. Per record: bulk-reads 148 bytes via `_fread(&buffer, 0x94, 1, file)` — all 37 fields are **int32** in the file
5. Distributes fields across the three arrays, applying type conversions (int→float casts and divisor scaling)
6. After all records: opens localized file (e.g., `Data\SOX\ENG\ENG_troopinfo.sox`) via `OpenLocalizedSOXFile` (0x005e4930)
7. Localized file per-record: 4-byte record index + 2-byte string length + string bytes → stored as null-terminated string pointer at Array1 offset 0x10

**Divisor Constants** (confirmed from `.rdata` section of `Kuf2Main.exe`):

| Address | Hex Value | Float | Applies To |
|---------|-----------|-------|------------|
| `DAT_006bba64` | `0x41200000` | **10.0** | MoveSpeed, MoveAccel, MoveDecel, MaxUnitSpeedMult, LevelUp bonuses |
| `DAT_006bc014` | `0x447a0000` | **1000.0** | RotateRate only |
| `DAT_006bb6dc` | `0x42c80000` | **100.0** | All 10 resistance fields, DamageDistribution |

**Record Structure (148 bytes = 37 × int32 in file):**

All fields are stored as **int32** in the SOX file. The game converts them during loading as noted in the Transform column.

| Field | Name | File Type | Transform | Description |
|-------|------|-----------|-----------|-------------|
| 0 | Job | int32 | direct | Troop job type (K2JobDef.h) |
| 1 | TypeID | int32 | direct | Troop type ID (K2TroopDef.h) |
| 2 | MoveSpeed | int32 | ÷ 10.0 → float | Max move speed (e.g., 130 → 13.0) |
| 3 | RotateRate | int32 | ÷ 1000.0 → float | Max rotate rate (e.g., 50 → 0.05) |
| 4 | MoveAcceleration | int32 | ÷ 10.0 → float | Move acceleration (e.g., 150 → 15.0) |
| 5 | MoveDeceleration | int32 | ÷ 10.0 → float | Move deceleration (e.g., 150 → 15.0) |
| 6 | SightRange | int32 | cast → float | Visible range (e.g., 2000 → 2000.0) |
| 7 | AttackRangeMax | int32 | cast → float | Max attack range |
| 8 | AttackRangeMin | int32 | cast → float | Min attack range (0 = melee only) |
| 9 | AttackFrontRange | int32 | cast → float | Frontal attack range |
| 10 | DirectAttack | int32 | cast → float | Melee/frontal attack strength |
| 11 | IndirectAttack | int32 | cast → float | Ranged attack strength |
| 12 | Defense | int32 | cast → float | Defense strength |
| 13 | BaseWidth | int32 | cast → float | Base troop size |
| 14 | ResistMelee | int32 | ÷ 100.0 → float | Melee damage multiplier (100 = 1.0×) |
| 15 | ResistRanged | int32 | ÷ 100.0 → float | Ranged damage multiplier |
| 16 | ResistFrontal | int32 | ÷ 100.0 → float | Frontal damage multiplier |
| 17 | ResistExplosion | int32 | ÷ 100.0 → float | Explosion damage multiplier |
| 18 | ResistFire | int32 | ÷ 100.0 → float | Fire damage multiplier |
| 19 | ResistIce | int32 | ÷ 100.0 → float | Ice damage multiplier |
| 20 | ResistLightning | int32 | ÷ 100.0 → float | Lightning damage multiplier |
| 21 | ResistHoly | int32 | ÷ 100.0 → float | Holy damage multiplier |
| 22 | ResistCurse | int32 | ÷ 100.0 → float | Curse damage multiplier |
| 23 | ResistEarth | int32 | ÷ 100.0 → float | Earth/Poison damage multiplier |
| 24 | MaxUnitSpeedMultiplier | int32 | ÷ 10.0 → float | Unit speed multiplier cap (e.g., 15 → 1.5) |
| 25 | DefaultUnitHP | int32 | cast → float | Default HP per unit |
| 26 | FormationRandom | int32 | truncate → byte | Formation randomness |
| 27 | DefaultUnitNumX | int32 | truncate → byte | Formation width |
| 28 | DefaultUnitNumY | int32 | truncate → byte | Formation depth |
| 29 | UnitHPLevUp | int32 | cast → float | HP gain per level |
| 30 | LevelUpData[0].skillID | int32 | direct | Skill ID (-1 = none) |
| 31 | LevelUpData[0].bonus | int32 | ÷ 10.0 → float | Per-level bonus (e.g., 2 → 0.2) |
| 32 | LevelUpData[1].skillID | int32 | direct | Skill ID (-1 = none) |
| 33 | LevelUpData[1].bonus | int32 | ÷ 10.0 → float | Per-level bonus |
| 34 | LevelUpData[2].skillID | int32 | direct | Skill ID (-1 = none) |
| 35 | LevelUpData[2].bonus | int32 | ÷ 10.0 → float | Per-level bonus |
| 36 | DamageDistribution | int32 | ÷ 100.0 → float | Damage distribution factor |

**Resistance Fields (14-23) — File Order vs Memory Order:**

The SOX file stores resistance values in a different order than how they are arranged in memory. The game swaps consecutive pairs during loading to align with `ResistInfo.sox` ID order. This is confirmed in the decompiled binary where fields are read sequentially but written to non-sequential memory offsets.

| SOX File Field | SOX File Name | → Memory Offset | Memory Name (ResistInfo ID) |
|----------------|---------------|-----------------|----------------------------|
| 14 | ResistMelee | 0x38 | ResistMelee (ID 0) |
| 15 | ResistRanged | 0x3c | ResistRanged (ID 1) |
| **16** | **ResistFrontal** | **0x44** | ResistFrontal (ID 3) — **swapped** |
| **17** | **ResistExplosion** | **0x40** | ResistExplosion (ID 2) — **swapped** |
| 18 | ResistFire | 0x48 | ResistFire (ID 4) |
| **19** | **ResistIce** | **0x50** | ResistIce (ID 6) — **swapped** |
| **20** | **ResistLightning** | **0x4c** | ResistLightning (ID 5) — **swapped** |
| 21 | ResistHoly | 0x54 | ResistHoly (ID 7) |
| **22** | **ResistCurse** | **0x5c** | ResistCurse (ID 9) — **swapped** |
| **23** | **ResistEarth** | **0x58** | ResistPoison/Earth (ID 8) — **swapped** |

The pattern: pairs (16,17), (19,20), (22,23) are swapped during loading. The SOX file order is: Melee, Ranged, **Frontal, Explosion**, Fire, **Ice, Lightning**, Holy, **Curse, Poison**. The in-memory order is: Melee, Ranged, **Explosion, Frontal**, Fire, **Lightning, Ice**, Holy, **Poison, Curse**.

**Damage Multiplier Values (fields 14-23):**

Values are divided by 100.0 to produce a damage multiplier in memory. Higher values = more damage taken.

| File Value | In-Memory Float | Meaning |
|------------|-----------------|---------|
| 0 | 0.0 | Immune (0% damage) |
| 50 | 0.5 | Resistant (50% damage) |
| 100 | 1.0 | Normal (100% damage) |
| 150 | 1.5 | Vulnerable (150% damage) |
| 200 | 2.0 | Very vulnerable (200% damage) |
| 250 | 2.5 | Extremely vulnerable (250% damage) |
| 1000000+ | 10000.0+ | Instant death (recon units) |

**Example - Flying Units (Storm Riders, Black Wyverns):**
- Melee: file=100 → 1.0× (normal — ground immunity is engine-enforced)
- Ranged: file=150 → 1.5× (vulnerable — archers can shoot them down)
- Frontal: file=100 → 1.0× (normal)
- Explosion: file=200 → 2.0× (very vulnerable — anti-air siege weapons)
- All elemental: file=100 → 1.0× (normal)

**Damage Type Mapping:**
| Field | Type | Notes |
|-------|------|-------|
| 14 | Melee | Direct melee attacks |
| 15 | Ranged | Arrow/projectile attacks |
| 16 | Frontal | Frontal charge attacks |
| 17 | Explosion | Explosive damage (catapults, Ghoul Selfdestruct) |
| 18 | Fire | Fire elemental |
| 19 | Ice | Ice elemental |
| 20 | Lightning | Lightning elemental |
| 21 | Holy | Holy damage (Ghouls vulnerable) |
| 22 | Curse | Curse damage (Ghouls immune) |
| 23 | Earth | Earth/Poison damage (Ghouls immune) |

**TypeID Classes (from K2TroopDef.h):**
| Hex | TypeID | Category |
|-----|--------|----------|
| 0x00 | 0 | MELEE |
| 0x01 | 1 | ARCHER |
| 0x02 | 2 | SPEAR |
| 0x03 | 3 | RIDER |
| 0x04 | 4 | FLYING |
| 0x05 | 5 | SIEGE |
| 0x06 | 6 | AXE |
| 0x07 | 7 | BOMBER |
| 0x08 | 8 | WALL |
| 0x09 | 9 | MORTAR |
| 0x0A | 10 | CAVALRY_ARCHER |
| 0x0B | 11 | SAPPER |
| 0x0C | 12 | SELFDESTRUCTION |
| 0x0D | 13 | ELF_ARCHER |
| 0x0E | 14 | PALADIN |
| 0x0F | 15 | SCORPION |
| 0x20 | 32 | BATTALOON |
| 0x21 | 33 | BALLISTA |
| 0x22 | 34 | SCOUT |
| 0x23 | 35 | SWAMPMAMMOTH |
| 0x24 | 36 | ENCABLOSA_RANGED |
| 0x25 | 37 | ELF_WALL |
| 0x26 | 38 | ENCABLOSA_COLUMN |
| 0x27 | 39 | ENCABLOSA_BIG |

**Skill IDs (for LevelUpData):**
| Hex | Skill |
|-----|-------|
| 0x00 | Melee |
| 0x01 | Range |
| 0x02 | Frontal |
| 0x03 | Riding |
| 0x04 | Teamwork |
| 0x05 | Scouting |
| 0x06 | Gunpowder |
| 0x07 | Taming |
| 0x08 | Fire |
| 0x09 | Lightning |
| 0x0A | Ice |
| 0x0B | Holy |
| 0x0C | Earth |
| 0x0D | Curse |

**Notable Damage Resistances (file values → multipliers):**
| Unit | Normal (100 → 1.0×) | Vulnerabilities (>100) |
|------|----------------------|------------------------|
| Storm Riders | Melee, Frontal, All Magic | Ranged (150 → 1.5×), Explosion (200 → 2.0×) |
| Black Wyverns | Melee, Frontal, All Magic | Ranged (150 → 1.5×), Explosion (200 → 2.0×) |
| Ghoul | Earth (0 → immune), Curse (0 → immune) | Holy (300 → 3.0×), Ice (200 → 2.0×) |
| Infantry | Melee, Frontal, Explosion, Fire, Holy | Ice (250 → 2.5×), Lightning (120 → 1.2×), Curse (120 → 1.2×), Earth (150 → 1.5×), Ranged (80 → 0.8×) |
| Scout/Exp.Ghoul | None | All (1000000+ → instant death) |

**Note:** Flying units (Storm Riders, Wyverns, Dirigibles) cannot be hit by ground melee attacks - this is enforced by the game engine, not resistance values. Their melee resistance in the file is 100 (normal), not 0 (immune).

**In-Memory Resistance Values:**
TroopInfo resistance fields are stored as int32 in the SOX file and converted to float by dividing by 100.0 during loading. The resulting float is a damage multiplier (1.0 = normal, 0.5 = resistant, 2.5 = very vulnerable).

Float resistance values found elsewhere in the game (from Discord):
- `33 33 33 3F` = ~0.7 (70% damage taken = 30% resistance)
- `9A 99 99 3E` = ~0.3 (30% damage taken = 70% resistance)

**Resistance Sources:**
- **Troop resistances**: Defined in TroopInfo.sox (fields 14-23). Includes all 10 damage types.
- **Equipment resistances**: Defined in ItemTypeInfo.sox stat fields. Only elemental/magic resistances available on equipment (shields, armor). Physical resistances (Melee, Ranged, Explosion, Frontal) are NOT available on equipment.

**Troop List (by index):**
0. Archer, 1. Longbows, 2. Infantry, 3. Spearman, 4. Heavy Infantry,
5. Knight, 6. Paladin, 7. Cavalry, 8. Heavy Cavalry, 9. Storm Riders,
10. Sappers, 11. Pyro Techs, 12. Bomber Wings, 13. Mortar, 14. Ballista,
15. Harpoon, 16. Catapult, 17. Battaloon, 18. Dark Elves Archer,
19. Dark Elves Cavalry Archers, 20. Dark Elves Infantry, 21. Dark Elves Knights,
22. Dark Elves Cavalry, 23. Orc Infantry, 24. Orc Riders, 25. Orc Heavy Riders,
26. Orc Axe Man, 27. Orc Heavy Infantry, 28. Orc Sappers, 29. Orc Scorpion,
30. Orc Swamp Mammoth, 31. Orc Dirigible, 32. Orc Black Wyverns, 33. Orc Ghouls,
34. Orc Bone Dragon, 35. Wall Archers (Humans), 36. Scouts, 37. Ghoul Selfdestruct,
38. Encablossa Monster (Melee), 39. Encablossa Flying Monster,
40. Encablossa Monster (Ranged), 41. Wall Archers (Elves), 42. Encablossa Main

**Reference:** [Steam Modding Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2016205613)

---

## SkillInfo.sox Format

Defines all 15 skills available to heroes and troops.

**Main File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (15) |
| 0x08 | var | records | Variable-length records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (variable length):**
| Field | Type | Description |
|-------|------|-------------|
| Skill ID | int32 | 0-13, or -2 (0xFFFFFFFE) for "Any Elemental" |
| Loc Key | uint16 + string | Localization key (e.g., "@(S_Melee)") |
| Icon | uint16 + string | Icon path (e.g., "IL_SKL_Melee.tga") |
| Skill Type | uint32 | 1=Combat, 2=Magic |
| Max Level | uint32 | Maximum skill level |

**All Skills:**
| ID | Name | Type | MaxLv | Slots | Description |
|----|------|------|-------|-------|-------------|
| 0 | Melee | Combat | 50 | 1 | Hand-to-hand damage, best defense bonus |
| 1 | Ranged | Combat | 50 | 1 | Arrow/projectile damage |
| 2 | Frontal | Combat | 50 | 1 | Spearmen, axemen, cavalry charge attacks |
| 3 | Riding | Magic | 50 | 2 | Cavalry/rider turning speed |
| 4 | Teamwork | Combat | 50 | 1 | Sapper speed, siege weapon attack |
| 5 | Scouting | Combat | 3 | 1 | Scout speed and sight range |
| 6 | Gunpowder | Combat | 50 | 1 | Mortar and mine explosive damage |
| 7 | Taming | Combat | 50 | 1 | Beast attack (Dirigibles, Scorpions, Mammoths) |
| 8 | Fire | Magic | 25 | 2 | Meteor spell |
| 9 | Lightning | Magic | 25 | 2 | Thunderstorm spell |
| 10 | Ice | Magic | 25 | 2 | Chilling Touch spell |
| 11 | Holy | Magic | 25 | 2 | Curatio, Bless and Heal spells |
| 12 | Earth | Magic | 25 | 2 | Vine spell (poison damage) |
| 13 | Curse | Magic | 25 | 2 | Darkmist spell |
| -2 | Elemental | - | 25 | - | Special: Any elemental skill |

**Notes:**
- Skill ID 12 is "Earth" in the localization but uses "@(S_Poison)" internally
- Combat skills (type 1) generally take 1 skill slot
- Magic skills (type 2) generally take 2 skill slots
- Scouting has only 3 max levels (scout speed/range tiers)

---

## SkillPointTable.sox Format

Defines cumulative XP required for each level (1-100).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (100) |
| 0x08 | 8×N | records | Fixed 8-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (8 bytes):**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Level (1-100) |
| 0x04 | 4 | uint32 | Cumulative XP required |

**XP Progression Sample:**
| Level | Cumulative XP | Delta XP |
|-------|---------------|----------|
| 1 | 10 | 10 |
| 10 | 131 | 29 |
| 20 | 1,071 | 174 |
| 30 | 3,801 | 372 |
| 50 | 16,431 | 869 |
| 75 | 45,906 | 1,494 |
| 100 | 91,756 | 2,119 |

The XP curve is polynomial, with delta XP increasing by approximately 25 per level.

---

## CharInfo.sox Format

Defines character/unit type configurations for all unit types (63 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (63) |
| 0x08 | 136×N | records | Fixed 136-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (136 bytes = 34 uint32 fields):**
| Field | Type | Description |
|-------|------|-------------|
| 0 | uint32 | Character ID (0-62) |
| 1 | int32 | Animation ID (-1 = none, see below) |
| 2 | int32 | Model ID variant (-1 = default) |
| 3 | int32 | Unknown (-1 = default) |
| 4 | uint32 | Max HP |
| 5 | uint32 | **Hero Class** (see below) |
| 6 | uint32 | Unknown HP field |
| 7 | uint32 | Unknown (0) |
| 8 | uint32 | Unknown (2) |
| 9 | uint32 | Base stat value |
| 10 | uint32 | Unknown (15) |
| 11 | uint32 | Unknown (200) |
| 12-19 | uint32[8] | Combat stats and modifiers |
| 20-27 | int32[8] | Skill/ability slot IDs (-1 = empty) |
| 28-32 | int32[5] | Additional ability slots (-1 = empty) |
| 33 | uint32 | Unknown footer value |

**Field 5 - Hero Class:**

Controls whether a character can use elemental equipment attributes (Fire, Ice, Lightning, Holy, Poison, Curse from ItemAttInfo Category 7).

| Value | Meaning | Characters |
|-------|---------|------------|
| 5 | Human Hero | Gerald (Hironeiden), Kendal (Ecclesian) |
| 10 | Dark Elf Hero | Lucretia (Dark Legion) |
| 15 | Empowered Hero | Regnier (Dark Legion/Hexter, transformed by Ancient Heart) |
| 0 | Generic Troop | Siege units, flying units, etc. |
| 20 | Standard Troop | Infantry, spearmen, knights, etc. |
| 25 | Special NPC | Urukubarr |
| 40 | Officer/Support | Ellen, Morene, Rupert, Cirith, Leinhart, Thomas, Duane, etc. |

**Elemental Equipment Rule:** Only characters with Field5 in {5, 10, 15} can receive elemental equipment attributes. All officers (Field5=40) cannot.

**Field 1 - Animation ID (Playable Heroes):**

| Anim ID | Hero | Notes |
|---------|------|-------|
| 30 | Gerald | Human sword fighter |
| 31 | Kendal | Human mace/staff fighter |
| 32 | Regnier | Transformed human, huge flaming sword |
| 44 | Lucretia | Dark Elf dual sword fighter |

These 4 animation IDs are unique to the main playable heroes and have full player-controllable combat animations. Other characters use animation IDs 0-29 (troops) or 33+ (officers/NPCs).

**Main Playable Heroes:**

| Char ID | Name | Faction | Race | Field5 | Anim ID |
|---------|------|---------|------|--------|---------|
| 32 | Gerald | Hironeiden | Human | 5 | 30 |
| 37 | Kendal | Ecclesian | Human | 5 | 31 |
| 43 | Lucretia | Dark Legion | Dark Elf | 10 | 44 |
| 34 | Regnier | Dark Legion/Hexter | Human (empowered) | 15 | 32 |

**Localized File (_ENG.sox) Structure:**
Variable-length records with ID and display name:
- uint32: Character ID
- uint16 + string: Display name (length-prefixed)

---

## ResistInfo.sox Format

Defines resistance types and their UI display (10 types).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (10) |
| 0x08 | var | records | Variable-length records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (variable):**
| Field | Type | Description |
|-------|------|-------------|
| Resist ID | uint32 | 0-9 |
| Unknown1 | uint16 | Always 10 (0x0A) |
| Unknown2 | uint16 | Always 10 (0x0A) |
| Loc Key | uint16 + string | Localization key (e.g., "@(R_Melee)") |
| Icon | uint16 + string | Icon path (e.g., "IL_RES_MELEE.tga") |

**All Resistance Types:**
| ID | Loc Key | Icon | Description |
|----|---------|------|-------------|
| 0 | @(R_Melee) | IL_RES_MELEE.tga | Melee attacks |
| 1 | @(R_Range) | IL_RES_RANGE.tga | Ranged attacks |
| 2 | @(R_Unblock) | IL_RES_UNBLOKABLE.tga | Explosion/siege |
| 3 | @(R_Frontal) | IL_RES_FRONTAL.tga | Frontal charges |
| 4 | @(R_Fire) | IL_RES_FIRE.tga | Fire elemental |
| 5 | @(R_Light) | IL_RES_LIGHTING.tga | Lightning elemental |
| 6 | @(R_Ice) | IL_RES_ICE.tga | Ice elemental |
| 7 | @(R_Holy) | IL_RES_HOLY.tga | Holy magic |
| 8 | @(R_Poison) | IL_RES_POISON.tga | Poison/Earth |
| 9 | @(R_Curse) | IL_RES_CURSE.tga | Curse magic |

---

## UnitUVInfo.sox Format

Defines UV texture coordinates and animation data for units (109 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (109) |
| 0x08 | 32×N | records | Fixed 32-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (32 bytes = 8 uint32 fields):**
| Field | Type | Description |
|-------|------|-------------|
| 0 | uint32 | Unknown (typically 0) |
| 1 | uint32 | Base value (100) |
| 2 | uint32 | UV offset X (84) |
| 3 | uint32 | UV offset Y (50) |
| 4 | uint32 | UV scale (42) |
| 5 | uint32 | Animation frame data |
| 6 | uint32 | Animation timing |
| 7 | uint32 | Frame count / index |

**Notes:**
- First record appears to be a header/default template
- Values are interpreted as animation frame coordinates
- Used for rendering unit sprites on the battlefield

---

## UnitUVID.sox Format

Maps unit types to their available upgrades and variants (61 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (61) |
| 0x08 | 72×N | records | Fixed 72-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (72 bytes = 18 int32 fields):**
| Field | Type | Description |
|-------|------|-------------|
| 0 | int32 | Unit ID |
| 1-17 | int32[17] | Upgrade/variant slot IDs (-1 = unused) |

**Notes:**
- Each unit can have up to 17 upgrade paths or variants
- Value -1 (0xFFFFFFFF) indicates an unused slot
- Referenced by CharInfo for determining available upgrades
- Unit ID 0 typically has no upgrades (base template)

**Example Mappings:**
| Unit ID | Available Upgrades |
|---------|-------------------|
| 1 | [1, 2, 3] |
| 5 | [10, 11] |
| 13 | [21, 22, 23, 24, 25, 26] |

---

## WorldMap_CharInfo.sox Format

Defines character display data for the world map (63 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (63) |
| 0x08 | 28×N | records | Fixed 28-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (28 bytes = 7 int32 fields):**
| Field | Type | Description |
|-------|------|-------------|
| 0 | int32 | Character ID |
| 1 | int32 | Primary slot ID (-1 = none) |
| 2 | int32 | Secondary slot ID (-1 = none) |
| 3 | int32 | Unknown (-1 = none) |
| 4 | int32 | Unknown (-1 = none) |
| 5 | int32 | Icon/Portrait ID 1 |
| 6 | int32 | Icon/Portrait ID 2 |

**Notes:**
- Used for rendering characters on the world map screen
- Character ID 0 is often empty/template
- Slot IDs reference equipment or ability configurations
- Portrait IDs determine which character portrait to display

---

## WorldMap_TroopInfo.sox Format

Defines troop display data for the world map (35 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (35) |
| 0x08 | 28×N | records | Fixed 28-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (28 bytes = 7 int32 fields):**
| Field | Type | Description |
|-------|------|-------------|
| 0 | int32 | Troop ID |
| 1 | int32 | Primary reference ID |
| 2 | int32 | Secondary reference ID (-1 = none) |
| 3 | int32 | Tertiary reference ID (-1 = none) |
| 4 | int32 | Unknown (-1 = none) |
| 5 | int32 | Icon ID 1 |
| 6 | int32 | Icon ID 2 |

**Notes:**
- Similar structure to WorldMap_CharInfo
- Used for rendering troop icons on the world map
- Reference IDs link to CharInfo or TroopInfo entries

---

## LibraryInfo.sox Format

Defines offset pointers for library/encyclopedia entries (100 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (100) |
| 0x08 | 6×N | records | Fixed 6-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (6 bytes):**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Data offset (file position) |
| 0x04 | 2 | uint16 | Unknown (always 0) |

**Notes:**
- Offsets point to string data or external resources
- Offsets are absolute file positions (0x834+)
- Entries are evenly spaced (10 bytes apart in target data)
- Used for in-game library/codex content lookup

---

## SpecialNames.sox Format

Defines special named characters for localization (12 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (12) |
| 0x08 | var | records | Variable-length string records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (variable):**
| Field | Type | Description |
|-------|------|-------------|
| Name | uint16 + string | Length-prefixed name string |

**Main File Names (Korean):**
| Index | Name | Description |
|-------|------|-------------|
| 0 | monarch | King reference key |
| 1 | (empty) | - |
| 2 | hugh | Hugh reference key |
| 3 | (empty) | - |
| 4 | rumen | Rumen reference key |
| 5 | Rumen | Capitalized variant |
| 6 | --리스린-- | Korean: Rithrin |
| 7 | (empty) | - |
| 8 | --적수색대-- | Korean: Enemy Scout |
| 9 | (empty) | - |
| 10 | --월든-- | Korean: Walden |
| 11 | (empty) | - |

**Localized File (_ENG.sox) Names:**
| Index | Name |
|-------|------|
| 0 | King |
| 1 | Hugh |
| 2 | Rumen |
| 3 | Rithrin |
| 4 | Enemy Scout |
| 5 | Walden |
| 6 | Magic Infantry |
| 7 | Head Troop |
| 8 | Patriarch |
| 9 | Patriarchal Guard |
| 10 | Walter |
| 11 | Hugh |

---

## KUF2CustomRandomTable.sox Format

Defines random generation configuration for custom battles (10 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (10) |
| 0x08 | 2+4 | strings | "6000" and "7002" (gold values?) |
| 0x12 | 120×N | records | Fixed 120-byte records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Header Strings:**
- String 1: "6000" - Possibly starting gold for one side
- String 2: "7002" - Possibly starting gold for other side

**Record Structure (120 bytes = 30 uint32 fields):**
| Field | Type | Description |
|-------|------|-------------|
| 0-1 | uint32[2] | Unknown (0, 3) |
| 2-3 | uint32[2] | Config flags (1, 1) |
| 4-5 | uint32[2] | Unknown (0-1) |
| 6-29 | uint32[24] | Unit/ability pool IDs (999999 = unused) |

**Notes:**
- Used for generating random armies in custom/skirmish battles
- Value 999999 (0x000F423F) indicates unused slot
- Likely defines pools of available units for random selection
- First two records have actual configuration, rest are templates

---

## WorldMap_TroopInfo.sox Internal Structure

From Discord community (user Keaton), this struct controls barracks equipment display and model advancement:

```c
struct _WORLDMAP_TROOPINFO {
    int job;
    char weaponmeshid;
    char offhandid;
    char offhandid2;
    char offhandid3;
    short armorid_start;
    short armorid_end;
}
```

Note: This is distinct from TroopInfo.sox. The char/short fields are padded to 4-byte alignment in the actual SOX file.

---

## FontType.sox Format

Defines font styling configurations for UI text (34 entries).

**File Structure:**
| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0x00 | 4 | uint32 | Header marker (100) |
| 0x04 | 4 | uint32 | Record count (34) |
| 0x08 | var | records | Variable-length records |
| EOF-64 | 64 | char[] | 'THEND' + space padding |

**Record Structure (variable):**
| Field | Type | Description |
|-------|------|-------------|
| Font ID | uint32 | 0-33 |
| Style 1 | uint16 + string | Primary style string |
| Style 2 | uint16 + string | Secondary/fallback style string |

**Style String Format:**
Font styles use a markup format with directives:
- `@(restore)` - Reset to default style
- `@(color=ffRRGGBB)` - Set text color (ARGB hex)
- `@(regular)` or `@(italic)` - Font weight
- `@(scale=X.X,Y.Y)` - Text scale (width,height)
- `@(linespace=N)` - Line spacing in pixels

**Example Style Strings:**
| ID | Style Description |
|----|-------------------|
| 0 | Gold text (FFE9A3), 0.8x scale |
| 3 | Brown text (B59D68), 0.7x scale |
| 4 | Orange text (FFC000), 0.7x scale |
| 9 | Gold text (F1B423), 0.9x scale |
| 20 | Gold text (FFE9A3), 1.2x scale |

**Common Colors:**
| Hex | Name | Usage |
|-----|------|-------|
| FFE9A3 | Gold | Headers, important text |
| B59D68 | Brown | Secondary text |
| FFC000 | Orange | Highlights |
| F1B423 | Dark Gold | Titles |
| 89B3FF | Blue | Links, special text |
| FF6A42 | Red | Warnings |
| 878075 | Gray | Disabled/inactive |
