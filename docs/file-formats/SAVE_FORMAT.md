# Save File Format Documentation

Save file format documentation for Kingdom Under Fire: The Crusaders (PC port). Reverse engineered from `Kuf2Main.exe` via Ghidra binary analysis.

## Overview

The game has one active save format on the PC port. A second format (Campaign Save) exists in the binary as dead code from the Xbox version — it is never written or read. Neither format is encrypted (`EncryptSaveData_Noop` at `0x0042a2f0` returns 0 unconditionally).

| Format | Magic | Writer | Reader | Status |
|--------|-------|--------|--------|--------|
| World Map Save | `0x6E` (110) | `WriteSaveFile` (0x004c8be0) | `ReadSaveFile` (0x004c73e0) | **Active** — all PC save files use this format |
| Campaign Save | None | `WriteCampaignSave` (0x00626d60) | None | **Dead code on PC** — Xbox-only multi-phase battle checkpoint, never reached |

`ReadSaveFile` validates the first 4 bytes against `0x6E` and rejects anything else. Since the campaign save has no magic number, even if one were somehow written it could never be read back.

**Key characteristics:**
- **Byte order**: Little-endian (x86)
- **Alignment**: All save data is zero-padded to 0x8000 (32,768) bytes — an Xbox memory unit page size remnant
- **File wrapper**: When written via memory card path, a 4-byte size prefix is prepended: `uint32(stream_size + 4)`
- **Stream buffer**: All data passes through a 128KB (0x20000 byte) `BBufferStream` object

## File Locations

| File | Purpose | Path Pattern |
|------|---------|-------------|
| `kuf.sav` | Primary save | `{base_path}\{slot_name}\kuf.sav` |
| `kuf2.sav` | Backup save | `{save_path}\kuf2.sav` |

`{base_path}` is stored in global `DAT_007467c0`. `{slot_name}` is a save slot identifier (Xbox memory unit remnant; on PC it maps to a subdirectory).

## Format 1: World Map Save

The primary save format used during gameplay. Written by `SaveGameState` → `WriteSaveFile` and read by `ReadSaveFile` / `LoadKufSav`.

### File Structure

When written via file mode (`param_6 != 0`), the raw stream is written directly. When via memory card mode (`param_6 == 0`), a 4-byte size prefix wraps the stream.

