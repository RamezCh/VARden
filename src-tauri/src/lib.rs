mod commands;

use envcore::manager::EnvManager;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Backups live in the per-user local app data directory, e.g.
            // %LOCALAPPDATA%\dev.chreide.envmanager\backups on Windows.
            let backup_dir = app.path().app_local_data_dir()?.join("backups");
            app.manage(EnvManager::new(backup_dir));
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
