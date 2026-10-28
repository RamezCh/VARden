# Releasing VARden

Releases are built by GitHub Actions ([`.github/workflows/release.yml`](../.github/workflows/release.yml)) on Windows, macOS and Linux and attached to a **draft** release. Nothing is public until you publish the draft.

## What gets built

| Platform | Files | Notes |
|---|---|---|
| Windows | `VARden_x.y.z_x64-setup.exe` | Installs for the current user only, so it never asks for administrator rights. Adds a Start menu entry; the last page offers a desktop shortcut. |
| macOS | `VARden_x.y.z_universal.dmg` | One build for Apple Silicon and Intel. |
| Linux | `.AppImage`, `.deb`, `.rpm` | The AppImage updates itself; `.deb` and `.rpm` are updated by downloading the new package. |
| All | `latest.json` and `.sig` files | Read by the app's auto-updater. Every update is signed, and the app refuses one whose signature doesn't match. |

## One-time setup: the update key

The auto-updater only accepts updates signed with your private key. Create it once, on your own computer, and keep it safe: if it's lost, installed copies can't be updated automatically any more (people would have to download the next version by hand).

1. Generate the key pair (PowerShell, in the project folder). Choose a password when asked.

   ```powershell
   npx tauri signer generate -w "$HOME\.tauri\varden.key"
   ```

   This writes `varden.key` (private, never commit it) and `varden.key.pub` (public).

2. Put the **public** key into `src-tauri/tauri.conf.json`, under `plugins > updater > pubkey`:

   ```powershell
   Get-Content "$HOME\.tauri\varden.key.pub"
   ```

   Commit that change; the public key is meant to be public.

3. Add two repository secrets on GitHub (**Settings > Secrets and variables > Actions > New repository secret**):

   | Name | Value |
   |---|---|
   | `TAURI_SIGNING_PRIVATE_KEY` | The contents of `varden.key` (`Get-Content "$HOME\.tauri\varden.key" -Raw`) |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The password you chose |

4. Back up `varden.key` and its password, for example in your password manager.

## Making a release

1. Bump the version in all three places, to the same number:
   - `package.json` (`"version"`)
   - `src-tauri/Cargo.toml` (`version`)
   - `src-tauri/tauri.conf.json` (`"version"`)
2. Commit, then tag and push:

   ```bash
   git tag v0.2.0
   git push origin main v0.2.0
   ```

3. Wait for the **Release** workflow to finish (about 15 minutes). It creates a draft release named *VARden 0.2.0* with all the files.
4. Open the draft on GitHub, write what changed, and **Publish**. From then on, installed copies of VARden see the update (at their next start, or with *Settings > Check now*).

## Windows SmartScreen

The installer isn't signed with a paid code-signing certificate, so Windows may show *"Windows protected your PC"* the first time. Users click **More info > Run anyway**. Updates installed by the app itself don't show this.

## What the uninstaller does (Windows)

- Removes the terminal-sync lines VARden added to shell profiles (each profile is backed up first), so no shell keeps loading a hook that's gone.
- Removes the "Start with Windows" entry.
- If you tick **Delete the application data**, it also removes `%LOCALAPPDATA%\VARden` (backups, sync files, logs).
- Your variables are never removed. They belong to your account, not to the app.

On macOS and Linux there's no uninstaller to hook into. Remove terminal sync on the **Shells** page first, or run `varden --uninstall-cleanup`, then delete the app.