```
┌─────────────────────────────────────────────────┐
│ [Optional: 4 bytes] File size prefix            │  Memory card mode only
├─────────────────────────────────────────────────┤
│ Stream Data (padded to 0x8000 = 32,768 bytes)   │
│                                                 │
│   ┌─ Magic ─────────────────────────────────┐   │
│   │ [4 bytes] uint32 = 0x6E (110)           │   │
│   ├─ Save Context (optional) ───────────────┤   │
│   │ [0x438 bytes] Save context data         │   │  Only if context was provided
│   ├─ Campaign Index ────────────────────────┤   │
│   │ [4 bytes] uint32 campaign_index         │   │
│   ├─ Main Save Block ──────────────────────┤   │
│   │ [0x154 bytes] Game state data           │   │  340 bytes from state+0xA4
│   ├─ Unit Array ────────────────────────────┤   │
│   │ [4 bytes] uint32 unit_count             │   │
│   │ unit_count × [483 bytes] Per-Unit Data  │   │
│   ├─ Selected Unit Reference ───────────────┤   │
│   │ [4 bytes] uint32 selected_unit_ref      │   │
│   ├─ Roster State ──────────────────────────┤   │
│   │ [4 bytes] uint32 roster_count           │   │
│   │ roster_count × [8 bytes] Roster Entry   │   │
│   ├─ Second Array ──────────────────────────┤   │
│   │ [4 bytes] uint32 count                  │   │
│   │ count × [4 bytes] uint32 values         │   │
│   ├─ Mission Completion Data ───────────────┤   │
│   │ 20 × [4 bytes] Completion flags         │   │
│   ├─ Current Mission Index ─────────────────┤   │
│   │ [4 bytes] uint32 current_mission_index  │   │
│   ├─ Per-Mission State ─────────────────────┤   │
│   │ Variable-length mission state data      │   │
│   ├─ Current Mission Slot Index ────────────┤   │
│   │ [4 bytes] uint32 mission_slot_index     │   │
│   ├─ Per-Mission Array ─────────────────────┤   │
│   │ Variable-length per-mission entries     │   │
│   ├─ Campaign-Specific Data ────────────────┤   │
│   │ 5 or 50 bytes (campaign-dependent)      │   │
│   ├─ Script Objects ────────────────────────┤   │
│   │ [4 bytes] uint32 count                  │   │
│   │ count × [64 bytes] Script object data   │   │
│   ├─ Zero Padding ─────────────────────────┤   │
│   │ Zeros to 0x8000 boundary                │   │
│   └─────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

### Magic Number

| Offset | Size | Type | Value | Description |
|--------|------|------|-------|-------------|
| 0x00 | 4 | uint32 | `0x6E` (110) | Format magic. ReadSaveFile rejects files without this value |

### Save Context (Optional, 0x438 bytes)

Only present when `WriteSaveFile` is called with a save context (`param_1 != 0`). Built by `SaveGameState` with timestamps, play time, campaign info, and kernel state flags. The save context is initialized by `InitSaveContext` (0x004c4ab0).

### Campaign Index

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Campaign index: 0=Hironeiden (Gerald), 1=Vellond (Lucretia), 2=Ecclesia (Kendal), 3=Dark Legion (Regnier) |

### Main Save Block (0x154 bytes)

340 bytes of game state data copied from the game state object at offset 0xA4. Contains general gameplay state including current position, progress flags, and world map state.

### Unit Array (Player Barracks)

Written by `WriteUnitArray` (0x0055c860), read by `ReadUnitArray` (0x0055c5e0).

**This is the player's barracks** — every unit the player has accumulated across the campaign is stored here. Units persist between missions; their stats, equipment, abilities, and officer assignments are preserved.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Unit count |
| +0x04 | 483×N | bytes | N × Per-Unit Data (see below) |

**`char_id`** (save offset 32, runtime offset 0x20, from STG byte 0x56) is the universal linking field. It connects saved barracks units to mission unit slots, and is used for merge-on-load and deployment table matching.

`ReadUnitArray` has two modes controlled by a `mergeMode` parameter:
- **Mode 0 (fresh load)**: Frees the existing unit vector, creates new 0x508-byte unit objects from the save data.
- **Mode 1 (merge-by-char_id)**: Reads saved units and matches each against existing units by `char_id`. If a match is found, the entire 0x508-byte object is overwritten with the saved data. Unmatched units are appended. This preserves player upgrades when re-entering a previously visited mission.

### Selected Unit Reference

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Currently selected unit. Matched against the first uint32 of each world map node record during load |

### World Map Node State

Despite the name "Roster State" in earlier documentation, these are **world map node records** (0x7C bytes each in memory), NOT unit roster entries. Each record represents a location on the world map (castle, town, checkpoint). Populated by `ReadWorldMapBlockData` (0x004c7970) from `WMBLOCKINFO` blocks.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Node count (derived from `(vec.end - vec.begin) / 0x7C`) |

Per node entry in the save (8 bytes of state data):

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 1 | byte | Node field at record+0x61 |
| +0x01 | 1 | byte | Node field at record+0x60 |
| +0x02 | 1 | byte | Node field at record+0x62 (if non-zero and record+0x10 == 0, triggers unit spawn) |
| +0x03 | 1 | byte | Node field at record+0x63 |
| +0x04 | 4 | uint32 | Node field at record+0x64 |

For each active node (offset 0x10 != 0), `LoadWorldMapCastleSTGs` (0x004c5fe0) loads a `WorldmapCastle%04d_{H/V/E/X}.stg` file to initialize that node's mission data.

### Second Array

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Count |
| +0x04 | 4×N | uint32[] | N uint32 values |

### Mission Completion Data

Written by `WriteMissionCompletionData` (0x00494190), read by `ReadMissionCompletionData` (0x00493bc0).

20 entries, each 4 bytes, read from `DAT_00743960` with 0x4C stride at offset 0x48:

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 80 | uint32[20] | Mission completion flags (one per mission slot) |

### Current Mission Index

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Index of the current mission |

### Per-Mission State

Written by `WriteMissionStateData` (0x004941d0), read by `ReadMissionStateData` (0x00493c10).

**Critical**: Variable and sub-entry counts are taken from runtime data, NOT from the save file. The save format is NOT self-describing for these counts — the corresponding STG file must be loaded to correctly parse mission state.

Per visited mission:

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 2 | uint16 | Mission state (offset 0x40 in mission object) |
| +0x02 | 1 | byte | Flag byte (offset 0x48) |

Followed by variables (count from runtime, 0x14-byte records):

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Variable ID |
| +0x04 | 4 | uint32 | Variable value |
| +0x08 | var | bytes | Sub-entries (see Sub-Entry format) |

#### Sub-Entry Format

Written by `WriteSubEntryToSave` (0x00466bf0), read by `ReadSubEntryFromSave` (0x00466ab0).

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Event tag (validated against runtime) |
| +0x04 | 4 | uint32 | Status |
| +0x08 | 1 | byte | Flag |
| +0x09 | 4 | uint32 | Sub-sub count |
| +0x0D | 8×N | bytes | N × key/value pairs (4 bytes key + 4 bytes value) |

On read, if the sub-sub count mismatches runtime expectations, the reader seeks past the data and fills defaults based on status type.

#### Typed Values

Following the variables, typed values are written:

| Type Tag | Read Size | Description |
|----------|-----------|-------------|
| 0 | 4+4 bytes | Integer: tag + value tag + int32 |
| 1 | 4+4 bytes | Float: tag + value tag + float32 |
| 2 | 4+4+N bytes | String: tag + value tag + uint32 length + N bytes data |
| 3 | 4+4 bytes | Enum: tag + value tag + int32 |

#### Footer Entries

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Footer count (self-describing) |
| +0x04 | 8×N | bytes | N × 8-byte entries |

### Current Mission Slot Index

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Current mission slot index |

### Per-Mission Array

For each mission in the mission array:

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Mission ID |
| +0x04 | var | bytes | Mission state data (same format as Per-Mission State above) |

### Campaign-Specific Data

Written by `WriteCampaignSpecificData` (0x0055b080), read by `ReadCampaignSpecificData` (0x0055a6b0).

| Campaign Index | Size | Source Offset | Description |
|----------------|------|---------------|-------------|
| 0-1 (Gerald, Lucretia) | 5 bytes | offset 0x5DC | Campaign-specific state |
| 2-3 (Kendal, Regnier) | 50 bytes (0x32) | offset 0x5E1 | Extended campaign state |

### Script Object Array

Written by `WriteScriptObjectArray` (0x00454850), read by `ReadScriptObjectArray` (0x00454650).

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 4 | uint32 | Object count |
| +0x04 | 64×N | bytes | N × 64-byte (0x40) script object data |

On read, 0xC0-byte (192) runtime objects are allocated, and 64 bytes are read into each. Sequential IDs are assigned from a global counter.

### Zero Padding

The stream is zero-padded to exactly 0x8000 (32,768) bytes — an Xbox memory unit page size.

---

## Format 2: Campaign Save (Dead Code on PC)

> **This entire format is unreachable on the PC port.** The code path that triggers `WriteCampaignSave` requires the `Kuf2GameMenuManager` to show an Xbox memory unit save slot picker (vtable[2] code `0x1a`), which allocates a 0x100-byte data buffer. On PC, code `0x1a` is a no-op in all three vtable variants — `MenuMgrHandleSystemMessage` (0x00577f20) only handles codes 0x18/0x19. The buffer is never allocated, `SetCampaignSlotValue` would crash on the null pointer, and the state 0x13 path (`GameplayLoopCrusaders` returning 0x10) is never triggered. Documented here for completeness of the binary analysis.

On Xbox, this would have been written during **multi-phase Crusaders battle transitions** by `WriteCampaignSave` (0x00626d60). It captures the campaign-level unit roster across all 4 campaigns during mid-battle phase changes. It is a **write-only format** with no dedicated reader. Even on Xbox, the file would be written to `kuf.sav` and overwritten by the World Map Save when the player returns to the world map.

**Intended trigger (Xbox only)**: `GameplayLoopCrusaders` returns 0x10 → `GameMainFinalize` → state 0x13 → `RunGameMainPreInit(1)` → `SetCampaignSlotValue` + `WriteCampaignSave`. Only occurs in Crusaders mode (game_state+0x52 != 0).

The header's first 0x10 bytes contain the save slot directory name (e.g., "SAVE GAME 1") which is also used as the path for `CreateSaveFileForWrite`.

### File Structure

```
┌─────────────────────────────────────────────────┐
│ [4 bytes] File size prefix (WriteStreamToFile)  │
├─────────────────────────────────────────────────┤
│ Stream Data (padded to 0x8000 = 32,768 bytes)   │
│                                                 │
│   ┌─ Header ────────────────────────────────┐   │
│   │ [0x10 bytes] Campaign save header       │   │
│   ├─ Campaign Slot 0 ──────────────────────┤   │
│   │ [0x20 bytes] Slot header                │   │
│   │ [2 bytes] uint16 field                  │   │
│   │ [4 bytes] uint32 field                  │   │
│   │ [4 bytes] uint32 field                  │   │
│   │ [4 bytes] uint32 field                  │   │
│   │ [2 bytes] uint16 unit_count             │   │
│   │ unit_count × [483 bytes] Per-Unit Data  │   │
│   ├─ Campaign Slot 1 ──────────────────────┤   │
│   │ (same structure as Slot 0)              │   │
│   ├─ Campaign Slot 2 ──────────────────────┤   │
│   │ (same structure as Slot 0)              │   │
│   ├─ Campaign Slot 3 ──────────────────────┤   │
│   │ (same structure as Slot 0)              │   │
│   ├─ Zero Padding ─────────────────────────┤   │
│   │ Zeros to 0x8000 boundary                │   │
│   └─────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

