# Ryujinx UI Design Specification

This document describes the user interface design of Ryujinx, a Nintendo Switch emulator. The information provided is comprehensive enough to reimplement the UI in a different framework or programming language.

## Table of Contents

1. [Framework Overview](#framework-overview)
2. [Color System](#color-system)
3. [Typography](#typography)
4. [Window Structure](#window-structure)
5. [Main Window Layout](#main-window-layout)
6. [UI Components](#ui-components)
7. [Icons and Visual Assets](#icons-and-visual-assets)
8. [Settings Navigation](#settings-navigation)
9. [Game Library Views](#game-library-views)
10. [Status Indicators](#status-indicators)
11. [Dialogs and Modals](#dialogs-and-modals)
12. [Context Menus](#context-menus)
13. [Keyboard Shortcuts](#keyboard-shortcuts)

---

## Framework Overview

Ryujinx uses **Avalonia UI** with the **FluentAvalonia** extension for a modern, cross-platform desktop experience.

### Key Characteristics
- **Design Language**: Fluent Design System (Microsoft-inspired)
- **Pattern**: MVVM (Model-View-ViewModel)
- **Theming**: Light/Dark/Auto with user accent color support
- **Localization**: Full internationalization with RTL language support

---

## Color System

### Theme Structure

Ryujinx supports three themes: **Default** (system), **Light**, and **Dark**. Colors adapt based on the active theme.

### Color Definitions

#### Light Theme Colors

| Color Key | Hex Value | Usage |
|-----------|-----------|-------|
| `ThemeContentBackgroundColor` | `#DEDEDE` | Main content area background |
| `ThemeControlBorderColor` | `#C2C2C2` | Control borders, separators |
| `ThemeForegroundColor` | `#000000` | Primary text color |
| `MenuFlyoutPresenterBorderColor` | `#C1C1C1` | Menu/flyout borders |
| `AppListBackgroundColor` | `#B3FFFFFF` | Game list item background (70% white) |
| `AppListHoverBackgroundColor` | `#80CCCCCC` | Game list item hover (50% gray) |
| `SecondaryTextColor` | `#A0000000` | Secondary/muted text (63% black) |
| `DataGridSelectionColor` | `#00FABB` | Selected row highlight (cyan-green) |
| `ControlFillColorSecondary` | `#3DDCFF` | Secondary control fill (light cyan) |

#### Dark Theme Colors

| Color Key | Hex Value | Usage |
|-----------|-----------|-------|
| `ThemeContentBackgroundColor` | `#2D2D2D` | Main content area background |
| `ThemeControlBorderColor` | `#505050` | Control borders, separators |
| `ThemeForegroundColor` | `#FFFFFF` | Primary text color |
| `MenuFlyoutPresenterBorderColor` | `#3D3D3D` | Menu/flyout borders |
| `AppListBackgroundColor` | `#0FFFFFFF` | Game list item background (6% white) |
| `AppListHoverBackgroundColor` | `#1EFFFFFF` | Game list item hover (12% white) |
| `SecondaryTextColor` | `#A0FFFFFF` | Secondary/muted text (63% white) |
| `ControlFillColorSecondary` | `#008AA8` | Secondary control fill (teal) |

### Semantic Colors (Theme-Independent)

| Color Key | Hex Value | Usage |
|-----------|-----------|-------|
| `FavoriteApplicationIconColor` | `#FCD12A` | Favorite star icon (gold) |
| `WarningBackgroundColor` | `#FF6347` | Warning/error backgrounds (tomato) |
| `Switch` | `#2EEAC9` | Switch-related indicator (cyan) |
| `Unbounded` | `#FF4554` | Unbounded/unlimited indicator (red) |
| `Custom` | `#6483F5` | Custom setting indicator (blue) |
| `CustomConfig` | `#00B5B8` | Custom game configuration (teal) |
| `Warning` | `#800080` (light) / `#FFA500` (dark) | Warning indicator |

### Game Compatibility Status Colors

| Status | Color | Brush Name |
|--------|-------|------------|
| Unknown/Nothing | `DarkGray` | `Brushes.DarkGray` |
| Boots | `Red` | `Brushes.Red` |
| Menus | `Tomato` | `Brushes.Tomato` |
| In-Game | `Orange` | `Brushes.Orange` |
| Playable | `LimeGreen` | `Brushes.LimeGreen` |

---

## Typography

### Font Families

| Font | Usage |
|------|-------|
| **System Default** | General UI text |
| **JetBrains Mono** | Monospace text (code, logs, hex values) |
| **Segoe Fluent Icons** | System icons and glyphs |
| **FontAwesome** | Action icons in menus |
| **Material Design Icons** | Additional iconography |

### Font Sizes

| Size Key | Pixels | Usage |
|----------|--------|-------|
| `FontSizeSmall` | 8px | Tiny labels |
| `FontSizeNormal` | 10px | Small text |
| `FontSize` (default) | 12px | Standard body text |
| `ControlContentThemeFontSize` | 13px | Control labels |
| `FontSizeLarge` | 15px | Emphasized text |
| `h1` style | 16px | Section headers (bold) |
| Title bar | 14px | Window title |
| Loading heading | 30px | Game loading screen title (bold) |
| Loading status | 18px | Game loading screen status |

### Text Styles

- **Default text**: Wraps with overflow, vertically centered
- **Bold headers**: Used for game names, section titles
- **Secondary text**: Uses `SecondaryTextColor` for less important info

---

## Window Structure

### Window Dimensions

| Window | Min Width | Min Height | Default Width | Default Height |
|--------|-----------|------------|---------------|----------------|
| Main Window | 800px | 500px | 1280px | 720px |
| Settings Window | 844px | 480px | 1100px | — |

### Window Components

All windows use a custom `StyleableAppWindow` or `StyleableWindow` base class that provides:

- Custom title bar integration
- Theme switching support
- Locale/RTL support
- Consistent styling

### Title Bar

- Height: ~35px (includes logo and menu)
- Contains: App logo (25x25px), main menu bar
- Fullscreen style: Black background (`#000000`)

---

## Main Window Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ [Logo] │ File │ Options │ Actions │ View │ Help │              │  ← Menu Bar (35px min)
├─────────────────────────────────────────────────────────────────┤
│ [List] [Grid] [─────Size─────] □ Show Names    [Sort ▾] [🔍]   │  ← View Controls (35px)
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│                                                                 │
│                    Game Library Area                            │  ← Content Area (flexible)
│                 (List View or Grid View)                        │
│                                                                 │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│ [🔄] Games: X │ Progress... │ Time Played │ VSync │ Dock │ ... │  ← Status Bar (22px min)
└─────────────────────────────────────────────────────────────────┘
```

### Loading Screen Overlay

When a game is loading, an overlay appears with:

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│    ┌──────────┐                                                 │
│    │          │   Game Title (30px, bold)                       │
│    │  Game    │   ┌────────────────────────────────────┐        │
│    │  Icon    │   │ Progress Bar (10px height)         │        │
│    │ 256x256  │   └────────────────────────────────────┘        │
│    │          │   Loading status text (18px)                    │
│    └──────────┘                                                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

- Game icon: 256x256px with 2px black border, 4-8px shadow, 3px corner radius
- Progress bar: 500px min width, 5px corner radius
- Content centered in window with 40px margin

---

## UI Components

### Buttons

| Style | Min Width | Usage |
|-------|-----------|-------|
| Default | 80px | Standard buttons |
| Icon button | 25-40px | Toolbar buttons (transparent background) |
| Red/Danger | — | Destructive actions (red background, white text) |
| Accent | — | Primary actions (uses system accent color) |

### Menu Items

- Height: 26px (`MenuItemHeight`)
- Font size: 12px
- Padding: 5px horizontal
- Icons: FontAwesome solid icons (via `{ext:Icon fa-solid fa-*}`)
- Checkbox menus: 36x36px checkbox area

### Data Grid

- Column headers: Centered text, themed background, 5px padding
- Sort icon: 8px wide triangle
- Row selection: Uses `DataGridSelectionColor` with opacity
- Cell font size: 14px
- Cells: Center-aligned by default

### ListBox Items

- Padding: 0
- Corner radius: 5px
- Background: `AppListBackgroundColor`
- Border thickness: 2px
- Hover: `AppListHoverBackgroundColor`
- Margin between items: 2px (list) / 5px (grid)

### Progress Bar

- Height: 6-10px
- Corner radius: 5px
- No visible track (hidden)
- Foreground: `SystemAccentColorLight2`

### Sliders

- Tick frequency: Configurable per slider
- Snap to tick: Enabled for precision
- Used for: Volume, VSync interval, grid size scale

### Text Boxes

- Vertical content alignment: Center
- Watermark support for search fields

### Separators

- Background/Foreground: `ThemeControlBorderColor`
- Min height: 1px
- Mini vertical separator used in status bar

---

## Icons and Visual Assets

### Icon Libraries

1. **FontAwesome (Solid)**: Primary action icons
   - Play, pause, stop, gear, folder, file, etc.
   - Used via `{ext:Icon fa-solid fa-iconname}`

2. **FluentAvalonia Symbols**: System icons
   - List, Grid, Star, Settings, Refresh, etc.
   - Used via `<ui:SymbolIcon Symbol="SymbolName" />`

3. **Segoe Fluent Icons**: Custom glyphs
   - Chip (U+E954), Device (U+E7F7), Bug (U+EBE8)

### Controller Icons (SVG)

| File | Description |
|------|-------------|
| `Controller_JoyConLeft.svg` | Left Joy-Con outline |
| `Controller_JoyConRight.svg` | Right Joy-Con outline |
| `Controller_JoyConPair.svg` | Paired Joy-Cons |
| `Controller_ProCon.svg` | Pro Controller |

### File Type Icons (PNG)

| File | Description |
|------|-------------|
| `Icon_Blank.png` | Placeholder/unknown file |
| `Icon_NCA.png` | NCA file type |
| `Icon_NRO.png` | NRO file type |
| `Icon_NSO.png` | NSO file type |
| `Icon_NSP.png` | NSP file type |
| `Icon_XCI.png` | XCI file type |

### Logo Assets (PNG)

| File | Size | Description |
|------|------|-------------|
| `Logo_Ryujinx.png` | 259KB | Full resolution logo |
| `Logo_Ryujinx_AntiAlias.png` | 12KB | Anti-aliased version for small sizes |
| `Logo_Amiibo.png` | 11KB | Amiibo feature logo |
| `Logo_Discord_Dark.png` | 10KB | Discord logo (dark theme) |
| `Logo_Discord_Light.png` | 11KB | Discord logo (light theme) |
| `Logo_GitLab_Dark.png` | 4KB | GitLab logo (dark theme) |
| `Logo_GitLab_Light.png` | 5KB | GitLab logo (light theme) |

### Application Logo

- Display size: 25x25px
- Margin: 7px (top, left, right), 0 bottom
- Uses anti-aliased variant to prevent border crunching at small sizes

---

## Settings Navigation

The settings window uses a `NavigationView` pattern with a left sidebar.

### Navigation Structure

| Tab | Icon | Tag |
|-----|------|-----|
| General | Device glyph (U+E7F7) | UiPage |
| Input | Games symbol | InputPage |
| System | Settings symbol | SystemPage |
| CPU | Chip glyph (U+E954) | CpuPage |
| Graphics | Image symbol | GraphicsPage |
| Audio | Audio symbol | AudioPage |
| Hotkeys | Keyboard symbol | HotkeysPage |
| Network | Globe symbol | NetworkPage |
| Logging | Document symbol | LoggingPage |
| Debug | Bug glyph (U+EBE8) | DebugPage |
| Dirty Hacks | Code symbol | HacksPage (hidden by default) |

### Navigation Pane

- Open pane length: 200px
- Placeholder grid height: 40px
- Icons flow left-to-right regardless of RTL

### Settings Footer

```
┌────────────────────────────────────────────────────────────────┐
│ [Reset] □ Confirm Reset          [OK] [Cancel] [Apply]         │
└────────────────────────────────────────────────────────────────┘
```

- Reset button: Disabled until confirmation checkbox is checked
- Button order: Reversed on macOS (OK/Cancel/Apply → Apply/Cancel/OK)
- Escape key: Bound to Cancel
- OK button: Uses `accent` class (primary styling)

---

## Game Library Views

### List View

Each game entry displays:

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ★ ⚙                                                                      │
│ ┌────────┐                                                               │
│ │        │  Game Name (bold)                    Title ID        Last Played   │
│ │  Icon  │  Developer                           File Extension  Time Played   │
│ │ 50-120 │  Version                             [Multiplayer]   File Size     │
│ │   px   │  [Playability Status]                Custom Config                 │
│ └────────┘                                                               │
└──────────────────────────────────────────────────────────────────────────┘
```

#### Icon Sizes (List View)

| Size Class | Width |
|------------|-------|
| small | 50px |
| normal | 80px |
| large | 100px |
| huge | 120px |

#### Visual Indicators

- **Favorite star**: Top-left, gold (`FavoriteApplicationIconColor`), StarFilled symbol, 18px
- **Custom config gear**: Top-right, teal (`CustomConfig`), SettingsFilled symbol, 18px

### Grid View

```
┌─────────────┐
│ ★         ⚙│
│             │
│   [Icon]    │
│             │
│             │
├─────────────┤
│  Game Name  │  ← 50px height, shown if "Show Names" checked
└─────────────┘
```

#### Grid Sizes

| Size Class | Border Width |
|------------|--------------|
| small | 100px |
| normal | 130px |
| large | 160px |
| huge | 200px |

- Item margin: 5px
- Corner radius: 4px
- Favorite/config icons: 23px font size

### View Controls Bar

- List/Grid toggle buttons: 40x40px
- Size slider: 150px wide, range 1-4, snaps to ticks
- "Show Names" checkbox: Only visible in grid mode
- Sort dropdown: 150px wide
- Search box: 200px min width

### Sort Options

- Favorite
- Application (Title)
- Title ID
- Developer
- Time Played
- Last Played
- File Extension
- File Size
- Path

Order: Ascending / Descending

---

## Status Indicators

### Status Bar Layout (Game Not Running)

```
[🔄] Games: X loaded │ [Progress Bar] │ Total Time Played │ [Update Button] │ Firmware: X.X.X
```

### Status Bar Layout (Game Running)

```
VSync: ON/OFF │ Docked/Handheld │ Aspect Ratio │ Volume │ FPS/Game Status │ FIFO │ Shaders │ GPU │ Backend
```

### VSync Indicator

- Click to cycle through modes
- Color indicates current state (bound to `VSyncModeColor`)
- Custom interval picker: Slider 10-400%, flyout placement top

### Dock Mode Indicator

- Toggleable via click or F9
- Text: "Docked" / "Handheld"

### Aspect Ratio Control

- SplitButton with dropdown
- Options: 4:3, 16:9, 16:10, 21:9, 32:9, Stretched
- Tooltip: "AspectRatioTooltip" localization key

### Volume Control

- ToggleSplitButton (click to mute/unmute)
- Flyout: Slider 0.0-1.0, 150px wide, tick frequency 0.05
- Supports mouse wheel adjustment

### Update Available Button

- Background: System accent color
- Text: Foreground uses `SystemColorButtonText`
- Only visible when update available and game not running

---

## Dialogs and Modals

### Content Dialog

- Max width: 700px
- Max height: 756px

### Common Dialog Patterns

1. **Confirmation dialogs**: Reset settings, delete operations
2. **Manager dialogs**: Mods, DLC, Title Updates, Cheats
3. **Applet dialogs**: Controller selection, software keyboard, profile selector, error display

### Navigation Dialog Host

Used for multi-step dialogs (user profiles, firmware avatar selection).

---

## Context Menus

### Game Context Menu Structure

```
Run Application                    [fa-play]
Toggle Favorite                    [fa-star]
Create Shortcut                    [fa-bookmark]
Edit/Create Custom Configuration   [fa-gear]
Show Compatibility Entry           [fa-database]
Show Game Data                     [fa-chart-line]
─────────────────────────────────────────────
Open User Save Directory           [fa-sd-card]
Open Device Save Directory         [fa-hard-drive]
Open BCAT Save Directory           [fa-box-archive]
─────────────────────────────────────────────
Manage Title Updates               [fa-diagram-predecessor]
Manage DLC                         [fa-puzzle-piece]
Manage Cheats                      [fa-code]
Manage Mods                        [fa-sliders]
─────────────────────────────────────────────
Open Mods Directory                [fa-folder-closed]
Open SD Mods Directory             [fa-folder-closed]
─────────────────────────────────────────────
Trim XCI                           [fa-scissors]
Cache Management ►                 [fa-memory]
  ├─ Purge PPTC                    [fa-arrow-rotate-right]
  ├─────────────────────────────
  ├─ Nuke PPTC                     [fa-trash-can]
  ├─ Purge Shader Cache            [fa-trash-can]
  ├─────────────────────────────
  ├─ Open PPTC Directory           [fa-folder-closed]
  └─ Open Shader Cache Directory   [fa-folder-closed]
Extract Data ►                     [fa-file-export]
  ├─ ExeFS
  ├─ RomFS
  ├─ AoC RomFS (if DLC exists)
  └─ Logo
```

---

## Keyboard Shortcuts

### Global Shortcuts

| Key | Action |
|-----|--------|
| `Alt+Enter` | Toggle fullscreen |
| `F11` | Toggle fullscreen |
| `Cmd+Ctrl+F` | Toggle fullscreen (macOS) |
| `F9` | Toggle dock mode |
| `Escape` | Exit current state / Cancel |
| `Ctrl+A` | Open Amiibo window |
| `Ctrl+B` | Open .bin Amiibo file |

### Settings Window

| Key | Action |
|-----|--------|
| `Escape` | Cancel and close |

---

## Spacing and Layout Constants

| Constant | Value | Usage |
|----------|-------|-------|
| `PageMargin` | 40px 0 40px 0 | Main page margins |
| `Margin` | 0 5px 0 5px | Standard element margin |
| `TextMargin` | Dynamic | Text block margins |
| `MenuItemPadding` | 5px 0 5px 0 | Menu item internal padding |
| `ScrollBarThickness` | 15px | Scrollbar width |
| `ThemeBorderThickness` | 1px | Standard border width |

---

## Implementation Notes

### MVVM Bindings

- Use `[ObservableProperty]` for reactive properties
- Use `[RelayCommand]` for command bindings
- ViewModel names follow pattern: `{Feature}ViewModel`

### Localization

- All user-facing strings use `{ext:Locale LocaleKey}` markup
- Locale files stored in `assets/locales/*.json`
- `LocaleManager.Instance` provides runtime access
- RTL support via window base classes

### Theme Switching

- `RyujinxApp.ThemeChanged` event fires on theme change
- Use `{DynamicResource ResourceKey}` for theme-aware colors
- FluentAvalonia handles system theme detection when set to Auto

### Custom Controls

| Control | Purpose |
|---------|---------|
| `RyujinxLogo` | Application logo with anti-aliasing |
| `SliderScroll` | Slider with mouse wheel support |
| `MiniVerticalSeparator` | Thin vertical divider for status bar |
| `NavigationDialogHost` | Multi-page dialog navigation |
| `RendererHost` | Game rendering surface |

---

## Responsive Behavior

- Main window enforces 800x500px minimum
- Settings enforces 844x480px minimum
- Grid view wraps horizontally (`WrapPanel`)
- List view scrolls vertically (`StackPanel`)
- Size slider (1-4) scales icon sizes
- Content areas use `Stretch` alignment for flexible sizing
