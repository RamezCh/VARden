# Roadmap

Phases 1–3 of the original spec (backend core, dashboard, terminal sync, `.env` bridge, recovery) are done. The remaining work is done stage by stage, in order.

## Stage 1: Foundation
- [x] Only one instance can run; launching it again focuses the open window.
- [x] Log file for diagnostics, rotated so it never grows without limit.
- [x] Automatic refresh when variables change outside the app: registry change notifications on Windows, a file watcher on startup files on macOS/Linux, and a refresh whenever the window regains focus.
- [x] CI on Windows, macOS and Linux: formatting, lints, Rust and frontend tests, and a full app build.

## Stage 2: Validation
- [x] Rename the app to VARden, moving existing data and upgrading blocks already written to startup files.
- [x] Flag values that point to folders or files that don't exist (PATH entries, `JAVA_HOME` and similar) as 🔴.
- [x] Show the expanded value next to `%VAR%` values.
- [x] More help text for newcomers, including what happens to temporary variables when the app closes.

## Stage 3: Profiles
- [x] Named sets of variables (for example "Client A" and "Client B"), applied as User or Temporary.
- [x] 🔴 when a profile expects a variable that is missing or empty.
- [x] Rename the shell files in the UI to "startup files", to avoid a clash with profiles.
- [x] Screen for custom startup-file paths on macOS/Linux.

## Stage 4: Editing tools
- [x] Safe PATH editor: only your PATH is editable (the system PATH is read-only), warnings for missing folders, a preview of changes before saving.
- [x] Rename a variable.
- [x] Bulk select and delete in the table.
- [x] Up to 5 recent export folders.

## Stage 5: Release
- [ ] Installer with a Start menu entry and an optional desktop shortcut.
- [ ] Uninstaller that also removes the terminal-sync blocks from shell profiles.
- [ ] Auto-updater fed by GitHub Releases.
- [ ] Tray icon with Sync to terminals, and an optional "start with Windows".

## Stage 6: Launch
- [x] README with GIF and screenshots, comparison table, FAQ.
- [ ] License, contributing guide, release workflow that publishes installers.