### Header (0x10 bytes)

The first 0x10 bytes of the campaign data structure. Contents are copied directly from the in-memory campaign save object.

### Campaign Slots (4 slots, 0x3c-byte stride in source)

Each slot represents one campaign (Hironeiden, Vellond, Ecclesia, Dark Legion):

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| +0x00 | 0x20 | bytes | Slot header (campaign name, progress state) |
| +0x20 | 2 | uint16 | Unknown field |
| +0x24 | 4 | uint32 | Unknown field |
| +0x28 | 4 | uint32 | Unknown field (set by `SetCampaignSlotValue`) |
| +0x2C | 4 | uint32 | Unknown field |
| +0x30 | 2 | uint16 | Unit count (computed from unit vector size) |
| +0x32 | 483×N | bytes | N × Per-Unit Data (see below) |

### In-Memory Data Structure

The campaign save data is managed through a 0x3c-byte wrapper object created by `CampaignSaveInit` (0x00626b60):

| Wrapper Offset | Type | Description |
|----------------|------|-------------|
| 0x00 | ptr | Pointer to 0x100-byte data buffer (**always NULL on PC** — never allocated) |
| 0x04 | byte | Unknown flag |
| 0x14 | int32 | Active campaign index (-1 if none) |
| 0x18 | ptr | STG unit block vector begin |
| 0x1C | ptr | STG unit block vector end |
| 0x20 | ptr | STG unit block vector capacity |
| 0x24-0x30 | raw | Zero-initialized constants (from `0x006bd570`) |

