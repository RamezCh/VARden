//! The tray icon: open the window, sync terminals, start with the computer, quit.

use envcore::manager::EnvManager;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_autostart::ManagerExt;

use crate::focus_main_window;

/// Emitted after a sync started from the tray, with the report as payload.
pub const TRAY_SYNCED: &str = "tray-synced";

/// Menu items whose state changes while the app runs.
pub struct TrayItems {
    pub autostart: CheckMenuItem<Wry>,
}

pub fn is_available(app: &AppHandle) -> bool {
    app.try_state::<TrayItems>().is_some()
}

/// Keep the tray's checkmark in step when autostart changes elsewhere.
pub fn set_autostart_checked(app: &AppHandle, on: bool) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.autostart.set_checked(on);
    }
}

pub fn autostart_label() -> &'static str {
    if cfg!(windows) {
        "Start with Windows"
    } else if cfg!(target_os = "macos") {
        "Open at login"
    } else {
        "Start when you log in"
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let open = MenuItem::with_id(app, "open", "Open VARden", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "Sync to terminals", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        autostart_label(),
        true,
        enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit VARden", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &sync,
            &PredefinedMenuItem::separator(app)?,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("VARden")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => focus_main_window(app),
            "sync" => sync_from_tray(app),
            "autostart" => toggle_autostart(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // A left click opens the window; the menu is on right click.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                focus_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    app.manage(TrayItems { autostart });
    Ok(())
}

fn sync_from_tray(app: &AppHandle) {
    let mgr = app.state::<EnvManager>();
    match mgr.sync_terminals() {
        Ok(report) => {
            log::info!(
                "Synced terminals from the tray (sync {}, {} pushed)",
                report.generation,
                report.pushed
            );
            // The window refreshes itself on this event (it isn't an
            // outside change, so ENV_CHANGED would mislabel it).
            let _ = app.emit(TRAY_SYNCED, &report);
        }
        Err(e) => log::error!("Sync from the tray failed: {e}"),
    }
}

fn toggle_autostart(app: &AppHandle) {
    let launcher = app.autolaunch();
    let on = !launcher.is_enabled().unwrap_or(false);
    let result = if on {
        launcher.enable()
    } else {
        launcher.disable()
    };
    match result {
        Ok(()) => log::info!("Start with the computer: {on}"),
        Err(e) => log::error!("Couldn't change start with the computer: {e}"),
    }
    // Reflect the real state, whether or not the change worked.
    set_autostart_checked(app, launcher.is_enabled().unwrap_or(false));
    let _ = app.emit("autostart-changed", ());
}
