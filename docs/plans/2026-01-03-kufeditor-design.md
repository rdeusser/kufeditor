# KUF Editor Design Document

Technical design for a Kingdom Under Fire: The Crusaders/Heroes file editor.

## Overview

**Target Users:** Modding community (power users creating game mods)

**Scope (v1.0):**
- Binary SOX files (TroopInfo.sox, SkillInfo.sox, ExpInfo.sox)
- Mission files (.stg)
- Text SOX files (localization)
- Save game files

**Scope (v2.0):**
- Mod Manager with delta-based mods and conflict detection

**Out of Scope:**
- NAV files (poorly documented pathfinding data)
- K2A model archives (poorly documented)

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| GUI | Dear ImGui (docking branch) | Battle-tested, ImHex uses same stack |
| Window | GLFW 3 | Cross-platform, simple API |
| Render | OpenGL 3.3 | Widely supported, matches ImGui examples |
| JSON | nlohmann/json | Settings, registry files |
| Images | stb_image | Icon loading |
| Build | CMake 3.20+ | FetchContent for dependencies |
| Compiler | MSVC (VS 2022) | Windows-only target |
| Standard | C++20 | std::span for safe buffer handling |

## Project Structure

```
kufeditor/
├── src/
│   ├── core/
│   │   ├── application.cpp      # Main loop, window management
│   │   ├── settings.cpp         # User preferences
│   │   └── events.cpp           # Event system
│   ├── formats/
│   │   ├── file_format.h        # IFileFormat interface
│   │   ├── sox_binary.cpp       # Binary SOX parser
│   │   ├── sox_text.cpp         # Text SOX parser
│   │   ├── mission.cpp          # STG parser
│   │   └── savegame.cpp         # Save file parser
│   ├── ui/
│   │   ├── views/
│   │   │   ├── view.h           # Base View class
│   │   │   ├── troop_editor.cpp
│   │   │   ├── mission_editor.cpp
│   │   │   ├── text_sox_editor.cpp
│   │   │   └── save_editor.cpp
│   │   ├── widgets/
│   │   │   ├── resistance_bar.cpp
│   │   │   ├── enum_dropdown.cpp
│   │   │   └── coordinate_input.cpp
│   │   └── dialogs/
│   │       ├── file_picker.cpp
│   │       ├── about.cpp
│   │       └── backup_prompt.cpp
│   ├── backup/
│   │   └── backup_manager.cpp
│   └── undo/
│       ├── command.h            # ICommand interface
│       └── undo_stack.cpp
├── external/                    # Vendored dependencies
├── resources/                   # Icons, fonts
├── test/
│   ├── data/                    # Sample game files
│   └── ...
└── docs/
```

## Architecture

### Document-Based Model

Each open file is a "document" with:
- Parsed file format instance (`IFileFormat*`)
- Undo stack (`UndoStack`)
- Modification state (dirty flag)
- Editor view instance

Multiple documents open simultaneously in tabbed interface.

### File Format Interface

```cpp
enum class GameVersion { Crusaders, Heroes, Unknown };

class IFileFormat {
public:
    virtual ~IFileFormat() = default;

    virtual bool load(std::span<const std::byte> data) = 0;
    virtual std::vector<std::byte> save() const = 0;
    virtual std::string_view formatName() const = 0;
    virtual GameVersion detectedVersion() const = 0;
    virtual std::vector<ValidationIssue> validate() const = 0;
};
```

### Auto-Detection

1. Check file extension
2. For STG: Read UCD field size (4 bytes = Crusaders, 1 byte = Heroes)
3. For SOX: Parse header, validate against known structures

### UI Layout

```
┌─────────────────────────────────────────────────────────────┐
│  Menu Bar (File, Edit, View, Help)                          │
├───────────┬─────────────────────────────────────────────────┤
│           │  Tab Bar (open documents)                        │
│  File     ├─────────────────────────────────────────────────┤
│  Browser  │                                                  │
│           │           Active Editor View                     │
│  (dock-   │     (format-specific: table, form, tree)        │
│   able)   │                                                  │
│           ├─────────────────────────────────────────────────┤
│           │  Validation Log (warnings, errors)               │
├───────────┴─────────────────────────────────────────────────┤
│  Status Bar (file path, game version, modification state)    │
└─────────────────────────────────────────────────────────────┘
```

