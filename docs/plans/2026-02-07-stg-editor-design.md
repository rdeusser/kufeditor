# STG Editor Design

## Objective

Add STG (mission file) editing to kufeditor for Kingdom Under Fire: Crusaders. The STG format defines unit placement, map regions, mission variables, event scripting, and configuration for each mission.

## V1 Scope

V1 focuses on Header + Units editing only. All other sections are preserved as raw byte blobs for round-trip safety.

### In Scope

- Parse and edit Header (mission ID, filenames, known fields; raw blob for 36-byte config at 0x24C)
- Parse and edit Units (core data, leader, officers, unit config; raw byte overlay for unknown/reserved fields)
- Enum dropdowns for UCD (4 values), Direction (8 values), JobType (43 values), AbilityType (57+ values)
- Validation: unique unit IDs, enum ranges, officer count 0-2, Worldmap ID warnings
- Round-trip fidelity: byte-identical save for unmodified files
- Backup-on-save (.bak files)
- Crusaders only (544-byte troop blocks)

### Deferred

- AreaID editing (stored as raw blob, possibly displayed read-only)
- Variable section (raw blob -- entry format unverified, variable-length entries)
- Event editing (raw blob -- requires condition/action catalog that doesn't exist yet)
- Footer (42 bytes, raw blob, zero documentation)
- Map visualization (2D canvas with unit positions and area bounds)
- Heroes support (548-byte troop blocks, different header/AreaID layouts)
- Briefing file support

## Format Risks

### Variable Section Problem (Critical)

The variable section has variable-length entries (string name + data). The exact entry size is unverified. Since STG parsing is sequential (each section's offset depends on the previous), misparsing variable entries corrupts everything after. V1 avoids this entirely by treating variables through footer as a single raw blob.

### 1088-byte Double Block Detection (High)

Player-controlled units use 1088 bytes (two 544-byte entries for unit + sub-unit). The detection heuristic is unclear -- likely UCD == 0 (Player), but this must be verified against multiple mission files before unit parsing is reliable.

### Unknown Bytes in Unit Blocks (Medium)

~252 of 544 bytes per unit are unknown/reserved. The raw byte overlay pattern preserves them, but they cannot be validated or meaningfully displayed.

### Data Corruption (Medium)

A corrupted STG crashes the game or bricks saves. Mitigations: backup-on-save, round-trip verification, validation on save.

## Architecture

### Data Model

```
StgFile
+-- StgHeader (628 bytes)
|   +-- missionId (uint32)
|   +-- mapFile, bitmapFile, cameraDefault, cameraUser, settingsFile, skyEffects, aiScript, cubemap (char[64] each)
|   +-- unitCount (uint32)
|   +-- rawHeader (std::array<std::byte, 628>) -- overlay for unknown fields
+-- std::vector<StgUnit>
|   +-- CoreUnitData: name (char[32]), uniqueId, ucd, isHero, isEnabled, leaderHp, unitHp, posX, posY, direction, flags
|   +-- LeaderConfig: jobType, modelId, worldmapId, level, skills[4], abilities[23], officerCount
|   +-- std::optional<OfficerData> officer1: jobType, modelId, worldmapId, level, raw skills/abilities
|   +-- std::optional<OfficerData> officer2: same
|   +-- UnitConfig: gridX, gridY, troopInfoIndex, formation, statOverrides[22]
|   +-- std::optional<StgUnit> subUnit -- for 1088-byte player units
|   +-- rawUnit (std::array<std::byte, 544>) -- overlay for unknown fields
+-- rawTail (std::vector<std::byte>) -- everything from AreaID section through footer, preserved verbatim
```

Key principle: every struct holds both parsed fields AND the original raw bytes. `save()` starts from raw bytes and patches only known modified fields. This guarantees round-trip fidelity for unmodified data.

### Enums

```cpp
enum class UCD : uint8_t { Player=0, Enemy=1, Ally=2, Neutral=3 };
enum class Direction : uint8_t { East=0, NE=1, North=2, NW=3, West=4, SW=5, South=6, SE=7 };
enum class JobType : uint8_t { /* 0-42, from K2JobDef.h */ };
// AbilityType: 0-56+, -1 (0xFFFFFFFF) = empty slot
```

### Parser Design

Sequential, fail-fast. No error recovery -- if any section is wrong, every subsequent offset is wrong.

1. Read header (628 bytes), extract unitCount from offset 0x270.
2. Read unitCount units (544 bytes each). Detect 1088-byte player units.
3. Store everything from AreaID section to EOF as rawTail blob.
4. Verify consumed bytes + rawTail size == file size.

Validation is post-parse and separate from parsing: unique IDs, enum ranges, officer counts, Worldmap ID checks.

### Crusaders vs Heroes

Simple branching on GameVersion enum. Detect via filename (E####.stg = Crusaders). Heroes deferred -- guard with validation warning. Only two variants exist, so polymorphism/templates are overkill.

### Round-trip Fidelity

The raw byte overlay pattern:

- `load()` populates both raw byte arrays and parsed fields.
- `save()` copies raw bytes, then patches modified fields into the raw buffer.
- `load(save(file)) == file` when no edits were made.

Test this invariant explicitly in the test suite. This is the single most important test.

### Undo/Redo

Existing UndoStack is format-agnostic and needs no changes. New ICommand subclasses:

- **ChangeFieldCommand\<T\>** -- scalar field changes (covers most edits)
- **AddUnitCommand / RemoveUnitCommand** -- insert/remove from vector, store full StgUnit for undo
- **AddOfficerCommand / RemoveOfficerCommand** -- toggle officers
- **CompoundCommand** -- group multiple atomic commands into one undo step

All commands store data by value (not pointers), since vector addresses change on insert/remove.

### Integration

Files to create:

| File | Purpose |
|------|---------|
| `src/formats/stg_format.h` | StgFile, StgHeader, StgUnit structs + StgFormat class |
| `src/formats/stg_format.cpp` | Parser and serializer |
| `src/ui/tabs/stg_editor_tab.h` | StgEditorTab class |
| `src/ui/tabs/stg_editor_tab.cpp` | Editor UI |
| `test/stg_format_test.cpp` | Unit and round-trip tests |

Files to modify:

| File | Change |
|------|--------|
| `src/core/document.h` | Add `std::shared_ptr<StgFormat> stgData`, `isStg()` |
| `src/core/tab_manager.cpp` | Add STG load/save/tab-creation paths |
| `src/core/application.cpp` | Update file dialog filter, status bar, validation navigation |
| `CMakeLists.txt` | Add new source files |

Consideration: OpenDocument currently has SOX-specific fields (binaryData, textData). Adding stgData follows the existing pattern. A future refactor to a generic IFileFormat pointer would be cleaner but is not required for V1.

## UX Design

### Section Navigation

Vertical sidebar within the tab, listing: "Header", "Units (N)", "Areas", "Variables", "Events", "Footer". Clicking a section switches the right panel. Non-unit sections show read-only info or raw hex in V1. Matches the existing left-list + right-detail pattern from TroopEditorTab.

### Unit Editor

Left panel: unit list showing `[ID] UnitName (JobType)`. Color-coded by UCD:

| UCD | Color |
|-----|-------|
| Player | Green |
| Enemy | Red |
| Ally | Blue |
| Neutral | Gray |

Right panel: CollapsingHeader groups for the selected unit:

1. **Core** (default open): name, uniqueId, UCD dropdown, isHero, isEnabled, position X/Y, direction, HP overrides
2. **Leader**: JobType dropdown, modelId, worldmapId, level, skill slots (4 rows), ability slots (23 rows)
3. **Officer 1** (collapsed/disabled when officerCount < 1)
4. **Officer 2** (collapsed/disabled when officerCount < 2)
5. **Unit Config**: grid dimensions, troopInfoIndex, formation, stat overrides

### Enum Presentation

| Enum | Widget | Rationale |
|------|--------|-----------|
| UCD (4 values) | Standard ImGui::Combo | Trivially scannable |
| Direction (8 values) | Standard ImGui::Combo | Small enough |
| JobType (43 values) | Grouped combo by faction (Human/Dark Elf/Dark Orc/Special) | Needs grouping to be usable |
| AbilityType (57+ values) | Grouped combo with text filter | Too large without search |

All enum fields display `EnumName (decimal_value)`. Right-click allows direct numeric entry for power users who know hex values from hex editor workflows.

AbilityType -1 (0xFFFFFFFF) displays as "(Empty)" and is the first option.

### Stat Overrides

The 22 float stat override fields show "Default" (grayed out) when value is -1.0 (0xBF800000). A checkbox next to each toggles between default and override mode. This prevents accidental modification of sentinel values.

### Officer Sections

Officer sections are grayed out/disabled when officer count is 0 or 1. Officer count displayed as a DragInt at the top of the Leader section. Changing officer count from 0 to 1 auto-expands the Officer 1 header.

## Testing Strategy

### Synthetic tests (primary)

- `createMinimalStg()`: header + 1 unit + empty rawTail
- Round-trip: load, save, compare byte-for-byte (**critical test**)
- Header field parsing: mission ID, filenames
- Unit field parsing: name, position, JobType, UCD, officers
- Validation: duplicate IDs detected, invalid enums flagged, officer count range

### Real file tests (integration, tagged [.integration])

- Load actual game STG files (e.g., E1001.stg, E1100.stg), save, diff
- Skip when test data not present
- Test against files with varying unit counts and compositions

### Edge cases

- 0 units (empty mission)
- Maximum officers (2)
- Player units with 1088-byte sub-unit blocks
- Truncated files
- Invalid section counts

## Pre-implementation Research

These must be resolved before implementation begins:

1. **1088-byte double block detection**: How does the game determine which units have sub-units? Is it UCD == Player? A flag? Analyze multiple mission files.
2. **Variable entry format**: Collect STG files, hex-dump variable sections, confirm entry structure and sizes.
3. **Stat override field mapping**: Which of the 22 floats maps to which stat? Compare with TroopInfo.sox field order.

## Future Work (Post-V1)

| Feature | Phase | Dependency |
|---------|-------|------------|
| AreaID editing | v1.1 | Verify 84-byte entry structure |
| 2D map visualization | v1.1 | AreaID editing + bitmap loading |
| Event read-only display | v1.1 | Build condition/action catalog (MISSION_SCRIPTING.md) |
| Event editing | v2 | Complete condition/action catalog with parameter signatures |
| Variable editing | v2 | Verify variable entry format |
| Heroes support | v2 | Verify 548-byte blocks and Heroes AreaID layout |
| Briefing file support | v2 | Separate analysis needed |
| Cross-file validation | v2 | STG references to TroopInfo.sox and text files |
