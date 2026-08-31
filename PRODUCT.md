# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

KufEditor serves mod creators who edit Kingdom Under Fire game data and players who want to manage installed mods.
Neither workflow has priority. The interface must make both workflows easy to use.

## Product Purpose

KufEditor is a local desktop editor and mod manager for Kingdom Under Fire: The Crusaders and Kingdom Under Fire: Heroes.
Users can edit supported game files, manage mods, maintain backups, and inspect or apply supported executable patches.
Success means that users can complete these tasks without understanding file formats or storage internals.

## Operating Context

KufEditor runs as a native GPUI desktop application on Windows, macOS, and Linux.
It uses one cross-platform interface language and does not adapt its visual language to each OS.
The `web` platform value selects the platform-neutral Impeccable workflow. KufEditor is not a browser application.

Users work with local game installations, supported data files, mod packages, backups, and executable files.
The application can discover installations or use paths selected by the user.

## Capabilities and Constraints

- The editor supports TroopInfo and SkillInfo SOX, text SOX, Crusaders SAV, and Crusaders STG files.
- Document workflows include recent files, tabs, validation, undo, redo, Save As, and dirty-file protection.
- The mod workflow can create, import, apply, uninstall, and inspect packages. It can also create and restore backups.
- The patch workflow can inspect, apply, and revert supported executable changes.
- File editing must preserve unknown and reserved data. Saves must use atomic replacement. No-op saves must preserve the original bytes.
- Mod and patch changes must use recoverable backups. They must reject unexpected targets or data.
- The node editor remains outside the Rust rewrite until a separate design session.
- The application remains a Rust and GPUI desktop application.
- Interface changes must preserve product capabilities and user data.

## Brand Commitments

- `kufeditor` is the fixed project name. The user-facing title is `KufEditor`.
- Project-owned acronyms use uppercase spellings such as GPUI, SAV, SOX, STG, and ZIP.
- The interface uses a restrained, conventional native desktop utility language at full craft. It does not use a thematic visual metaphor or imitate one operating system.
- No logo or external brand asset was confirmed during initialization.

## Evidence on Hand

- The Rust application under `crates/kufeditor/src/` is evidence for current workflows and behavior.
- The format, workspace, game, mod, and patch crates contain behavior tests for the supported workflows.
- The release workflow builds native artifacts for Windows, macOS, and Linux.
- No user research, usage metrics, testimonials, or external brand assets were confirmed. Future work must not invent them.

## Product Principles

- Make file editing and mod management easy for their users.
- Keep file edits and game changes safe and reversible.
- Explain state, risk, and recovery in user terms.
- Use one coherent interface on all supported desktop operating systems.
- Preserve supported workflows when the interface changes.
