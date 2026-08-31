---
name: KufEditor
description: Native Utility / Focused Split for direct game-data editing and mod management.
colors:
  background: "#151B21"
  surface: "#171D24"
  raised: "#202832"
  border: "#303A46"
  text: "#E7ECF3"
  text-dim: "#93A0AF"
  accent: "#90BFF8"
  accent-dim: "#26497F"
  success: "#AFE0A4"
  warning: "#E5C07B"
  danger: "#E06C75"
typography:
  metadata:
    fontFamily: "Inter"
    fontSize: "11px"
  supporting:
    fontFamily: "Inter"
    fontSize: "12px"
  body:
    fontFamily: "Inter"
    fontSize: "14px"
  section-title:
    fontFamily: "Inter"
    fontSize: "17px"
  workspace-title:
    fontFamily: "Inter"
    fontSize: "18px"
  empty-title:
    fontFamily: "Inter"
    fontSize: "20px"
---

# Design System: KufEditor

## Overview

**Design thesis: Native Utility / Focused Split.** KufEditor is a direct operating tool for creators and players. Files, Mods, and Patches are equal peer workspaces. The application starts in Files and has no Home or dashboard. Settings stays in the global toolbar.

Each workspace puts context in a narrow navigator and work in one dominant canvas. Compact controls, hairline seams, and truthful state copy keep attention on the current object. A compact status bar reports writes, warnings, and unsaved changes.

This is one cross-platform GPUI language for Windows, macOS, and Linux. Detailed editor fields, brand campaigns, and OS-specific restyling are outside this system. The interface must not invent capabilities, content, or recovery state.

## Colors

The frontmatter palette is normative. Use each token by role:

| Token | Use |
| --- | --- |
| `background` | App ground, work canvases, and empty states. |
| `surface` | Toolbar, navigators, controls, panes, and full-width surfaces. |
| `raised` | Hover fields, raised rows, badges, and adjacent tonal separation. |
| `border` | One-pixel seams, dividers, and control outlines. |
| `text` | Primary labels, values, and headings. |
| `text-dim` | Metadata, paths, explanations, and disabled labels. |
| `accent` | Focus outlines, selected borders, active rules, and sparse emphasis. |
| `accent-dim` | Selected, focused, and pressed fields. |
| `success` | Ready and completed state only. |
| `warning` | Pending confirmation, active work, and unsaved state. |
| `danger` | Actionable failure that needs attention. |

Status colors communicate state, not decoration. Pair every status color with text, shape, or position. Do not add gradients, glow, translucent effects, or decorative color ramps.

## Typography

Inter is the only interface family. Body copy inherits the 14px root size. Use 11px for compact metadata, 12px for supporting copy, 17px for section titles, and 18px for workspace titles. Reserve 20px for a strong empty-state title.

Use sentence case for actions and headings. Uppercase 11px labels can name a navigator or status group. Never stack an eyebrow label above a heading. Do not add a display face, ornamental weight, or wide tracking.

## Layout

The default window is 1320×840px. The enforced minimum is 1180×720px. At the minimum size, the split stays horizontal and primary operations remain visible.

The shell has four fixed layers:

1. A 58px global toolbar contains KufEditor, a segmented game context, contextual actions, dirty state, and Settings.
2. A 44px strip divides the full width equally among Files, Mods, and Patches.
3. The active workspace takes all remaining space. Notices never add a separate workspace row.
4. A 32px status bar anchors the bottom edge.

Settings is a toolbar destination, not a fourth peer tab. The shell has no permanent product rail. Workspace actions stay near the selected object or operation.

| Workspace | Contextual navigator | Dominant canvas |
| --- | --- | --- |
| Files | A fixed 260px recent-files list remains visible with or without an open document. | A 40px document-tab row sits above the editor. The empty canvas explains supported files and offers Open file. |
| Mods | A fixed 260px rail selects Installed, Library, Backups, or Create. Its footer shows the selected game and folder. | A 62px contextual header leads full-width rows, forms, issues, confirmation, and progress. |
| Patches | A fixed 260px navigator lists patch controls after inspection succeeds. | A 62px route header leads status, executable details, fire-rate controls, and inline confirmation. |
| Settings | No workspace navigator. | A flexible primary column stays at least 700px. A 400px secondary column follows after a 12px gap. |

Use one-pixel seams and compact spacing. Common gaps are 6–8px inside controls, 12px between groups, and 16–18px inside shell surfaces. Use 20–32px only for large content or empty-state padding. Keep toolbar controls at 30px, workspace actions at 32–34px, rail rows at 36px, navigator headers at 48px, and route headers at 62px.

The game context is part of the application identity. Put it after KufEditor in one segmented control. Separate it from file actions with space and a vertical seam. Group Open and Save actions together. Put Undo and Redo in a second action group.