The 0x100-byte data buffer layout (never allocated on PC):

| Buffer Offset | Size | Description |
|---------------|------|-------------|
| 0x00-0x0F | 0x10 | Header |
| 0x10-0x4B | 0x3C | Campaign slot 0 (Hironeiden) |
| 0x4C-0x87 | 0x3C | Campaign slot 1 (Vellond) |
| 0x88-0xC3 | 0x3C | Campaign slot 2 (Ecclesia) |
| 0xC4-0xFF | 0x3C | Campaign slot 3 (Dark Legion) |

Within each 0x3C slot in the buffer:

| Slot Offset | Size | Description |
|-------------|------|-------------|
| 0x00-0x1F | 0x20 | Slot header |
| 0x20-0x21 | 2 | uint16 field |
| 0x24-0x27 | 4 | uint32 field |
| 0x28-0x2B | 4 | uint32 field (written by SetCampaignSlotValue) |
| 0x2C-0x2F | 4 | uint32 field |
| 0x30-0x33 | ptr | Unit vector begin |
| 0x34-0x37 | ptr | Unit vector end |
| 0x38-0x3B | ptr | Unit vector capacity |

---

## Per-Unit Save Data (483 bytes)

Written by `WriteUnitSaveData` (0x0058ea50), read by `ReadUnitSaveData` (0x0058e780). Each unit is serialized from a 0x508-byte (1288 bytes) runtime object.

**Note**: Two fields are written out of sequential order (marked with ⚠️).

| Save Offset | Size | Runtime Offset | Type | Default | Description |
|-------------|------|----------------|------|---------|-------------|
| 0 | 4 | 0x24 | int32 | -1 | Unknown index |
| 4 | 4 | 0x28 | int32 | -1 | Troop info index |
| 8 | 4 | 0x2C | int32 | | Job type (0-42 = K2_JOB_TYPE, ≥43 = CharInfo index) |
| 12 | 4 | 0x30 | int32 | | Model ID / sub_type |
| 16 | 4 | 0x34 | int32 | | STG field (from offset 0x190) |
| 20 | 4 | 0x38 | int32 | | STG field (from offset 0x192) |
| 24 | 4 | 0x3C | int32 | | STG field (from offset 0x194) |
| 28 | 4 | 0x40 | int32 | | STG field (from offset 0x198) |
| 32 | 4 | 0x20 | int32 | -1 | ⚠️ Char ID (unit identity, from STG byte 0x56) |
| 36 | 4 | 0x48 | int32 | -1 | Troop info index (second copy, from STG 0x1C0) |
| 40 | 4 | 0x4C | int32 | 3 | UCD: 0=Player, 1=Enemy, 2=Ally, 3=Neutral |
| 44 | 4 | 0x44 | int32 | | ⚠️ Formation type / combat config |
| 48 | 4 | 0x50 | int32 | | Grid/formation config |
| 52 | 4 | 0x54 | int32 | 1 | Skill level (computed by `CalcUnitSkillLevel`, cap 99) |
| 56 | 1 | 0x58 | byte | 0x01 | Byte field |
| 57 | 1 | 0x59 | byte | | Hero flag (0=hero character, 1=regular troop) |
| 58 | 1 | 0x5A | byte | 0x01 | Byte field |
| 59 | 4 | 0x60 | int32 | 0 | Unknown |
| 63 | 4 | 0x64 | int32 | 0 | Unknown |
| 67 | 4 | 0x68 | int32 | 0 | Unknown |
| 71 | 24 | 0x6C | int32[6] | 0xFFFF pairs | Equipment items (6 slots × 4 bytes) |
| 95 | 64 | 0x84 | byte[64] | | Leader ability/skill set 1 |
| 159 | 64 | 0x144 | byte[64] | | Officer 1 ability/skill set 1 |
| 223 | 64 | 0x204 | byte[64] | | Officer 2 ability/skill set 1 |
| 287 | 64 | 0x2C4 | byte[64] | | Leader ability/skill set 2 |
| 351 | 64 | 0x384 | byte[64] | | Officer 1 ability/skill set 2 |
| 415 | 64 | 0x444 | byte[64] | | Officer 2 ability/skill set 2 |
| 479 | 4 | 0x504 | int32 | 0 | Unknown |

