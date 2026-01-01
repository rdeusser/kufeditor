# Mod Manager Design

## Overview

A mod management system for KUFEditor that allows users to create, install, order, and apply mods to Kingdom Under Fire game files.

## Mod Format

Mods are distributed as `.kufmod` files (ZIP archives) containing:

```
my-mod.kufmod
├── mod.json       # Manifest with metadata and patches
└── assets/        # Optional: additional assets
```

### mod.json Schema

```json
{
  "id": "balanced-warriors",
  "name": "Balanced Warriors",
  "version": "1.0.0",
  "author": "ModderName",
  "description": "Rebalances warrior unit stats for better gameplay",
  "game": "Crusaders",
  "patches": [
    {
      "file": "TroopInfo.sox",
      "action": "modify",
      "record": "Gerald",
      "fields": { "HP": 500, "Attack": 75 }
    },
    {
      "file": "TroopInfo.sox",
      "action": "add",
      "data": { "Name": "Elite Guard", "HP": 800, "Attack": 100 }
    },
    {
      "file": "SkillInfo.sox",
      "action": "delete",
      "record": "Unused Skill"
    }
  ]
}
```

### Patch Actions

- **modify**: Change specific fields on an existing record (name-based lookup)
- **add**: Add a new record to the file
- **delete**: Remove a record from the file

## Mod Manager State

Stored in `mods.json` in app data directory:

```json
{
  "Crusaders": ["balanced-warriors", "extra-units"],
  "Heroes": ["hero-buffs"]
}
```

- Array order = load order
- Presence in array = enabled
- Absence = disabled (mod file still exists)

## Apply Process

1. Restore all modified files from pristine backups
2. For each enabled mod in order:
   - Parse the target SOX file
   - Apply each patch (modify/add/delete)
   - Track which fields were touched and by which mod
3. Write modified files to game directory
4. Report any conflicts (same field touched by multiple mods)

## Conflict Detection

Conflicts occur when two mods modify the exact same field on the same record. The applier tracks:

```
mod-id → file → record → field
```

When a conflict is detected, the later mod in order wins, but the conflict is reported to the user.

## Components

### KUFEditor.Core

- `Mod` - Data model for mod metadata and patches
- `ModPatch` - Individual patch operation
- `ModManager` - Load/save state, enumerate installed mods
- `ModApplier` - Apply patches with conflict detection

### KUFEditor (UI)

- `ModManagerWindow` - Main mod management dialog
- Tools menu integration

## UI Layout

```
┌─────────────────────────────────────────────────────────────┐
│ Mod Manager                                           [X]   │
├─────────────────────────────────────────────────────────────┤
│ [Crusaders ▼]                    [Import] [Create] [Apply]  │
├──────────────────────────┬──────────────────────────────────┤
│ ☑ Balanced Warriors   ▲  │  Balanced Warriors v1.0.0        │
│ ☑ Extra Units         ▼  │  by ModderName                   │
│ ☐ Hero Buffs             │                                  │
│                          │  Rebalances warrior unit stats   │
│                          │  for better gameplay.            │
│                          │                                  │
│                          │  Files modified:                 │
│                          │  - TroopInfo.sox (3 records)     │
│                          │  - SkillInfo.sox (1 record)      │
│                          │                                  │
│                          │  [Delete Mod]                    │
├──────────────────────────┴──────────────────────────────────┤
│ ⚠ 2 conflicts detected                              [View]  │
└─────────────────────────────────────────────────────────────┘
```

## Integration Points

- **BackupManager**: Used for pristine file restoration
- **SOX Parsers**: Used for field-level patching (TroopInfoSoxFile, etc.)
- **Settings**: Mods directory path stored in settings

## Future Considerations (Not In Scope)

- Mod dependencies
- Mod versioning/updates
- Nexus Mods integration
- Mod profiles (save different mod configurations)
