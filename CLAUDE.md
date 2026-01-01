# CLAUDE.md

Guidance for Claude Code when working with this repository.

## Project Overview

KUFEditor is a Kingdom Under Fire editor for Crusaders and Heroes. Modifies map files, SOX files, and other game resources. Built with Avalonia UI and .NET 9.

## Build Commands

```bash
dotnet build                           # Build solution
dotnet build -c Release                # Release build
dotnet run --project src/KUFEditor     # Run application
dotnet clean                           # Clean artifacts
```

## Solution Structure

```
src/
├── KUFEditor/           # Main Avalonia UI application
│   ├── UI/
│   │   ├── KUFEditor.axaml(.cs)           # Main window
│   │   ├── Dialogs/                        # Modal dialogs
│   │   └── Views/                          # UI components
│   ├── KUFEditorApp.axaml(.cs)            # Application entry
│   └── Program.cs
├── KUFEditor.Core/      # Settings, backup management, interfaces
├── KUFEditor.Maps/      # Map data structures
└── KUFEditor.Assets/    # SOX file handling (TroopInfo, etc.)
```

## Key Patterns

- **No MVVM**: Logic lives in code-behind files, not ViewModels.
- **Ryujinx naming**: `KUFEditorApp` (not App), `UI/KUFEditor` (not MainWindow).
- **Namespace**: Views use `KUFEditor.UI.Views`.
- **Windows-only**: The game is Windows-only, so the editor targets Windows.

## File Format Documentation

See [docs/FILE_FORMATS.md](docs/FILE_FORMATS.md) for detailed specifications of:
- Binary SOX files (TroopInfo.sox, SkillInfo.sox, ExpInfo.sox)
- Text SOX files (ItemTypeInfo_ENG.sox)
- Mission files (.stg) with troop deployment data
- Navigation files (.nav)
- 3D model files (.K2A)
- Save game files

## Adding a New SOX File Editor

1. Create data structures in `src/KUFEditor.Assets/`
2. Implement binary reader/writer (see `TroopInfoSoxFile.cs`)
3. Create editor view in `src/KUFEditor/UI/Views/`
4. Update `EditorArea.CreateSoxEditor()` to detect the file type

## UI Design Notes

- Soft shadows instead of hard borders
- Rounded corners (12px panels, 8px cards)
- Opacity-based visual hierarchy
- `ClipToBounds="False"` on containers to prevent clipping
- No emojis or unicode symbols in UI - use Avalonia/Fluent icons
- Use `StorageProvider` API for file dialogs (not deprecated `SaveFileDialog` etc.)
- Avoid control names that conflict with inherited properties (e.g., don't use `NameProperty`)