**Total: 483 bytes per unit**

### Ability/Skill Sets

Each unit has 6 ability/skill sets (64 bytes each = 384 bytes total), organized as 3 character slots × 2 sets:

| Character | Set 1 (Save Offset) | Set 2 (Save Offset) | Runtime Stride |
|-----------|---------------------|---------------------|----------------|
| Leader | 95 | 287 | 0xC0 (192) bytes per slot |
| Officer 1 | 159 | 351 | |
| Officer 2 | 223 | 415 | |

Only 64 bytes are saved from each 192-byte runtime slot. On read, each ability slot is initialized by `InitAbilitySlot` (0x0058ebe0) which assigns a sequential ID from global counter `DAT_0073235c`.

### Equipment Items

The 6 equipment slots at save offset 71 (24 bytes) use `0xFFFF` as a pair marker for empty slots. Equipment values are uint16 pairs packed into 4-byte int32 slots.

### Unit Name Resolution on Load

After reading unit data, `ReadUnitArray` resolves display names through the following priority chain:
1. `CheckSpecialNameOverride` — names starting with `-` are looked up in SpecialNames.sox
2. `GetUnitDisplayName` — checks CharInfo.sox for hero/leader types (job_type ≥ 0x20)
3. `GetWorldMapCharName` — fallback to WorldMap_CharInfo.sox

---

## Stream I/O System

### BBufferStream (128KB)

All save data is serialized through a `BBufferStream` object created by `BBufferStream_Create` (0x005becb0).

| Object Offset | Type | Description |
|---------------|------|-------------|
| 0x00 | ptr | vtable pointer |
| 0x04 | ptr | Buffer pointer (GameMalloc-allocated) |
| 0x08 | int32 | Read position |
| 0x0C | int32 | Write position |
| 0x10 | int32 | Buffer capacity (0x20000 = 131,072) |
| 0x14 | int32 | Flags |

**Vtable methods:**

| Vtable Offset | Method | Description |
|---------------|--------|-------------|
| +0x00 | Destructor | Frees buffer |
| +0x04 | Seek/Reset | Resets stream position |
| +0x08 | Read | `read(dest, size)` |
| +0x0C | Write | `write(src, size)` |
| +0x1C | GetSize | Returns current stream size |

### File Handle Wrapper

Save files are opened through a handle wrapper initialized by `SaveFileHandle_Init` (0x005c3da0) and closed by `SaveFileHandle_Close` (0x005c48d0).

### File I/O Functions

| Function | Address | Description |
|----------|---------|-------------|
| `OpenSaveFileForRead` | 0x005c5050 | Opens file with `CreateFileA(GENERIC_READ, OPEN_EXISTING)` |
| `CreateSaveFileForWrite` | 0x005c5120 | Creates directory + file with `CreateFileA(GENERIC_WRITE, CREATE_ALWAYS)` |
| `WriteStreamToFile` | 0x005c5400 | Writes 4-byte size prefix + stream data |
| `ReadFileIntoStream` | 0x005c52f0 | Validates 4-byte size prefix, reads data into stream |
| `ReadSaveFromMemoryCard` | 0x005c4ed0 | Opens kuf.sav, validates size prefix, reads into BBufferStream |

---

## Save Orchestration

### Save Flow (WriteSaveFile)

```
SaveGameState (0x004c9110)
  ├─ PausePlayTimer (0x00558a30)
  ├─ GetTotalPlayTime (0x005595e0)
  ├─ GetAndResetPauseTime (0x00559700)
  ├─ InitSaveContext (0x004c4ab0)     ← builds 0x438-byte context
  ├─ WriteSaveFile (0x004c8be0)       ← main serialization
  │   ├─ BBufferStream_Create(0x20000)
  │   ├─ Write magic 0x6E
  │   ├─ Write save context (0x438 bytes)
  │   ├─ Write campaign index
  │   ├─ Write main save block (0x154 bytes)
  │   ├─ WriteUnitArray
  │   ├─ Write selected unit ref
  │   ├─ Write roster state
  │   ├─ Write second array
  │   ├─ WriteMissionCompletionData (20 × 4 bytes)
  │   ├─ Write current mission index
  │   ├─ WriteMissionStateData (per visited mission)
  │   ├─ Write mission slot index
  │   ├─ Per-mission array (ID + WriteMissionStateData)
  │   ├─ WriteCampaignSpecificData
  │   ├─ WriteScriptObjectArray
  │   └─ Zero-pad to 0x8000
  └─ CommitSaveToSlot (0x00594830)    ← copies to slot, reloads config
```

