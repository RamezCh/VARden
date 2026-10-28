mod clipboard;
mod commands;
mod tray;

use std::sync::Mutex;
use std::time::Duration;

use envcore::manager::EnvManager;
use envcore::{paths, watch};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};
use tauri_plugin_window_state::StateFlags;

/// Passed by "start with your computer": open in the tray, not on screen.
const MINIMIZED_ARG: &str = "--minimized";
/// Run by the Windows uninstaller before it removes the app.
const CLEANUP_ARG: &str = "--uninstall-cleanup";

/// Event the frontend listens to; it refreshes the dashboard.
const ENV_CHANGED: &str = "environment-changed";

/// Earlier builds kept their files in `EnvManager` or
/// `dev.chreide.envmanager`. Move backups (originals included), sync state
/// and hook scripts into the VARden folder, then remove the old folders.
fn migrate_legacy_data() {
    match paths::migrate_legacy(&paths::app_data_dir(), &paths::legacy_data_dirs()) {
        Ok(done) => done.iter().for_each(|d| log::info!("Migration: {d}")),
        Err(e) => log::error!("Migration from an older version stopped: {e}"),
    }
}

/// Refresh the dashboard when variables change outside the app.
pub(crate) fn spawn_watcher(app: &AppHandle, paths: Vec<std::path::PathBuf>) -> watch::Watcher {
    let handle = app.clone();
    watch::spawn(paths, move |source| {
        let mgr = handle.state::<EnvManager>();
        if mgr.wrote_within(Duration::from_millis(1500)) {
            return; // the echo of the app's own write
        }
        log::info!("Outside change detected ({source:?}); refreshing");
        let _ = handle.emit(ENV_CHANGED, ());
    })
}

/// A second launch just brings the running window to the front.
pub(crate) fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Folder {
                path: paths::logs_dir(),
                file_name: Some("varden".into()),
            }),
            Target::new(TargetKind::Stdout),
        ])
        // At most 5 files of 1 MB each.
        .max_file_size(1_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .level(if cfg!(debug_assertions) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        // Dependencies are noisy at debug level.
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("notify", log::LevelFilter::Warn)
        .build()
}

/// Panics go to the log file too, not just to a console nobody sees.
fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("Panic: {info}");
        default(info);
    }));
}

/// First close: ask whether VARden should stay in the tray, remember the
/// answer (it can be changed in Settings), then hide or quit.
fn ask_what_close_does(window: &tauri::Window) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let app = window.app_handle().clone();
    let tray = if cfg!(target_os = "macos") {
        "menu bar"
    } else {
        "tray"
    };
    window
        .dialog()
        .message(format!(
            "VARden can keep running in the {tray} when you close its window, so Sync to terminals stays one click away.\n\nYou can change this later in Settings."
        ))
        .title("Keep VARden running?")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            format!("Keep in {tray}"),
            "Quit".into(),
        ))
        .parent(window)
        .show(move |keep| {
            let mgr = app.state::<EnvManager>();
            let prefs = envcore::manager::AppPrefs {
                close_to_tray: keep,
                close_asked: true,
                ..mgr.app_prefs()
            };
            if let Err(e) = mgr.set_app_prefs(prefs) {
                log::error!("Couldn't save the close choice: {e}");
            }
            log::info!("First close: keep in tray = {keep}");
            if keep {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            } else {
                app.exit(0);
            }
        });
}

/// Undo what VARden added outside its own folder, so uninstalling leaves
/// nothing behind: the terminal-sync lines in shell profiles (each backed up
/// first). Variables themselves stay; they belong to the user, not the app.
fn uninstall_cleanup() {
    let manager = EnvManager::new(paths::backups_dir());
    let (removed, failed) = manager.uninstall_all_hooks();
    for p in removed {
        println!("Removed terminal sync from {}", p.display());
    }
    for (p, e) in failed {
        eprintln!("Could not remove terminal sync from {}: {e}", p.display());
    }
}

pub fn run() {
    // Handled before anything else (including the single-instance check,
    // which would hand the request to a running window and exit).
    if std::env::args().any(|a| a == CLEANUP_ARG) {
        uninstall_cleanup();
        return;
    }
    let start_minimized = std::env::args().any(|a| a == MINIMIZED_ARG);

    tauri::Builder::default()
        // Must be registered first so a second launch exits before doing anything.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("Second launch: focusing the open window");
            focus_main_window(app);
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_dialog::init())
        // Reopen at the size and place the window was closed at. Visibility
        // isn't restored: closing to the tray hides the window, and the next
        // launch should still show it.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_ARG]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        // Closing the window keeps VARden in the tray, unless turned off.
        // The first time, the user is asked which they prefer.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !tray::is_available(window.app_handle()) {
                    return;
                }
                let prefs = window.state::<EnvManager>().app_prefs();
                if !prefs.close_asked {
                    api.prevent_close();
                    ask_what_close_does(window);
                } else if prefs.close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                    log::info!("Window closed to the tray");
                }
            }
        })
        .setup(move |app| {
            log_panics();
            log::info!(
                "VARden {} starting on {}",
                app.package_info().version,
                std::env::consts::OS
            );
            migrate_legacy_data();
            if let Err(e) = paths::secure_app_dir() {
                log::warn!("Couldn't restrict the app folder to this user: {e}");
            }

            // %LOCALAPPDATA%\VARden\backups on Windows.
            let manager = EnvManager::new(paths::backups_dir());
            // Hook scripts embed the sync folder's path, and installed hooks
            // point at them: keep both current (e.g. after the rename).
            match manager.refresh_installed_hooks() {
                Ok(updated) => updated
                    .iter()
                    .for_each(|p| log::info!("Updated terminal sync in {}", p.display())),
                Err(e) => log::error!("Could not update terminal hooks: {e}"),
            }

            let paths = manager.watch_paths();
            app.manage(manager);
            app.manage(std::sync::Arc::new(clipboard::Clipboard::default()));
            let watcher = spawn_watcher(app.handle(), paths);
            app.manage(Mutex::new(watcher));

            if let Err(e) = tray::create(app.handle()) {
                // Some Linux desktops have no tray; the app works without it.
                log::warn!("No tray icon: {e}");
            }
            // The window starts hidden (see tauri.conf.json) so a start with
            // the computer doesn't flash it on screen.
            if start_minimized && tray::is_available(app.handle()) {
                log::info!("Started with the computer; staying in the tray");
            } else {
                focus_main_window(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_platform_info,
            commands::list_variables,
            commands::set_variable,
            commands::delete_variable,
            commands::delete_variables,
            commands::rename_variable,
            commands::get_path_info,
            commands::check_path_entries,
            commands::set_user_path,
            commands::recent_export_folders,
            commands::detect_shells,
            commands::get_shell_targets,
            commands::set_shell_targets,
            commands::list_backups,
            commands::preview_restore,
            commands::restore_backup,
            commands::delete_backups,
            commands::get_terminal_sync,
            commands::install_terminal_hook,
            commands::remove_terminal_hook,
            commands::sync_terminals,
            commands::read_env_file,
            commands::import_variables,
            commands::export_env_file,
            commands::open_logs_folder,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::apply_profile,
            commands::deactivate_profile,
            commands::get_app_prefs,
            commands::set_app_prefs,
            commands::get_autostart,
            commands::set_autostart,
            commands::prepare_report,
            commands::copy_text,
            commands::save_report,
        ])
        .build(tauri::generate_context!())
        .expect("error while building VARden")
        .run(|_app, _event| {
            // macOS: clicking the Dock icon while the window is hidden
            // (closed to the menu bar) brings it back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                focus_main_window(_app);
            }
        });
}
