mod commands;

use std::sync::Mutex;
use std::time::Duration;

use envcore::manager::EnvManager;
use envcore::{paths, watch};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

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
fn focus_main_window(app: &AppHandle) {
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

pub fn run() {
    tauri::Builder::default()
        // Must be registered first so a second launch exits before doing anything.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("Second launch: focusing the open window");
            focus_main_window(app);
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_dialog::init())
        // Reopen at the size and place the window was closed at.
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .setup(|app| {
            log_panics();
            log::info!(
                "VARden {} starting on {}",
                app.package_info().version,
                std::env::consts::OS
            );
            migrate_legacy_data();

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
            let watcher = spawn_watcher(app.handle(), paths);
            app.manage(Mutex::new(watcher));
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running VARden");
}