### Load Flow (ReadSaveFile)

```
LoadWorldMapSave (0x004c6ef0)
  ├─ Open save file
  ├─ ReadWorldMapBlockData (0x004c7970)
  │   └─ Read WMBLOCKINFO_{HN/VL/EL/HT} blocks → 0x7C-byte node records
  ├─ LoadWorldMapCastleSTGs (0x004c5fe0)
  │   └─ For each active node: ReadSTGFile("WorldmapCastle%04d_{H/V/E/X}.stg")
  └─ LoadKufSav (0x004c8ac0)
      └─ ReadSaveFile (0x004c73e0)
          ├─ BBufferStream_Create(0x20000)
          ├─ Validate magic == 0x6E
          ├─ Read save context (if present)
          ├─ Read campaign index
          ├─ Read main save block (0x154 bytes)
          ├─ ReadUnitArray (mode 0: fresh load)
          │   ├─ Free existing unit vector
          │   ├─ Allocate 0x508-byte objects
          │   ├─ ReadUnitSaveData (483 bytes each)
          │   └─ Resolve display names
          ├─ Read + resolve selected unit ref (match against node records)
          ├─ Read world map node state (8 bytes per node)
          ├─ Read second array
          ├─ ReadMissionCompletionData
          ├─ Read current mission index
          ├─ ReadMissionStateData
          ├─ Read mission slot index
          ├─ Per-mission array
          ├─ ReadCampaignSpecificData
          └─ ReadScriptObjectArray
```

### Save → Briefing → Battle Flow

After loading a save, the player enters the world map and selects a mission. The **deployment table** at `DAT_00746894+4 → +0x14` connects barracks units (from the save) to mission unit slots (from the STG file). Each deployment table entry is 8 bytes containing two `int16` char_id values: one identifying the STG mission slot, the other identifying the saved barracks unit.

```
World Map → Briefing → GameMainInit (0x0054fb60)
  ├─ BriefingGetResult (0x0054e670)
  │   ├─ Load mission STG via AllocAndReadSTG
  │   └─ Determine campaign from player hero's job_type
  ├─ ApplyBriefingResult (0x0055c8b0)
  │   ├─ Iterate deployment table (8-byte entries):
  │   │   ├─ entry[0] (int16): char_id of STG mission unit slot
  │   │   ├─ entry[1] (int16): char_id of saved barracks unit
  │   │   ├─ Find STG unit block where offset 0x20 == entry[0]
  │   │   ├─ Find saved unit where char_id == entry[1]
  │   │   └─ PopulateUnitFromSaveData (0x0055bda0)
  │   │       ├─ ApplyTroopInfoToUnit (combat stats from TroopInfo.sox)
  │   │       ├─ Resolve name (SpecialNames → CharInfo → WorldMap_CharInfo)
  │   │       ├─ Set faction via GetFactionFromJobType
  │   │       ├─ Copy job_type, sub_type, char_id, formation
  │   │       ├─ Copy 6 equipment items (uint16 pairs → int32)
  │   │       └─ Copy ability sets (6 × 64 bytes)
  │   └─ Assign up to 2 officers (UCD=1 or 2) with equipment
  ├─ CreateUnitsFromSTGBlocks (0x0055c290)
  │   └─ Create runtime unit objects from populated STG blocks
  └─ LoadWorldAndTroops (0x0054e930)
      └─ Create World, WorldView, load terrain/characters
```

The `char_id` field is the critical link at every stage: save ↔ deployment table ↔ STG mission slots ↔ runtime troop objects.

### Campaign Save Flow (Dead Code on PC)

The campaign save flow exists in the binary but is never executed on PC. The code path requires `GameplayLoopCrusaders` to return 0x10, which triggers state 0x13 → `RunGameMainPreInit(1)`. On PC, `GameplayLoopCrusaders` never returns this value. Additionally, `Kuf2GameMenuManager::vtable[2]` code 0x1a (called in param_2==0 path) is a no-op on PC, so the 0x100-byte data buffer that `SetCampaignSlotValue` dereferences without a null check is never allocated.

