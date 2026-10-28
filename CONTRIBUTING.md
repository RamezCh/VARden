# Contributing to VARden

Thanks for wanting to help. VARden edits something people rely on every day (their environment), so the bar is: **never lose a user's data, never surprise them.** Everything below follows from that.

## Ways to help

- **Report a problem.** In VARden, use **Settings > Report a problem**: it prepares a cleaned log (names and secrets hidden) and opens the issue form with your version filled in.
- **Suggest an idea.** Open an issue with the *Suggest an idea* form. Describe the problem you have, not only the solution.
- **Improve the docs.** The README, FAQ and website (`site/`) are written for people who don't know what an environment variable is yet. Clearer wording is always welcome.
- **Write code.** Pick an open issue, or open one first for anything bigger than a small fix so we can agree on the approach.

## Getting set up

You need Rust (stable), Node.js 24 or newer (the version in `.nvmrc` is what CI uses) and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system.

```bash
npm install
npm run tauri dev      # the app, with hot reload
cargo test --workspace # Rust tests, including terminal sync in real shells you have installed
npm test               # frontend tests
```

[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) explains how the code is organised and why things work the way they do. Read the section for the area you're changing before you start.

## Rules for changes

1. **Writes are backed up.** Anything that changes the registry or a startup file goes through the stores in `crates/envcore`, which back up first. Never write to those places directly.
2. **User-level only.** VARden never writes system-wide variables or asks for administrator rights. Changes that would need elevation won't be accepted.
3. **Values never reach logs.** Log variable names and actions, never values.
4. **Anything written into a shell is escaped and tested.** If you touch how values are written into startup files or sync scripts, add the tricky value to the tests that run real shells (`sync.rs`, `managed_block.rs`).
5. **Logic lives in `envcore`.** It's plain Rust with no UI and is where tests go. The Tauri layer (`src-tauri`) should stay thin.
6. **Plain words in the interface.** Say what something does for the user ("Saved for your account") rather than how it's built ("HKCU registry value"). Sentence case, no jargon without a **?** help.

## Before you open a pull request

These are what CI runs on Windows, macOS and Linux:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test
npm run build
```

Keep pull requests focused on one change, explain the *why* in the description, and include screenshots for anything visible. If you changed behaviour, update the README, FAQ or `docs/DEVELOPMENT.md` in the same pull request.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE), like the rest of VARden.
