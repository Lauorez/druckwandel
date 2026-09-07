use std::path::PathBuf;
use tauri::{AppHandle, Manager};

// Debug-only test isolation; release builds never accept an environment override.
fn test_root() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(root) = std::env::var_os("ERECHNUNG_TEST_ROOT") {
        let root = PathBuf::from(root);
        if root.is_absolute() {
            return Some(root);
        }
    }
    None
}

pub(crate) fn documents() -> Result<PathBuf, String> {
    test_root()
        .map(|p| p.join("documents"))
        .or_else(dirs::document_dir)
        .ok_or_else(|| "Dokumente-Ordner konnte nicht ermittelt werden.".into())
}

pub(crate) fn app_data(app: &AppHandle, local: bool) -> Result<PathBuf, String> {
    if let Some(root) = test_root() {
        return Ok(root.join(if local { "local" } else { "roaming" }));
    }
    (if local {
        app.path().app_local_data_dir()
    } else {
        app.path().app_data_dir()
    })
    .map_err(|_| "Der lokale Anwendungsspeicher ist nicht verfügbar.".into())
}