## Elevation & Depth

The interface has no shadows. Adjacent graphite tones and one-pixel borders establish depth. `raised` can move a row or control one tonal step forward. It must not make a floating card layer. Do not add gradients, glow, backdrop blur, or simulated glass.

## Shapes

Structural panes, tab strips, navigator shells, and full-width surfaces use square corners. Interactive controls use the existing compact medium radius, visually 4–6px. Selected mod rows can use the same control radius. Reserve full pills and circles for compact status badges and dots.

Borders are one pixel by default. A selected workspace uses a two-pixel bottom rule. A selected rail item uses a two-pixel left rule. Do not soften the shell with large radii or rounded container stacks.

## Components

- **Selection:** Use `accent-dim` for compact rows and fields. Use `raised` with an `accent` rule for navigation tabs. Selection must change more than text color.
- **Hover:** Move one adjacent surface tone. Toolbar controls can also change their border to `accent`. Hover must not add lift or glow.
- **Focus:** Use an `accent` border with an `accent-dim` field. Keep the focus state visible for keyboard navigation.
- **Disabled:** Use `text-dim` at 45% opacity. Remove pointer behavior and action binding. Put a specific reason beside disabled Mods and Patches actions.
- **Empty and loading:** Keep the current navigator and canvas structure. Give one factual status and one next action. Files keeps recent files visible. Mods and Patches keep state inside their work canvases.
- **Errors:** Keep errors near the failed operation and retain useful detail. Use `danger` for the highest-priority status failure. Do not rely on color alone.
- **Confirmation:** Show the exact subject, consequence, and recovery target before a write. Keep Cancel beside an explicit action verb. Confirmations stay inline at the lower edge of the workspace.

The status bar is a typed state projection. Its priority is fixed:

1. Settings-save failure.
2. Pending mod confirmation.
3. Pending patch confirmation.
4. Active mod operation.
5. Active patch operation.
6. Unsaved document changes.
7. Ready.

Pending copy describes what will change, what stays unchanged, and whether deletion is permanent. It must describe a backup as future work until that backup exists. Active copy names the current operation, actual backup target, progress, or safe cancellation boundary. Apply copy can name transactional writes and before-images because the implementation provides them. Restore and uninstall copy must name their real recovery mechanism. Patch copy must show the executable and backup paths. Do not replace these facts with generic claims such as “safe” or “backup ready.”

Persistent state stays on the left side of the status bar. Transient notices appear on the right side. Long notice details truncate inside the bar and never change the workspace height.

Successful completion notices remain for three seconds, then clear. Warnings and errors remain until the related state changes.

After completion, the right side reports the result. The left side then shows the next highest state or Ready. Failures retain actionable detail and recovery paths when available. The Ready state shows no supporting text.

Notices share the status-bar surface and use a status dot. Do not put notices in a separate strip or fill them with a status color.

Normal text pairings must meet at least 4.5:1 contrast. Existing tests cover `accent` on `surface` and `text` on `accent-dim`. Extend contrast tests when a new text pairing becomes normative. Preserve visible keyboard focus, logical focus order, and Enter or Space activation for new keyboard controls.

Structural debug selectors are part of the shell test contract. Keep each selector on the semantic element that owns the asserted bounds. This contract includes `workspace-tabs`, each `workspace-*` tab, `files-navigator`, `files-document-canvas`, `files-active-editor`, `mods-canvas`, `mods-scroll`, `patches-navigator`, `patches-canvas`, both `settings-*-column` selectors, and `status-bar`. Confirmation, progress, row, and action selectors must keep stable identities. Update tests with any intentional selector change.

## Do's and Don'ts

### Do

- Do use semantic GPUI elements and text-first actions. Native file pickers can follow host behavior, but the shell visuals stay fixed.
- Do keep Files, Mods, and Patches equal in hierarchy. Keep contextual navigation inside the active workspace.
- Do state the exact write scope, backup path, rollback behavior, and unchanged files that the operation can prove.
- Do treat `.impeccable/mocks/app-shell-focused-split.webp` as a reference-only composition. Its fictional content, badge, and action glyphs are not product assets.

### Do not

- Do not add a dashboard Home, permanent global product rail, card grid, or centered max-width workspace.
- Do not add gradients, glow, large shadows, glass effects, or large rounded containers.
- Do not create OS-specific redesigns. Windows, macOS, and Linux use the same GPUI design language.
- Do not claim that work is safe, recoverable, or backed up unless current state proves the claim.
- Do not place eyebrow labels above headings or use status color as decoration.
- Do not ship the approved comp or any generated raster. No external logo, font file, or icon asset is approved.

Runtime screenshot capture was privacy-blocked on macOS 15. Current evidence is source review and GPUI geometry tests. No screenshot approval is claimed.
