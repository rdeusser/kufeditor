# KUFEditor UI Redesign

## Overview

Restructure KUFEditor to support editing all Kingdom Under Fire file formats with a workspace-based navigation system and robust backup/restore functionality.

## UI Structure

### Main Layout (3 panels)

```
┌─────────────────────────────────────────────────────────────────┐
│  [Crusaders ▼]                                          [≡]    │
├──────────────┬────────────────────────────────┬─────────────────┤
│              │                                │                 │
│  WORKSPACE   │        EDITOR TABS             │   INFO PANEL    │
│  NAVIGATOR   │                                │   (collapsible) │
│              │  ┌─────┬─────┬─────┐          │                 │
│  ▼ SOX Files │  │Tab 1│Tab 2│ ... │          │  File: xxx.sox  │
│    TroopInfo │  └─────┴─────┴─────┘          │  Size: 12 KB    │
│    SkillInfo │                                │  Modified: ...  │
│    ExpInfo   │  ┌──────────────────┐          │                 │
│              │  │                  │          │  ── Snapshots ──│
│  ▼ Missions  │  │  Editor Content  │          │  • Before nerf  │
│    H0000.stg │  │                  │          │  • Working v2   │
│    H0001.stg │  │                  │          │  [Restore]      │
│              │  │                  │          │                 │
│  ▼ Saves     │  └──────────────────┘          │  [New Snapshot] │
│    slot1.sav │                                │                 │
│              │                                │                 │
├──────────────┴────────────────────────────────┴─────────────────┤
│  Ready                                          Memory: 45 MB   │
└─────────────────────────────────────────────────────────────────┘
```

### Workspace Navigator (Left Panel)

- **Game Selector** — Dropdown at top to switch between Crusaders/Heroes
- **Categorized Tree** — Files organized by type:
  - SOX Files (Data/SOX/)
  - Missions (Data/Mission/)
  - Save Games (Documents/KUF2 {Game}/)
- **Search** — Filter files within current workspace

### Editor Area (Center)

- **Standard tabs** — Each file opens in its own tab
- **Unsaved indicator** — Dot or asterisk on modified tabs
- **Close button** — Per-tab, with save prompt if unsaved
- **Type-specific editors** — Appropriate editor based on file type

### Info Panel (Right, Collapsible)

- **File metadata** — Path, size, last modified, format type
- **Snapshot list** — Named snapshots for this file
- **Restore button** — Restore selected snapshot
- **New Snapshot button** — Create named snapshot
- **Collapse toggle** — Hide panel to maximize editor space

## Backup System

### Three Tiers

1. **Pristine Originals**
   - Captured once when game path is first configured
   - Stored in: `{BackupRoot}/{Game}/pristine/`
   - Never modified after initial capture
   - Used as "factory reset" restore point

2. **Named Snapshots**
   - User-created with custom names
   - Stored in: `{BackupRoot}/{Game}/snapshots/{filename}/{snapshot-name}/`
   - Preserved indefinitely until user deletes
   - Displayed in Info Panel for quick restore

3. **No auto-backups** — User is in control

### Backup Directory Structure

```
KUFBackup/
├── Crusaders/
│   ├── pristine/
│   │   ├── TroopInfo.sox
│   │   ├── SkillInfo.sox
│   │   └── ...
│   └── snapshots/
│       ├── TroopInfo.sox/
│       │   ├── Before balance changes/
│       │   │   └── TroopInfo.sox
│       │   └── Working state/
│       │       └── TroopInfo.sox
│       └── H0000.stg/
│           └── Original mission/
│               └── H0000.stg
└── Heroes/
    ├── pristine/
    └── snapshots/
```

## Editors

### Priority Order

1. **Mission Editor (.stg)**
   - Troop list with add/remove/reorder
   - Per-troop editing: IDs, allegiance, HP, skills, position
   - Officer configuration
   - Briefing data editing
   - Future: Visual placement on 2D map

2. **Save Game Editor**
   - Character selection
   - Troop configuration (job, equipment, skills)
   - Level/XP editing
   - Progress flags

3. **Text SOX Editor**
   - Fixed-width field editing with length validation
   - Hex view toggle for byte-level editing
   - Character count display

4. **Binary SOX Editors**
   - SkillInfo.sox — Skill definitions, level caps
   - ExpInfo.sox — Experience tables
   - Pattern follows existing TroopInfoEditor

5. **Navigation Editor (.nav)** — Lower priority, format less documented

## Implementation Phases

### Phase 1: UI Restructure
- Convert FileExplorer to WorkspaceNavigator
- Add game selector dropdown
- Implement categorized file tree
- Remove Properties Panel
- Add collapsible Info Panel
- Wire up tab system improvements

### Phase 2: Backup System
- Implement pristine capture on first game path setup
- Add named snapshot creation/storage
- Build snapshot restore functionality
- Update Info Panel with snapshot UI

### Phase 3: Mission Editor
- Define .stg data structures in KUFEditor.Assets
- Implement binary reader/writer
- Build MissionEditor view
- Add to EditorArea detection

### Phase 4: Save Game Editor
- Define save file data structures
- Implement reader/writer
- Build SaveGameEditor view

### Phase 5: Remaining Editors
- Text SOX Editor
- SkillInfo.sox Editor
- ExpInfo.sox Editor
- Navigation Editor (if needed)

## Design Decisions

- **No MVVM** — Logic in code-behind, consistent with existing pattern
- **Incremental refactor** — Keep app functional throughout
- **Workspace concept** — One game active at a time, cleaner UX
- **User-controlled backups** — No automatic saves, explicit snapshots only
