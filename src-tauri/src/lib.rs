mod commands;

use envcore::backup::BackupStore;
use envcore::manager::EnvManager;
use envcore::paths;
use tauri::Manager;

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
        Ok(_) => {
            let _ = std::fs::remove_dir_all(&legacy);
        }
        Err(e) => eprintln!(
            "Keeping {}: could not move all backups: {e}",
            legacy.display()
        ),
    }
}

pub fn run() {
    migrate_legacy_data();
    tauri::Builder::default()
        .setup(|app| {
            // %LOCALAPPDATA%\EnvManager\backups on Windows.
            app.manage(EnvManager::new(paths::backups_dir()));
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
            commands::restore_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Environment Manager");
}
