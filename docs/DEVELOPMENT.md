# Developing VARden

How the code is organised, how to build it, and the design decisions behind each part. For what VARden does and how to use it, see the [README](../README.md).

## Code layout

```
crates/envcore/        Platform core: plain Rust, no Tauri, fully unit-tested
  src/managed_block.rs   Finds, parses and rewrites the "# >>> VARDEN MANAGED BLOCK >>>" block
  src/store/             Where user-level vars persist
    windows_registry.rs    HKCU\Environment + WM_SETTINGCHANGE broadcast   (Windows)
    shell_file.rs          Managed block inside .bashrc / .zshrc / config.fish / env.nu / $PROFILE
  src/backup.rs          Timestamped backups before every write, restore, pruning
  src/shells.rs          Shell + profile auto-detection
  src/status.rs          Status pill logic (Active & Synced / Persistent Only / Process Only / Missing)
  src/manager.rs         EnvManager (core API): what the Tauri commands call
  src/sync.rs            Terminal sync: plan, per-shell hook and apply scripts (tested in real shells)
  src/terminal.rs        Hook install/remove in profiles, Sync to terminals
  src/dotenv.rs          .env parser and writer
  src/transfer.rs        .env import/export
  src/paths.rs           App data folder (%LOCALAPPDATA%\VARden)
  src/watch.rs           Notices outside changes (registry notifications, file watcher)
  src/profiles.rs        Profiles: apply as user or temporary, turn off restores previous values
  src/path_editor.rs     Safe PATH editor: your entries only, around the inherited/system PATH
  src/validate.rs        Name rules, secret detection, missing-path checks
src-tauri/             Tauri app (binary `varden`): commands.rs wraps the core API
src/                   React + Tailwind v4 frontend
  lib/api.ts             Typed wrappers for every Tauri command
  lib/useEnvironment.ts  Owns all data and mutations; the table is always replaced by what the backend returns
  lib/filter.ts          Pure search, filter and sort logic (unit-tested with Vitest)
  lib/pathEdit.ts        PATH editor list logic: moves, validation, diff (unit-tested)
  lib/theme.ts           Light/dark theme following the system, with a manual override
  components/            Pages (Variables, Profiles, Shells, Backups), modals, table, toasts
```

## Prerequisites

On Windows:

1. Rust via [rustup](https://rustup.rs), using the MSVC toolchain
2. Visual Studio Build Tools with the "Desktop development with C++" workload
3. Node.js 20 or newer
4. WebView2 (preinstalled on Windows 10 and 11)

On macOS: Xcode Command Line Tools, Rust and Node.js 20+. On Linux: Rust, Node.js 20+ and the [Tauri system packages](https://tauri.app/start/prerequisites/#linux) (`libwebkit2gtk-4.1-dev` and friends).

## Build and test

```powershell
npm install
npm run tauri dev        # dev window with hot reload
cargo test -p envcore    # core unit tests
npm test                 # frontend unit tests
npm run tauri build      # installer in target/release/bundle
```

## Design notes

The sections below were written as each stage landed. They explain behaviour in more detail than the README.

### Phase 1: core

- **Values are written literally.** Shell files get single quotes (`export K='v'`), so `$`, backticks and `%` never expand by accident. A hand-written line inside the block that uses expansion (`export PATH="$HOME/bin:$PATH"`) is kept exactly as written. The app lists its raw text, and setting that key from the app replaces the line.
- **Malformed blocks are never touched.** If a start or end marker is missing or duplicated, every write to that file is refused with an explanation, until you fix the file or restore a backup.
- **Safe file writes.** The app writes a temp file and renames it into place. It follows symlinks, so dotfile managers keep working, and it keeps the BOM, CRLF line endings and file permissions as they were. If a write wouldn't change anything, it is skipped and no backup is made.
- **Registry details.** An existing `REG_EXPAND_SZ` value (such as `Path`) keeps its type. New values that contain `%VAR%` become `REG_EXPAND_SZ`. Name matching ignores case, so the app never creates `path` next to an existing `Path`. The `WM_SETTINGCHANGE` broadcast runs on a background thread so a hung window can't freeze the UI.
- **Backups.** Before every change, the app saves a copy of the file, or a JSON snapshot of all of `HKCU\Environment`, to `%LOCALAPPDATA%\VARden\backups\<timestamp>\` (on macOS `~/Library/Application Support/VARden/backups`, on Linux `~/.local/share/VARden/backups`). The first backup of each file (and of the registry) is its **original** state from before the app touched it, and it is never pruned. Only distinct states are kept: if the same content was already backed up, no new backup is written and the app says which existing backup already holds that state. Besides the original, the 50 newest backups per target are kept. Restoring a backup first backs up the current state, so a restore can itself be undone.
- **Status semantics.** "Current" means the environment the app was launched with, plus any process-level variables set in the app. A variable you just saved therefore shows 🟡 *Persistent Only* until the app is restarted, because that is also true for terminals that are already open. For `PATH`, the status counts as synced when every user entry appears in the process PATH. (On Windows the process PATH is the system PATH plus the user PATH.)
- **Windows uses only the registry.** PowerShell, cmd and Git Bash all inherit `HKCU\Environment`, so profile files are only detected, not written. On macOS and Linux the default target is your login shell's rc file, and `set_shell_targets` can add or swap files.

### Phase 2: dashboard

- **Status strip.** A proportional bar showing how your variables split across the four states. Click a state to filter the table; you can select several at once.
- **Table.** Search, sorting by name, scope or status, a scope filter, inline editing, and a delete confirmation inside the row. The **N** key adds a variable and **/** jumps to search.
- **Secrets.** Values that look sensitive are masked with a fixed-length mask, so their length isn't shown. Each has its own eye toggle, there's a global "Show secrets" switch, and values can be copied without being revealed. Search only matches values that are currently visible, so a masked secret can't be found by guessing.
- **Temporary variables.** Never saved. **Sync to terminals** pushes them into open terminals that have terminal sync, where they win over a saved variable of the same name. Removing one drops it from those terminals at the next sync; new terminals and other programs never see them. Editing a temporary variable lets you save it to your user environment. Making a saved variable temporary isn't offered, because that would quietly delete saved data.
- **Notices.** Every change is confirmed with a toast. If no new backup was needed, a toast says which existing backup already holds that state.
- **Font.** The interface uses Atkinson Hyperlegible Next and Mono, bundled with the app, so 0/O and 1/l/I in tokens can't be confused.

### Phase 3: terminal sync, .env bridge, recovery

- **Terminal sync.** On the Shells page, **Install** adds a short block to that shell's profile, marked `# >>> VARDEN SHELL HOOK >>>`, and backs the profile up first. The block sources a small hook script from `%LOCALAPPDATA%\VARden\shell`. **Sync to terminals** writes per-shell apply scripts to `VARden\sync` and then bumps a generation number. Before drawing its next prompt, each hooked terminal reads that number (one small file read) and applies the changes if it has moved. This works for PowerShell 7, Windows PowerShell, Git Bash, bash, zsh and fish. Command Prompt has no prompt hook and Nushell isn't supported yet, so open a new window for those.
  - Apply scripts always describe the full desired state, so a terminal that missed several syncs still ends up correct, and variables deleted since an earlier sync are removed. On Windows, deleting a user variable that overrides a machine variable brings back the machine value.
  - PATH is merged entry by entry instead of overwritten, so an activated virtualenv or conda environment survives a sync.
  - A new terminal starts with your saved variables already, so its hook only applies syncs made after it opened.
  - The spec suggested `eval "$(envmanager hook)"` (the project's working title). The profile sources a static script instead, so nothing has to be on PATH and starting a shell never launches the app.
- **.env import.** Use **Import .env**, or drop a file anywhere on the window. You see every variable first, marked *New*, *Replaces current value* or *Unchanged* (unchanged ones start unticked), plus any lines that couldn't be read and why. You choose the user environment or this session. The import is written with one backup and can be undone in one step.
- **.env export.** Pick variables, preselected from what the table is currently showing, then choose where to save. You're warned when secrets would be written as plain text. Windows `%VAR%` values are written expanded.
- **Recovery.** Before a restore, the Backups page lists exactly which variables would come back, change or be removed. Every change's toast has **Undo**. An error about a damaged managed block offers a shortcut to Backups.

### Stage 1: foundation

- **One instance.** Launching the app while it's already running brings the open window to the front instead of starting a second copy, so two windows can never write at the same time.
- **Automatic refresh.** Changes made outside the app (`setx`, the Windows dialog, an editor saving `.zshrc`) show up on their own. On Windows the app subscribes to change notifications for `HKCU\Environment`. On every platform it watches the folders that hold the startup files, which also catches editors that save by replacing the file. Bursts of events become one refresh, and the echo of the app's own writes is ignored. The window also refreshes when it regains focus, and a notice appears when something changed.
- **Log file.** Diagnostics are written to `%LOCALAPPDATA%\VARden\logs` (macOS `~/Library/Application Support/VARden/logs`, Linux `~/.local/share/VARden/logs`): at most 5 files of 1 MB each. Logs contain variable names and actions, never values. **Open log folder** in the sidebar opens it.
- **CI.** Every push runs formatting, clippy, Rust and frontend tests and a full app build on Windows, macOS and Linux, including the terminal hook in real bash, zsh, fish and PowerShell.

### Stage 2: validation

- **Broken paths are 🔴.** Path lists (PATH, PYTHONPATH, CLASSPATH, PSModulePath and similar, plus any `*_PATH` variable holding a list) are checked entry by entry, and a value that is a single absolute path (`JAVA_HOME`, `GOROOT`) is checked as a whole, after expanding `%VAR%` or `~`. Missing entries are listed under the value, and the status becomes *Missing / stale*. The row keeps tracking whether open terminals are up to date. URLs, relative paths and shell expressions are never checked, and neither are network paths (`\\server\share`), because an unreachable share can stall a check for seconds.
- **Expanded values.** Values with `%VAR%` show what programs actually get, for example *Expands to C:\Users\you\go*.
- **More help.** The help texts explain what happens to temporary variables when VARden closes, and what *Missing / stale* means.

### Stage 3: profiles and startup files

- **Profiles** are named sets of variables, for example one per client or project. **Apply** saves them as user variables; **Apply temporarily** keeps them for open terminals only (pushed by *Sync to terminals*, never saved). One profile is active at a time and switching turns the previous one off. Applying remembers each variable's previous value and **Turn off** puts it back, except for values you changed by hand in between. An entry with an empty value marks a required variable: it isn't written, and while the profile is active it shows as *Missing* until it has a value. Editing an active profile offers **Re-apply**. Profiles can be filled by hand, from current variables, or from a `.env` file. They are stored in `profiles.json` in the app folder; like the registry and startup files, values are plain text.
- **Startup files.** The shell files VARden writes to are called *startup files*, so "profile" only means variable sets. On macOS and Linux, *Shells → Where variables are saved* lets you add or remove startup files (for example one in a dotfiles folder). The choice is saved in `settings.json` and survives restarts, and newly chosen files are watched for outside changes right away.
- **Safer migration.** Moving backups from an older folder no longer drops a backup whose id (a millisecond timestamp) collides with an existing one; it gets a fresh id instead.

### Stage 4: editing tools

- **Safe PATH editor** (*Edit PATH*, or the edit button on the PATH row). Only your own folders can be added, removed, edited or reordered. The system part of PATH is shown read-only and is never written: on Windows it is the machine `Path` in HKLM, which always comes first; on macOS and Linux it is the PATH the shell already has, and your folders are written as one line in VARden's block that adds them around it (`export PATH="$HOME/bin:$PATH"`, or `set -gx PATH … $PATH` in fish), so PATH is never replaced. Each folder is checked as you type: missing folders, folders already earlier on PATH, and what `%VAR%`, `$HOME` or `~` expand to. Nothing is saved until you have reviewed the changes (added, removed, order changed). A backup is taken first and the toast offers Undo. A PATH line the editor can't read safely (one that runs a command, for example) is reported, not silently rewritten. Nushell and PowerShell startup files aren't supported by the editor yet.
- **Rename.** Change the name in the edit row; the value moves to the new name with one backup. Renaming onto an existing name, or a case-only rename on Windows, is refused. Shell expressions in startup files are renamed by hand.
- **Bulk select.** Checkboxes on every row, plus one that selects everything shown. Selected variables can be exported or deleted together, with one backup per store.
- **Recent export folders.** The export dialog lists the last five folders you exported to; clicking one opens the save dialog there. Stored in `settings.json`; folders that no longer exist are skipped.

### Interface polish

- **Light and dark themes.** VARden follows the system theme; the sidebar's Theme switch can pin it to light or dark, and the native title bar follows the choice.
- **Desktop behaviour.** The window reopens at its last size and position. Shortcuts: Ctrl+1–4 switch pages, Ctrl+F or `/` searches, Ctrl+N or `N` adds a variable, F5 re-reads everything, and in the table Ctrl+A selects what's shown, Del deletes the selection and Esc clears it. Double-click a row to edit it. Interface text isn't selectable and the browser's context menu, reload and print shortcuts are off, but values, paths and inputs can still be selected and copied.
- **Readable long values.** Long values, path lists and `%VAR%` values open under their row: lists one entry per line with missing folders marked, other values wrapped in full, each with a Copy button.
- **Calmer layout.** One header style on every page, the same width everywhere, grouped actions (Edit PATH, a `.env` menu, Sync, New variable), compact status filters instead of the coloured bar, row actions that appear on hover, sidebar icons, and shell cards that show a file's name with its folder underneath.
- **Fix:** a value that is a folder followed by a separator (JetBrains installers write `IntelliJ IDEA` = `C:\...\bin;` and add `%IntelliJ IDEA%` to PATH) is no longer flagged as missing.

### Renamed to VARden

Earlier builds were called *Environment Manager*. On first start, VARden moves their data (backups including originals, sync state, hook scripts) from `%LOCALAPPDATA%\EnvManager` into `%LOCALAPPDATA%\VARden` and removes the old folder. Blocks in startup files written with the old `ENVMANAGER` markers are still recognised and are rewritten with `VARDEN` markers on the next change. Installed terminal-sync hooks are updated to the new location right away, with a backup first.

### Backups page and help

- **Grouped by day.** Today, Yesterday, then dated groups. Each row shows the time and what the backup was taken before.
- **Deleting.** You can delete one backup, a whole day, or everything with **Clear backups**. Each asks for confirmation first, and deleting can't be undone. The **original** of each file (or of the registry) is never deleted, in the UI or in the backend, so you can always return to the state from before the app was used.
- **Help.** A small **?** next to terms people may not know (scope, statuses, terminal sync, the registry location, originals, `.env` files) explains them on hover, on keyboard focus, or on click.

## Tauri commands

Defined in `src-tauri/src/commands.rs`, typed for the frontend in `src/lib/api.ts`:

`get_platform_info`, `list_variables`, `set_variable`, `delete_variable`, `detect_shells`, `get_shell_targets`, `set_shell_targets`, `list_backups`, `preview_restore`, `restore_backup`, `delete_backups`, `get_terminal_sync`, `install_terminal_hook`, `remove_terminal_hook`, `sync_terminals`, `read_env_file`, `import_variables`, `export_env_file`, `open_logs_folder`, `list_profiles`, `save_profile`, `delete_profile`, `apply_profile`, `deactivate_profile`, `delete_variables`, `rename_variable`, `get_path_info`, `check_path_entries`, `set_user_path`, `recent_export_folders`. Each mutation returns the refreshed variable list plus the backups it created.