### Editor Views

| View | Format | Widget Style |
|------|--------|--------------|
| TroopEditorView | TroopInfo.sox | Table with inline editing, resistance bars |
| MissionEditorView | *.stg | Tree for troops + property panel |
| TextSoxEditorView | Text SOX | Table with text fields, length indicators |
| SaveEditorView | Save files | Tabbed per-character, form fields |

### Custom Widgets

- **ResistanceBar** - Visual 0-2.0 resistance (red/green gradient)
- **EnumDropdown** - Maps int values to readable names
- **CoordinateInput** - X/Y/Facing with visual preview
- **ValidationBadge** - Warning/error indicator

## BackupManager

### First-Run Workflow

1. Dialog: "Select your KUF game installation"
2. Auto-detect Crusaders vs Heroes from directory structure
3. Dialog: "Create backup of game files? (Recommended)"
4. If yes: Copy editable directories to `%APPDATA%/KUFEditor/vanilla/{crusaders|heroes}/`
5. If no: Show warning, continue without backup

### Storage

```
%APPDATA%/KUFEditor/
├── vanilla/
│   ├── crusaders/
│   │   ├── Data/SOX/
│   │   ├── Data/Mission/
│   │   └── ...
│   └── heroes/
│       └── ...
├── registry.json
└── settings.json
```

### API

```cpp
class BackupManager {
public:
    bool hasVanillaCopy(GameVersion) const;
    void createVanillaCopy(const std::filesystem::path& gameDir);
    std::filesystem::path vanillaPath(GameVersion, const std::string& relativePath);
    void restoreFile(const std::filesystem::path& target);
    bool isModified(const std::filesystem::path& target);
};
```

### v2.0 Hooks

- `isModified()` returns diff structure for Mod Manager
- File hashes enable conflict detection
- Storage organized by game version

## Undo/Redo System

### Command Pattern

```cpp
class ICommand {
public:
    virtual ~ICommand() = default;
    virtual void execute() = 0;
    virtual void undo() = 0;
    virtual std::string description() const = 0;
};

class UndoStack {
    std::vector<std::unique_ptr<ICommand>> history_;
    size_t position_ = 0;

public:
    void execute(std::unique_ptr<ICommand> cmd);
    void undo();
    void redo();
    bool canUndo() const;
    bool canRedo() const;
};
```

### Per-Document Stacks

Each document has its own UndoStack. Ctrl+Z affects active document only.

### Command Types

- `SetFieldCommand<T>` - Change field value (stores old + new)
- `AddTroopCommand` - Add troop to STG
- `RemoveTroopCommand` - Remove troop from STG

### Limits

100 operations per document (configurable).

## Validation System

### Issue Structure

```cpp
enum class Severity { Info, Warning, Error };

struct ValidationIssue {
    Severity severity;
    std::string field;
    std::string message;
    size_t recordIndex;
};
```

### Known Validations

| Check | Description |
|-------|-------------|
| Float overflow | Values outside safe range (80BF Extra Stats Block issue) |
| Invalid IDs | Skill/job/formation IDs not in valid range |
| Text length | Text SOX field length != declared prefix |
| Missing refs | STG referencing non-existent troop indices |

## Testing Strategy

### Unit Tests

- Parser tests: Load known-good files, verify field values
- Round-trip tests: Load → save → load → compare bytes
- Validation tests: Load broken files, expect warnings

### Test Data

Include sample files in `test/data/`:
- Valid files from Crusaders and Heroes
- Intentionally corrupted files for validation testing

## Roadmap

### v1.0

- [ ] Core application framework (window, docking, theming)
- [ ] Binary SOX parser and editor
- [ ] Text SOX parser and editor
- [ ] Mission file parser and editor
- [ ] Save game parser and editor
- [ ] BackupManager with first-run prompt
- [ ] Undo/redo system
- [ ] Validation warnings

### v2.0

- [ ] Mod Manager
- [ ] Delta-based mod format
- [ ] Mod conflict detection
- [ ] Mod load ordering
