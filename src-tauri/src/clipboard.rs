//! Copying values. Secrets are kept out of clipboard history (Windows' Win+V
//! list, cloud clipboard sync, Mac and Linux clipboard managers) and are
//! cleared again after 30 seconds, unless something else was copied since.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a copied secret stays on the clipboard.
pub const SECRET_LIFETIME: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct Clipboard {
    // On Linux the clipboard's contents are served by the process that set
    // them, so the handle has to live as long as the app.
    handle: Mutex<Option<arboard::Clipboard>>,
    // Bumped on every copy, so an older timer never clears a newer copy.
    generation: Arc<AtomicU64>,
}

impl Clipboard {
    fn with<T>(
        &self,
        f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
    ) -> Result<T, String> {
        let mut guard = self.handle.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            *guard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        f(guard.as_mut().expect("just set")).map_err(|e| e.to_string())
    }

    pub fn copy(self: &Arc<Self>, text: String, secret: bool) -> Result<(), String> {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.with(|c| {
            let set = c.set();
            if !secret {
                return set.text(text.clone());
            }
            #[cfg(windows)]
            let set = {
                use arboard::SetExtWindows;
                set.exclude_from_history()
                    .exclude_from_cloud()
                    .exclude_from_monitoring()
            };
            #[cfg(target_os = "macos")]
            let set = {
                use arboard::SetExtApple;
                set.exclude_from_history()
            };
            #[cfg(target_os = "linux")]
            let set = {
                use arboard::SetExtLinux;
                set.exclude_from_history()
            };
            set.text(text.clone())
        })?;
        if secret {
            let me = Arc::clone(self);
            std::thread::spawn(move || {
                std::thread::sleep(SECRET_LIFETIME);
                if me.generation.load(Ordering::SeqCst) != generation {
                    return; // something newer was copied through VARden
                }
                // Only clear our own copy, never something the user copied
                // elsewhere since.
                let still_ours = me.with(|c| c.get_text()).is_ok_and(|t| t == text);
                if still_ours && me.with(|c| c.clear()).is_ok() {
                    log::info!("Cleared a copied secret from the clipboard");
                }
            });
        }
        Ok(())
    }
}
