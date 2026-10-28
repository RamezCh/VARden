# Environment Manager

A Tauri 2 desktop app for managing **user-level** (persistent) and **process-level** (session) environment variables. It never writes system-level variables, so it never needs admin rights.

## Layout

```
crates/envcore/        Platform core: plain Rust, no Tauri, fully unit-tested
  src/managed_block.rs   Finds, parses and rewrites the "# >>> ENVMANAGER MANAGED BLOCK >>>" block
  src/store/             Where user-level vars persist
    windows_registry.rs    HKCU\Environment + WM_SETTINGCHANGE broadcast   (Windows)
    shell_file.rs          Managed block inside .bashrc / .zshrc / config.fish / env.nu / $PROFILE
  src/backup.rs          Timestamped backups before every write, restore, pruning
  src/shells.rs          Shell + profile auto-detection
  src/status.rs          Status pill logic (Active & Synced / Persistent Only / Process Only / Missing)
  src/manager.rs         EnvManager: the API the Tauri commands call
src-tauri/             Tauri app: commands.rs wraps EnvManager
src/                   React + Tailwind v4 frontend
  lib/api.ts             Typed wrappers for every Tauri command
  lib/useEnvironment.ts  Owns all data and mutations; the table is always replaced by what the backend returns
  lib/filter.ts          Pure search, filter and sort logic (unit-tested with Vitest)
  components/            Dashboard: status strip, variable table, shells, backups, toasts
```

## Prerequisites (Windows)

1. Rust via [rustup](https://rustup.rs), using the MSVC toolchain
2. Visual Studio Build Tools with the "Desktop development with C++" workload
3. Node.js 20 or newer
4. WebView2 (preinstalled on Windows 10 and 11)

## Run

```powershell
npm install
npm run tauri dev        # dev window with hot reload
cargo test -p envcore    # core unit tests
npm test                 # frontend unit tests
npm run tauri build      # installer in target/release/bundle
```

## Phase 1 design decisions

- **Values are written literally.** Shell files get single quotes (`export K='v'`), so `$`, backticks and `%` never expand by accident. A hand-written line inside the block that uses expansion (`export PATH="$HOME/bin:$PATH"`) is kept exactly as written. The app lists its raw text, and setting that key from the app replaces the line.
- **Malformed blocks are never touched.** If a start or end marker is missing or duplicated, every write to that file is refused with an explanation, until you fix the file or restore a backup.
- **Safe file writes.** The app writes a temp file and renames it into place. It follows symlinks, so dotfile managers keep working, and it keeps the BOM, CRLF line endings and file permissions as they were. If a write wouldn't change anything, it is skipped and no backup is made.
- **Registry details.** An existing `REG_EXPAND_SZ` value (such as `Path`) keeps its type. New values that contain `%VAR%` become `REG_EXPAND_SZ`. Name matching ignores case, so the app never creates `path` next to an existing `Path`. The `WM_SETTINGCHANGE` broadcast runs on a background thread so a hung window can't freeze the UI.
- **Backups.** Before every change, the app saves a copy of the file, or a JSON snapshot of all of `HKCU\Environment`, to `<app local data>/backups/<timestamp>/`. The first backup of each file (and of the registry) is its **original** state from before the app touched it, and it is never pruned. Only distinct states are kept: if the same content was already backed up, no new backup is written and the app says which existing backup already holds that state. Besides the original, the 50 newest backups per target are kept. Restoring a backup first backs up the current state, so a restore can itself be undone.
- **Status semantics.** "Current" means the environment the app was launched with, plus any process-level variables set in the app. A variable you just saved therefore shows 🟡 *Persistent Only* until the app is restarted, because that is also true for terminals that are already open. For `PATH`, the status counts as synced when every user entry appears in the process PATH. (On Windows the process PATH is the system PATH plus the user PATH.)
- **Windows uses only the registry.** PowerShell, cmd and Git Bash all inherit `HKCU\Environment`, so profile files are only detected, not written. On macOS and Linux the default target is your login shell's rc file, and `set_shell_targets` can add or swap files.

## Phase 2: dashboard

- **Status strip.** A proportional bar showing how your variables split across the four states. Click a state to filter the table; you can select several at once.
- **Table.** Search, sorting by name, scope or status, a scope filter, inline editing, and a delete confirmation inside the row. The **N** key adds a variable and **/** jumps to search.
- **Secrets.** Values that look sensitive are masked with a fixed-length mask, so their length isn't shown. Each has its own eye toggle, there's a global "Show secrets" switch, and values can be copied without being revealed. Search only matches values that are currently visible, so a masked secret can't be found by guessing.
- **Session variables.** Editing a session variable lets you save it to your user environment. Moving a user variable to the session isn't offered, because that would quietly delete saved data.
- **Notices.** Every change is confirmed with a toast. If no new backup was needed, a toast says which existing backup already holds that state.
- **Font.** The interface uses Atkinson Hyperlegible Next and Mono, bundled with the app, so 0/O and 1/l/I in tokens can't be confused.

## Tauri commands

`get_platform_info`, `list_variables`, `set_variable`, `delete_variable`, `detect_shells`, `get_shell_targets`, `set_shell_targets`, `list_backups`, `restore_backup`. Each mutation returns the refreshed variable list plus the backups it created.
