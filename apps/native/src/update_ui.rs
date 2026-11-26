//! Background update checks, downloads, and safe install lifecycle wiring for Slint.
use super::*;
use druckwandel_native::updater::{
    self, UpdateCheckOutcome, UpdateError, UpdateInfo, current_app_version, default_download_dir,
    format_bytes, is_public_key_configured, now_unix_secs, should_auto_check,
};
use std::cell::Cell;
use std::sync::mpsc;

enum UpdateEvent {
    CheckFinished {
        manual: bool,
        result: Result<UpdateCheckOutcome, UpdateError>,
    },
    DownloadProgress {
        downloaded: u64,
        total: u64,
    },
    DownloadFinished {
        version: String,
        result: Result<PathBuf, UpdateError>,
    },
}

fn install_button_label() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Update laden & öffnen"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "Update herunterladen & installieren"
    }
}

fn spawn_check(
    window: &MainWindow,
    in_flight: &Rc<Cell<bool>>,
    sender: &mpsc::Sender<UpdateEvent>,
    manual: bool,
) {
    if in_flight.get() {
        return;
    }
    in_flight.set(true);
    window.set_update_checking(true);
    if manual {
        window.set_update_is_error(false);
        window.set_update_status("Suche nach Updates …".into());
    }
    let tx = sender.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("app-update-check".into())
        .spawn(move || {
            let result = updater::check_for_update();
            let _ = tx.send(UpdateEvent::CheckFinished { manual, result });
        })
    {
        in_flight.set(false);
        window.set_update_checking(false);
        if manual {
            window.set_update_is_error(true);
            window.set_update_status(
                format!("Update-Prüfung konnte nicht gestartet werden: {error}").into(),
            );
        }
    }
}