```
GameMainPreInit (0x0054f4a0)                    ← DEAD CODE ON PC
  ├─ CampaignSaveInit (0x00626b60)    ← creates wrapper, sets data_ptr = NULL
  ├─ LoadSOXPrimaryData               ← loads global data
  ├─ LoadSOXSecondaryData
  ├─ vtable[2](0x1a)                  ← NO-OP on PC (Xbox: shows save slot picker, allocates buffer)
  ├─ If resuming (param_2 == 1 or 2):
  │   ├─ SetCampaignSlotValue (0x00627020)  ← would crash: dereferences NULL data_ptr
  │   └─ WriteCampaignSave (0x00626d60)
  │       ├─ BBufferStream_Create(0x20000)
  │       ├─ Write 0x10 header
  │       ├─ 4 × Campaign Slot:
  │       │   ├─ Write 0x20 slot header
  │       │   ├─ Write uint16 + 3 × uint32
  │       │   ├─ Write uint16 unit_count
  │       │   └─ unit_count × WriteUnitSaveData
  │       ├─ Zero-pad to 0x8000
  │       ├─ CreateSaveFileForWrite → kuf.sav
  │       ├─ WriteStreamToFile
  │       └─ OpenFileForWrite → kuf2.sav (backup)
  └─ Continue to GameMainInit
```

---

## Function Reference

### Top-Level Save Functions

| Address | Name | Size | Description |
|---------|------|------|-------------|
| 0x004c9110 | `SaveGameState` | | High-level save orchestrator |
| 0x004c8be0 | `WriteSaveFile` | | World map save writer (magic 0x6E) |
| 0x004c73e0 | `ReadSaveFile` | | World map save reader |
| 0x004c8ac0 | `LoadKufSav` | | Wrapper: `ReadSaveFile("kuf.sav")` |
| 0x004c8b10 | `QuickSaveToFile` | | Wrapper: `WriteSaveFile("kuf.sav")` |
| 0x00626d60 | `WriteCampaignSave` | 692 | Campaign snapshot writer (write-only) |
| 0x004c6ef0 | `LoadWorldMapSave` | | World map save loader with block data and castle STGs |
| 0x004c5fe0 | `LoadWorldMapCastleSTGs` | | Loads WorldmapCastle STGs for active world map nodes |
| 0x004c7970 | `ReadWorldMapBlockData` | | Reads WMBLOCKINFO blocks into 0x7C-byte node records |

### Unit Data Functions

| Address | Name | Size | Description |
|---------|------|------|-------------|
| 0x0058ea50 | `WriteUnitSaveData` | | Writes 483 bytes per unit |
| 0x0058e780 | `ReadUnitSaveData` | | Reads 483 bytes per unit |
| 0x0058e4c0 | `UnitSaveObjectInit` | | Constructor for 0x508-byte unit object |
| 0x0055c860 | `WriteUnitArray` | | Writes count + N × units |
| 0x0055c5e0 | `ReadUnitArray` | | Reads count + N × units; mode 0=fresh, mode 1=merge-by-char_id |
| 0x0055c8b0 | `ApplyBriefingResult` | | Merges barracks units into STG slots via deployment table |
| 0x0055bda0 | `PopulateUnitFromSaveData` | | Applies TroopInfo stats, name, faction, equipment to STG unit |
| 0x0058ebe0 | `InitAbilitySlot` | | Initializes 64-byte ability slot |

### Mission State Functions

| Address | Name | Size | Description |
|---------|------|------|-------------|
| 0x004941d0 | `WriteMissionStateData` | | Per-mission state writer |
| 0x00493c10 | `ReadMissionStateData` | 677 | Per-mission state reader |
| 0x00494190 | `WriteMissionCompletionData` | | 20 × 4-byte completion flags |
| 0x00493bc0 | `ReadMissionCompletionData` | | Reads completion flags |
| 0x00466bf0 | `WriteSubEntryToSave` | 150 | Event state sub-entry writer |
| 0x00466ab0 | `ReadSubEntryFromSave` | 290 | Event state sub-entry reader |

### Campaign-Specific Functions

| Address | Name | Size | Description |
|---------|------|------|-------------|
| 0x0055b080 | `WriteCampaignSpecificData` | 62 | 5 or 50 bytes by campaign |
| 0x0055a6b0 | `ReadCampaignSpecificData` | 62 | Mirror of write |
| 0x00454850 | `WriteScriptObjectArray` | | Count + N × 64 bytes |
| 0x00454650 | `ReadScriptObjectArray` | | Allocates 0xC0-byte objects |
| 0x00626b60 | `CampaignSaveInit` | 62 | Campaign wrapper initialization |
| 0x00626c50 | `FreeCampaignSaveData` | | Frees 0x100-byte buffer + unit vectors |
| 0x00627020 | `SetCampaignSlotValue` | 92 | Sets value in active campaign slot |

### Stream/Buffer I/O Functions

