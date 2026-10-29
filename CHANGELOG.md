# Changelog

## 0.1.1 (2026-10-29)

Security and reliability fixes. Updating is recommended.

- **`.env` export:** values containing a backtick could run a command when the exported file was sourced by a shell (`set -a; . ./.env`). Backticks are now escaped.
- **Terminal sync:** variables with names shells can't set (such as `ProgramFiles(x86)` or `MY-VAR`) could break the sync script for every variable. They are now left out and reported.
- **File writes:** the problem report can only be saved as a `.txt` or `.log` file, and exports only as `.env` files.
- **Path checks:** Git Bash style paths like `/usr/bin` on Windows are no longer flagged as missing.
- **PATH editor:** a drive root (`C:\`) is no longer treated as the same entry as `C:`.
- Smaller fixes: PowerShell quote handling in hand-edited profiles, the move from older app folders, and fewer background threads during bursts of saves on Windows.

## 0.1.0 (2026-10-28)

First release.