pub fn wire(
    window: &MainWindow,
    store: &Rc<Workspace>,
    selected: &Rc<RefCell<Option<String>>>,
    draft: &Rc<RefCell<InvoiceInput>>,
    settings: &Rc<RefCell<Settings>>,
) -> slint::Timer {
    window.set_update_auto_check(settings.borrow().updates.auto_check);
    window.set_update_install_label(install_button_label().into());
    if !is_public_key_configured() {
        window.set_update_is_error(false);
        window.set_update_status(
            "Hinweis: In diesem Entwicklungsbuild ist noch kein produktiver Ed25519-Signaturschlüssel hinterlegt."
                .into(),
        );
    }

    let (sender, receiver) = mpsc::channel::<UpdateEvent>();
    let in_flight = Rc::new(Cell::new(false));
    let available_update = Rc::new(RefCell::new(None::<UpdateInfo>));

    let weak = window.as_weak();
    let preferences = Rc::clone(settings);
    let root = store.root().to_owned();
    window.on_update_auto_check_changed(move || {
        let Some(window) = weak.upgrade() else { return };
        preferences.borrow_mut().updates.auto_check = window.get_update_auto_check();
        if let Err(error) = preferences.borrow().save(&root) {
            window.set_settings_status(
                format!("Einstellungen konnten nicht gespeichert werden: {error}").into(),
            );
        }
    });

    let weak = window.as_weak();
    let busy_flag = Rc::clone(&in_flight);
    let check_sender = sender.clone();
    window.on_check_for_updates(move || {
        let Some(window) = weak.upgrade() else { return };
        spawn_check(&window, &busy_flag, &check_sender, true);
    });

    let weak = window.as_weak();
    window.on_open_update_notes(move || {
        let Some(window) = weak.upgrade() else { return };
        let url = window.get_update_notes_url().to_string();
        if url.trim().is_empty() {
            return;
        }
        if let Err(error) = updater::open_release_notes(&url) {
            window.set_update_is_error(true);
            window.set_update_status(
                format!("Release Notes konnten nicht geöffnet werden: {error}").into(),
            );
        }
    });

    let weak = window.as_weak();
    let busy_flag = Rc::clone(&in_flight);
    let available = Rc::clone(&available_update);
    let download_sender = sender.clone();
    let download_dir = default_download_dir(store.root());
    window.on_install_update(move || {
        let Some(window) = weak.upgrade() else { return };
        if busy_flag.get() {
            return;
        }
        if window.global::<OutputState>().get_busy() {
            window.set_update_is_error(false);
            window.set_update_status(
                "Bitte warten, bis der laufende Export oder die Erkennung abgeschlossen ist."
                    .into(),
            );
            return;
        }
        let Some(info) = available.borrow().clone() else {
            return;
        };
        busy_flag.set(true);
        window.set_update_downloading(true);
        window.set_update_is_error(false);
        window.set_update_progress(0.0);
        window.set_update_progress_text(
            format!("0 B / {} (0 %)", format_bytes(info.asset.size_bytes)).into(),
        );
        window.set_update_status(
            format!(
                "Update {} wird heruntergeladen und per SHA-256 geprüft …",
                info.version
            )
            .into(),
        );
        let tx = download_sender.clone();
        let target_dir = download_dir.clone();
        let version = info.version.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("app-update-download".into())
            .spawn(move || {
                let progress_tx = tx.clone();
                let result = updater::download_verified_installer(
                    &info,
                    &target_dir,
                    move |downloaded, total| {
                        let _ =
                            progress_tx.send(UpdateEvent::DownloadProgress { downloaded, total });
                    },
                );
                let _ = tx.send(UpdateEvent::DownloadFinished { version, result });
            })
        {
            busy_flag.set(false);
            window.set_update_downloading(false);
            window.set_update_is_error(true);
            window.set_update_status(
                format!("Download konnte nicht gestartet werden: {error}").into(),
            );
        }
    });

    // Silent automatic startup check respecting the 24h cooldown.
    let update_prefs = settings.borrow().updates.clone();
    if is_public_key_configured()
        && should_auto_check(
            update_prefs.auto_check,
            update_prefs.last_checked_unix,
            now_unix_secs(),
        )
    {
        spawn_check(window, &in_flight, &sender, false);
    }

    let timer = slint::Timer::default();
    let weak = window.as_weak();
    let store = Rc::clone(store);
    let selected = Rc::clone(selected);
    let draft = Rc::clone(draft);
    let preferences = Rc::clone(settings);
    let busy_flag = Rc::clone(&in_flight);
    let available = Rc::clone(&available_update);
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            let Some(window) = weak.upgrade() else { return };
            while let Ok(event) = receiver.try_recv() {
                match event {
                    UpdateEvent::CheckFinished { manual, result } => {
                        busy_flag.set(false);
                        window.set_update_checking(false);
                        match result {
                            Ok(UpdateCheckOutcome::UpToDate { current_version }) => {
                                *available.borrow_mut() = None;
                                window.set_update_available(false);
                                window.set_update_available_version("".into());
                                window.set_update_notes_url("".into());
                                window.set_update_is_error(false);
                                window.set_update_status(
                                    format!(
                                        "Druckwandel ist aktuell (Version {current_version})."
                                    )
                                    .into(),
                                );
                                preferences.borrow_mut().updates.last_checked_unix =
                                    Some(now_unix_secs());
                                let _ = preferences.borrow().save(store.root());
                            }
                            Ok(UpdateCheckOutcome::UpdateAvailable(info)) => {
                                let version = info.version.clone();
                                let size = format_bytes(info.asset.size_bytes);
                                window.set_update_available(true);
                                window.set_update_available_version(version.clone().into());
                                window.set_update_notes_url(info.notes_url.clone().into());
                                window.set_update_is_error(false);
                                window.set_update_status(
                                    format!(
                                        "Version {version} ist verfügbar (aktuell: {}, Paketgröße: {size}).",
                                        current_app_version()
                                    )
                                    .into(),
                                );
                                *available.borrow_mut() = Some(info);
                                preferences.borrow_mut().updates.last_checked_unix =
                                    Some(now_unix_secs());
                                let _ = preferences.borrow().save(store.root());
                            }
                            Err(error) => {
                                if manual {
                                    window.set_update_is_error(true);
                                    window.set_update_status(error.to_string().into());
                                } else {
                                    window.set_update_is_error(false);
                                    window.set_update_status(
                                        format!(
                                            "Automatische Update-Prüfung nicht abgeschlossen: {error}"
                                        )
                                        .into(),
                                    );
                                }
                            }
                        }
                    }
                    UpdateEvent::DownloadProgress { downloaded, total } => {
                        let fraction = if total > 0 {
                            (downloaded as f64 / total as f64).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let percent = (fraction * 100.0).round() as u32;
                        window.set_update_progress(fraction as f32);
                        window.set_update_progress_text(
                            format!(
                                "{} / {} ({percent} %)",
                                format_bytes(downloaded),
                                format_bytes(total)
                            )
                            .into(),
                        );
                    }
                    UpdateEvent::DownloadFinished { version, result } => {
                        busy_flag.set(false);
                        window.set_update_downloading(false);
                        match result {
                            Ok(verified_path) => {
                                if window.global::<OutputState>().get_busy() {
                                    window.set_update_is_error(true);
                                    window.set_update_status(
                                        "Export oder Erkennung läuft noch. Bitte das Update nach Abschluss erneut starten."
                                            .into(),
                                    );
                                    continue;
                                }
                                if let Err(error) = save_current(&window, &store, &selected, &draft)
                                {
                                    let message = format!(
                                        "Update abgebrochen – aktueller Entwurf konnte nicht gespeichert werden: {error}"
                                    );
                                    window.set_update_is_error(true);
                                    window.set_update_status(message.clone().into());
                                    window.set_status_text(message.into());
                                    continue;
                                }
                                match updater::launch_installer(&verified_path) {
                                    Ok(()) => {
                                        window.set_update_is_error(false);
                                        #[cfg(target_os = "windows")]
                                        {
                                            window.set_update_status(
                                                format!(
                                                    "Installer für Version {version} gestartet – Druckwandel wird jetzt beendet …"
                                                )
                                                .into(),
                                            );
                                            let _ = slint::quit_event_loop();
                                        }
                                        #[cfg(target_os = "macos")]
                                        {
                                            window.set_update_status(
                                                format!(
                                                    "Update {version} wurde geprüft und geöffnet. Bitte ziehen Sie Druckwandel.app im geöffneten Fenster in den Ordner „Programme“ und starten Sie Druckwandel danach neu."
                                                )
                                                .into(),
                                            );
                                        }
                                        #[cfg(not(any(
                                            target_os = "windows",
                                            target_os = "macos"
                                        )))]
                                        {
                                            let _ = version;
                                        }
                                    }
                                    Err(error) => {
                                        window.set_update_is_error(true);
                                        window.set_update_status(error.to_string().into());
                                    }
                                }
                            }
                            Err(error) => {
                                window.set_update_is_error(true);
                                window.set_update_status(error.to_string().into());
                            }
                        }
                    }
                }
            }
        },
    );
    timer
}
