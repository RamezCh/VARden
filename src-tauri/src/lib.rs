mod commands;

use std::sync::Mutex;
use std::time::Duration;

use envcore::backup::BackupStore;
use envcore::manager::EnvManager;
use envcore::{paths, watch};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// Event the frontend listens to; it refreshes the dashboard.
const ENV_CHANGED: &str = "environment-changed";

/// Builds before the rename kept backups under the bundle identifier
/// (`dev.chreide.envmanager`). Move them, originals included, then remove
/// the old folder, which only held those backups and the WebView cache.
fn migrate_legacy_data() {
    let Some(legacy) = paths::legacy_data_dir() else {
        return;
    };
    if !legacy.exists() {
        return;
    }
    match BackupStore::new(paths::backups_dir()).adopt_from(&legacy.join("backups")) {
        Ok(moved) => {
            log::info!("Moved {moved} backup(s) from {}", legacy.display());
            let _ = std::fs::remove_dir_all(&legacy);
        }
        Err(e) => log::error!(
            "Keeping {}: could not move all backups: {e}",
            legacy.display()
        ),
    }
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
                file_name: Some("envmanager".into()),
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
        .setup(|app| {
            log_panics();
            log::info!(
                "Environment Manager {} starting on {}",
                app.package_info().version,
                std::env::consts::OS
            );
            migrate_legacy_data();

            // %LOCALAPPDATA%\EnvManager\backups on Windows.
            let manager = EnvManager::new(paths::backups_dir());
            // Hook scripts embed the sync folder's path; keep them current.
            if let Err(e) = manager.refresh_hook_scripts() {
                log::error!("Could not write terminal hook scripts: {e}");
            }

            // Refresh the dashboard when variables change outside the app.
            let handle = app.handle().clone();
            let watcher = watch::spawn(manager.watch_paths(), move |source| {
                let mgr = handle.state::<EnvManager>();
                if mgr.wrote_within(Duration::from_millis(1500)) {
                    return; // the echo of the app's own write
                }
                log::info!("Outside change detected ({source:?}); refreshing");
                let _ = handle.emit(ENV_CHANGED, ());
            });

            app.manage(manager);
            app.manage(Mutex::new(watcher));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_platform_info,
            commands::list_variables,
            commands::set_variable,
            commands::delete_variable,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Environment Manager");
}