| Address | Name | Size | Description |
|---------|------|------|-------------|
| 0x005becb0 | `BBufferStream_Create` | | 128KB stream constructor |
| 0x005c3da0 | `SaveFileHandle_Init` | | Handle init (INVALID_HANDLE_VALUE) |
| 0x005c48d0 | `SaveFileHandle_Close` | | CloseHandle wrapper |
| 0x005c5050 | `OpenSaveFileForRead` | | CreateFileA(GENERIC_READ) |
| 0x005c5120 | `CreateSaveFileForWrite` | | CreateDirectory + CreateFileA(GENERIC_WRITE) |
| 0x005c5400 | `WriteStreamToFile` | | 4-byte prefix + stream data |
| 0x005c52f0 | `ReadFileIntoStream` | | Validate prefix + read |
| 0x005c4ed0 | `ReadSaveFromMemoryCard` | | Opens kuf.sav, validates, reads to stream |
| 0x005c50f0 | `OpenSaveFileBySlot` | | Opens save file by slot name |
| 0x005c4c30 | `GetSaveDiskSpace` | | GetDiskFreeSpaceExA wrapper |
| 0x005c4bf0 | `GetFreeStorageBlocks` | | Free 16KB block count |
| 0x005c4ea0 | `ValidateMemCardSpace` | | Checks free storage |

### Utility Functions

| Address | Name | Description |
|---------|------|-------------|
| 0x00558a30 | `PausePlayTimer` | Pauses timer, accumulates elapsed time |
| 0x005595e0 | `GetTotalPlayTime` | Returns total play time |
| 0x00559700 | `GetAndResetPauseTime` | Returns and resets pause accumulator |
| 0x00558a60 | `GetElapsedTimeSinceRef` | timeGetTime() - reference |
| 0x0055b680 | `StartPlayTimer` | Records current time |
| 0x0055b550 | `SetTotalPlayTime` | Sets play time value |
| 0x0055af40 | `ResetPlayTimer` | Resets timer state |
| 0x00594830 | `CommitSaveToSlot` | Post-save slot copy (0x740 bytes/slot) |
| 0x0055ba70 | `FreeUnitVector` | Frees all unit objects in vector |
| 0x0055b750 | `FreeUnitVectorAndArray` | Frees units + backing array |
| 0x0042a2f0 | `EncryptSaveData_Noop` | Returns 0 (encryption disabled on PC) |

---

## Notes

### Xbox Remnants

1. **32KB alignment**: Save data is padded to 0x8000 bytes, matching Xbox memory unit page sizes
2. **Memory card path**: The save system has two code paths — file-based (PC) and memory card (Xbox). The PC port uses the file path but retains the memory card infrastructure
3. **Save slot system**: The 0x740-byte per-slot structure with wide-string names comes from the Xbox memory card UI
4. **4-byte size prefix**: Memory card saves use a size prefix for integrity validation
5. **Campaign Save format**: The entire `WriteCampaignSave` system is dead code — the `Kuf2GameMenuManager` never shows the Xbox memory unit save slot picker (code 0x1a is a no-op on PC), so the 0x100-byte data buffer is never allocated and the code path is never reached

### Parsing Limitations

1. **Mission state is NOT self-describing**: Variable counts, sub-entry counts, and typed value counts are taken from runtime (loaded STG) data, not from the save file. Parsing mission state data without the corresponding STG file is impossible.
2. **Footer counts ARE self-describing**: The footer section within mission state includes its own count.
3. **Campaign-specific data size varies**: Must know the campaign index to determine whether to read 5 or 50 bytes.

### Modding Considerations

1. **Save editing**: Since saves are unencrypted, direct hex editing is possible. The 32KB padding ensures fixed file size.
2. **Unit modification**: Change unit stats at known offsets within the 483-byte per-unit blocks. Key fields: job_type (offset 8), skill_level (offset 52), equipment (offset 71).
3. **`char_id` is the universal link**: The `char_id` field (save offset 32) connects barracks units to mission slots. When editing saves, preserve char_id values — changing them breaks the deployment table matching and the unit won't be recognized during briefing/battle transitions.
4. **Unit array IS the barracks**: All units in the save file's unit array are the player's accumulated barracks. Adding units to this array adds them to the barracks; removing units removes them. The merge-by-char_id mode (mode 1) ensures that returning to a previously visited mission preserves player upgrades.
5. **Campaign index**: At a fixed offset after the magic number (and optional context), the campaign index determines which world map and mission set to load.
6. **Only one format matters**: On the PC port, all save files use the World Map Save format (magic `0x6E`). The Campaign Save format is dead code and can be ignored for modding purposes.
7. **`kuf2.sav`**: Only written by the dead campaign save path. On PC, only `kuf.sav` is ever written (by `WriteSaveFile`).
