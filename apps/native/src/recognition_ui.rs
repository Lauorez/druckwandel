use super::*;
use druckwandel_native::{
    extraction::reconstruct_lines,
    learning::ProfileStore,
    recognition::{
        RecognitionRegistry, RecognitionRequest, RecognitionResult,
        wsystem::{
            UsageInfo, WSYSTEM_BILLING_URL, WSYSTEM_DPA_URL, WSYSTEM_FREE_KEY_URL, WSystemClient,
            WSystemRecognizer,
        },
    },
    secrets::{ApiKey, OsSecretStore, SecretStore},
};
use std::{
    path::Path,
    sync::{Arc, mpsc},
};

struct RecognizedOutcome {
    request: RecognitionRequest,
    result: RecognitionResult,
    units_remaining: Option<u64>,
}

enum Completion {
    Recognized {
        id: String,
        initial: bool,
        outcome: Box<Result<RecognizedOutcome, String>>,
    },
    Learned(Result<String, String>),
    UsageChecked(Result<UsageInfo, String>),
    Failed(String),
}

pub fn source_text(request: &RecognitionRequest) -> String {
    reconstruct_lines(&request.pages)
        .iter()
        .map(|line| format!("Seite {}: {}", line.page, line.text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn open_allowed_url(url: &str) -> Result<(), String> {
    if url != WSYSTEM_FREE_KEY_URL && url != WSYSTEM_BILLING_URL && url != WSYSTEM_DPA_URL {
        return Err("Nicht freigegebene URL.".into());
    }
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    result
        .map(|_| ())
        .map_err(|error| format!("Link konnte nicht im Browser geöffnet werden: {error}"))
}

fn refresh_wsystem_key_state(window: &MainWindow, secrets: &dyn SecretStore) {
    match secrets.has_wsystem_key() {
        Ok(true) => {
            window.set_wsystem_key_saved(true);
            window.set_wsystem_key_status(
                "API-Schlüssel sicher im System-Schlüsselbund gespeichert.".into(),
            );
        }
        Ok(false) => {
            window.set_wsystem_key_saved(false);
            window.set_wsystem_key_status("Kein W-System-API-Schlüssel gespeichert.".into());
        }
        Err(error) => {
            window.set_wsystem_key_saved(false);
            window.set_wsystem_key_status(error.into());
        }
    }
}

fn refresh_profiles(window: &MainWindow, root: &Path) -> Result<(), String> {
    let store = ProfileStore::load(root)?;
    window.set_learning_profiles(ModelRc::new(VecModel::from(
        store
            .profiles
            .iter()
            .map(|p| p.name.clone().into())
            .collect::<Vec<SharedString>>(),
    )));
    window.set_learning_profile_index(
        store
            .profiles
            .iter()
            .position(|p| p.id == store.active)
            .unwrap_or(0) as i32,
    );
    let profile = store.active();
    window.set_learning_name(profile.name.clone().into());
    window.set_learning_can_undo(profile.undo.is_some());
    let rules = profile
        .memory
        .fields
        .iter()
        .map(|rule| LearningRuleData {
            id: rule.id.clone().into(),
            enabled: rule.enabled,
            label: format!(
                "{} · Seite {}",
                druckwandel_native::recognition::field_label(&rule.field),
                rule.page
            )
            .into(),
        })
        .chain(profile.memory.tables.iter().map(|rule| LearningRuleData {
            id: rule.id.clone().into(),
            enabled: rule.enabled,
            label: format!("Positionstabelle · Seite {}", rule.page).into(),
        }))
        .collect::<Vec<_>>();
    window.set_learning_rules(ModelRc::new(VecModel::from(rules)));
    Ok(())
}

fn change_profiles(
    window: &MainWindow,
    root: &Path,
    change: impl FnOnce(&mut ProfileStore) -> Result<(), String>,
) {
    if window.global::<OutputState>().get_busy() {
        return;
    }
    let result = (|| {
        let mut store = ProfileStore::load(root)?;
        change(&mut store)?;
        store.save(root)?;
        refresh_profiles(window, root)
    })();
    window.set_settings_status(match result {
        Ok(()) => "Lernprofil gespeichert.".into(),
        Err(error) => error.into(),
    });
}

fn wire_profiles(window: &MainWindow, root: &Path) {
    if let Err(error) = refresh_profiles(window, root) {
        window.set_settings_status(error.into());
    }
    macro_rules! action {
        ($method:ident, $operation:expr) => {{
            let weak = window.as_weak();
            let root = root.to_owned();
            window.$method(move || {
                let Some(window) = weak.upgrade() else { return };
                change_profiles(&window, &root, |store| ($operation)(store, &window));
            });
        }};
    }
    action!(
        on_select_learning_profile,
        |store: &mut ProfileStore, ui: &MainWindow| {
            let id = store
                .profiles
                .get(ui.get_learning_profile_index() as usize)
                .ok_or("Profil nicht gefunden.")?
                .id
                .clone();
            store.active = id;
            Ok(())
        }
    );
    action!(
        on_create_learning_profile,
        |store: &mut ProfileStore, ui: &MainWindow| store.create(ui.get_learning_name().as_str())
    );
    action!(
        on_rename_learning_profile,
        |store: &mut ProfileStore, ui: &MainWindow| store.rename(ui.get_learning_name().as_str())
    );
    action!(
        on_delete_learning_profile,
        |store: &mut ProfileStore, _: &MainWindow| store.delete()
    );
    action!(
        on_clear_learning_profile,
        |store: &mut ProfileStore, _: &MainWindow| {
            store.clear();
            Ok(())
        }
    );
    action!(
        on_undo_learning_profile,
        |store: &mut ProfileStore, _: &MainWindow| store.undo()
    );
    let weak = window.as_weak();
    let root_toggle = root.to_owned();
    window.on_toggle_learning_rule(move |id| {
        let Some(window) = weak.upgrade() else { return };
        change_profiles(&window, &root_toggle, |store| {
            store.edit_rule(id.as_str(), false)
        });
    });
    let weak = window.as_weak();
    let root_remove = root.to_owned();
    window.on_remove_learning_rule(move |id| {
        let Some(window) = weak.upgrade() else { return };
        change_profiles(&window, &root_remove, |store| {
            store.edit_rule(id.as_str(), true)
        });
    });
}

fn spawn(
    window: &MainWindow,
    sender: mpsc::Sender<Completion>,
    work: impl FnOnce() -> Completion + Send + 'static,
) {
    window.global::<OutputState>().set_busy(true);
    window.set_recognition_running(true);
    if let Err(error) = std::thread::Builder::new()
        .name("invoice-recognition".into())
        .spawn(move || {
            let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| {
                    Completion::Failed(
                        "Diese PDF konnte nicht sicher erkannt werden. Die manuelle Erfassung bleibt verfügbar."
                            .into(),
                    )
                });
            let _ = sender.send(completed);
        })
    {
        window.global::<OutputState>().set_busy(false);
        window.set_recognition_running(false);
        window.set_status_text(error.to_string().into());
    }
}

fn wire_wsystem_settings(
    window: &MainWindow,
    root: &Path,
    settings: &Rc<RefCell<Settings>>,
    secrets: &Arc<dyn SecretStore>,
    client: &WSystemClient,
    sender: &mpsc::Sender<Completion>,
) {
    window.set_wsystem_consent(settings.borrow().wsystem.consent_accepted);
    window.set_wsystem_auto_recognize(
        settings.borrow().wsystem.consent_accepted && settings.borrow().wsystem.auto_recognize,
    );
    window.set_wsystem_api_key_input("".into());
    window.set_wsystem_usage_status("".into());
    window.set_wsystem_usage_checking(false);
    refresh_wsystem_key_state(window, secrets.as_ref());

    let weak = window.as_weak();
    let preferences = Rc::clone(settings);
    let settings_root = root.to_owned();
    window.on_wsystem_settings_changed(move || {
        let Some(window) = weak.upgrade() else { return };
        let consent = window.get_wsystem_consent();
        let auto = consent && window.get_wsystem_auto_recognize();
        window.set_wsystem_auto_recognize(auto);
        preferences.borrow_mut().wsystem = druckwandel_native::settings::WSystemSettings {
            consent_accepted: consent,
            auto_recognize: auto,
        };
        if let Err(error) = preferences.borrow().save(&settings_root) {
            window.set_settings_status(error.into());
        }
    });

    let weak = window.as_weak();
    let store = Arc::clone(secrets);
    window.on_save_wsystem_key(move || {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
        let raw = window.get_wsystem_api_key_input().to_string();
        match ApiKey::parse(&raw).and_then(|key| store.save_wsystem_key(&key)) {
            Ok(()) => {
                window.set_wsystem_api_key_input("".into());
                window.set_wsystem_usage_status("".into());
                refresh_wsystem_key_state(&window, store.as_ref());
                window.set_settings_status(
                    "W-System-API-Schlüssel sicher im System-Schlüsselbund gespeichert.".into(),
                );
            }
            Err(error) => {
                window.set_settings_status(error.into());
            }
        }
    });

    let weak = window.as_weak();
    let store = Arc::clone(secrets);
    window.on_delete_wsystem_key(move || {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
        match store.delete_wsystem_key() {
            Ok(()) => {
                window.set_wsystem_api_key_input("".into());
                window.set_wsystem_usage_status("".into());
                refresh_wsystem_key_state(&window, store.as_ref());
                window.set_settings_status(
                    "W-System-API-Schlüssel aus dem System-Schlüsselbund entfernt.".into(),
                );
            }
            Err(error) => window.set_settings_status(error.into()),
        }
    });

    let weak = window.as_weak();
    let store = Arc::clone(secrets);
    let client = client.clone();
    let usage_sender = sender.clone();
    window.on_check_wsystem_usage(move || {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() || window.get_wsystem_usage_checking() {
            return;
        }
        window.set_wsystem_usage_checking(true);
        window.set_wsystem_usage_status("W-System-Kontingent wird abgerufen …".into());
        let store = Arc::clone(&store);
        let client = client.clone();
        let sender = usage_sender.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("wsystem-usage".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let key = store
                        .load_wsystem_key()?
                        .ok_or_else(|| "Kein W-System-API-Schlüssel gespeichert.".to_owned())?;
                    client.check_usage(&key)
                }))
                .unwrap_or_else(|_| Err("Kontingentabfrage fehlgeschlagen.".into()));
                let _ = sender.send(Completion::UsageChecked(outcome));
            })
        {
            window.set_wsystem_usage_checking(false);
            window.set_wsystem_usage_status(error.to_string().into());
        }
    });

    let weak = window.as_weak();
    window.on_open_wsystem_free_key(move || {
        let Some(window) = weak.upgrade() else { return };
        if let Err(error) = open_allowed_url(WSYSTEM_FREE_KEY_URL) {
            window.set_settings_status(error.into());
        }
    });

    let weak = window.as_weak();
    window.on_open_wsystem_billing(move || {
        let Some(window) = weak.upgrade() else { return };
        if let Err(error) = open_allowed_url(WSYSTEM_BILLING_URL) {
            window.set_settings_status(error.into());
        }
    });

    let weak = window.as_weak();
    window.on_open_wsystem_dpa(move || {
        let Some(window) = weak.upgrade() else { return };
        if let Err(error) = open_allowed_url(WSYSTEM_DPA_URL) {
            window.set_settings_status(error.into());
        }
    });
}

pub fn wire(
    window: &MainWindow,
    store: &Rc<Workspace>,
    selected: &Rc<RefCell<Option<String>>>,
    draft: &Rc<RefCell<InvoiceInput>>,
    saved: &Rc<RefCell<Option<SavedOutput>>>,
    settings: &Rc<RefCell<Settings>>,
) -> slint::Timer {
    let secrets: Arc<dyn SecretStore> = Arc::new(OsSecretStore::for_workspace(store.root()));
    wire_with_wsystem(
        window,
        store,
        selected,
        draft,
        saved,
        settings,
        secrets,
        WSystemClient::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn wire_with_wsystem(
    window: &MainWindow,
    store: &Rc<Workspace>,
    selected: &Rc<RefCell<Option<String>>>,
    draft: &Rc<RefCell<InvoiceInput>>,
    saved: &Rc<RefCell<Option<SavedOutput>>>,
    settings: &Rc<RefCell<Settings>>,
    secrets: Arc<dyn SecretStore>,
    wsystem_client: WSystemClient,
) -> slint::Timer {
    wire_profiles(window, store.root());
    window.set_ocr_enabled(settings.borrow().ocr.enabled);
    window.set_ocr_language(match settings.borrow().ocr.languages.as_str() {
        "deu" => 1,
        "eng" => 2,
        _ => 0,
    });
    window.set_ocr_status(druckwandel_native::ocr::status().into());
    let weak = window.as_weak();
    let preferences = Rc::clone(settings);
    let root = store.root().to_owned();
    window.on_ocr_changed(move || {
        let Some(window) = weak.upgrade() else { return };
        preferences.borrow_mut().ocr = druckwandel_native::ocr::OcrSettings {
            enabled: window.get_ocr_enabled(),
            languages: match window.get_ocr_language() {
                1 => "deu",
                2 => "eng",
                _ => "deu+eng",
            }
            .into(),
        };
        if let Err(error) = preferences.borrow().save(&root) {
            window.set_settings_status(error.into());
        }
    });

    let (sender, receiver) = mpsc::channel::<Completion>();
    wire_wsystem_settings(
        window,
        store.root(),
        settings,
        &secrets,
        &wsystem_client,
        &sender,
    );

    let weak = window.as_weak();
    let current = Rc::clone(selected);
    let preferences = Rc::clone(settings);
    let root = store.root().to_owned();
    let recognition_sender = sender.clone();
    let wsystem_secrets = Arc::clone(&secrets);
    let recognition_client = wsystem_client.clone();
    window.on_start_recognition(move |initial| {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
        let Some(id) = current.borrow().clone() else {
            return;
        };
        let source = PathBuf::from(window.get_selected_path().as_str());
        let hash = window.get_selected_hash().to_string();
        let method = selected_recognition(&window);
        let options = preferences.borrow().ocr.clone();
        let wsystem_settings = preferences.borrow().wsystem.clone();
        if method == RecognitionMethod::WSystem && initial && !wsystem_settings.auto_recognize {
            window.set_recognition_text(
                "W System Cloud-Erkennung ist ausgewählt.\n\nNeue PDFs werden nicht automatisch an W System übertragen. Klicken Sie auf ‚Fehlende Felder über W System erkennen‘, um diese Rechnung an W System (api.wsystem.dev, Unterauftragsverarbeiter OpenAI) zu übermitteln, oder aktivieren Sie das automatische Senden unter Einstellungen → Erkennung."
                    .into(),
            );
            window.set_status_text(
                "W System ausgewählt – zum Übermitteln bitte im Reiter ‚Erkennung‘ starten.".into(),
            );
            return;
        }
        let root = root.clone();
        let secrets = Arc::clone(&wsystem_secrets);
        let client = recognition_client.clone();
        spawn(&window, recognition_sender.clone(), move || {
            let outcome = (|| match method {
                RecognitionMethod::WSystem => {
                    if !wsystem_settings.consent_accepted {
                        return Err(
                            "Die Übermittlung an W System ist noch nicht freigegeben. Bitte unter Einstellungen → Erkennung das Einverständnis bestätigen."
                                .into(),
                        );
                    }
                    let native_only = druckwandel_native::ocr::OcrSettings {
                        enabled: false,
                        languages: options.languages,
                    };
                    let request = RecognitionRequest::from_pdf_with_ocr(source, &hash, &native_only)?;
                    let provider = WSystemRecognizer::new(client, secrets, true);
                    let mut registry = RecognitionRegistry::default();
                    let remaining_tracker = Arc::new(std::sync::Mutex::new(None));
                    let tracked_provider = TrackedWSystem {
                        inner: provider,
                        remaining: Arc::clone(&remaining_tracker),
                    };
                    registry.register(Box::new(tracked_provider))?;
                    let result = registry.recognize(RecognitionMethod::WSystem, &request)?;
                    let units_remaining = remaining_tracker.lock().ok().and_then(|g| *g);
                    Ok(RecognizedOutcome {
                        request,
                        result,
                        units_remaining,
                    })
                }
                _ => {
                    let registry = application_registry();
                    if !registry.available(method) {
                        return Err(
                            "Lokale KI-Erkennung ist noch nicht installiert. Bitte Schemaerkennung wählen oder den separaten lokalen Adapter ergänzen."
                                .into(),
                        );
                    }
                    let request = RecognitionRequest::from_pdf_with_ocr(source, &hash, &options)?;
                    let mut result = registry.recognize(method, &request)?;
                    match ProfileStore::load(&root) {
                        Ok(profiles) => profiles.apply(&request, &mut result),
                        Err(error) => result.warnings.push(error),
                    }
                    Ok(RecognizedOutcome {
                        request,
                        result,
                        units_remaining: None,
                    })
                }
            })();
            Completion::Recognized {
                id,
                initial,
                outcome: Box::new(outcome),
            }
        });
    });

    let weak = window.as_weak();
    window.on_recognize(move || {
        if let Some(window) = weak.upgrade() {
            window.invoke_start_recognition(false);
        }
    });

    let weak = window.as_weak();
    let current = Rc::clone(selected);
    let current_draft = Rc::clone(draft);
    let workspace = Rc::clone(store);
    let root = store.root().to_owned();
    let learning_sender = sender.clone();
    window.on_learn_corrections(move || {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
        let Some(id) = current.borrow().clone() else {
            return;
        };
        if let Err(error) = save_current(&window, &workspace, &current, &current_draft) {
            window.set_status_text(error.into());
            return;
        }
        let invoice = collect_form(&window, current_draft.borrow().clone());
        let root = root.clone();
        let path = PathBuf::from(window.get_selected_path().as_str());
        let hash = window.get_selected_hash().to_string();
        spawn(&window, learning_sender.clone(), move || {
            Completion::Learned((|| {
                let store = Workspace::new(root.clone())?;
                let request = store.load_recognition_source(&id)?.ok_or(
                    "Bitte zuerst ‚Fehlende Felder erkennen‘ ausführen, damit aktuelle Fundstellen verfügbar sind.",
                )?;
                if request.document_sha256 != hash || read_verified_pdf(&path, &hash).is_err() {
                    return Err("Die Quelle wurde verändert. Erkennung erneut starten.".into());
                }
                let mut profiles = ProfileStore::load(&root)?;
                let message = profiles.learn(&request, &invoice)?;
                profiles.save(&root)?;
                Ok(message)
            })())
        });
    });

    let weak = window.as_weak();
    let store = Rc::clone(store);
    let current = Rc::clone(selected);
    let current_draft = Rc::clone(draft);
    let saved = Rc::clone(saved);
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            let Some(window) = weak.upgrade() else { return };
            let Ok(completed) = receiver.try_recv() else {
                return;
            };
            match completed {
                Completion::UsageChecked(result) => {
                    window.set_wsystem_usage_checking(false);
                    match result {
                        Ok(usage) => {
                            let summary = usage.summary();
                            window.set_wsystem_usage_status(summary.clone().into());
                            window.set_settings_status(summary.into());
                        }
                        Err(error) => {
                            window.set_wsystem_usage_status(error.clone().into());
                            window.set_settings_status(error.into());
                        }
                    }
                }
                Completion::Recognized {
                    id,
                    initial,
                    outcome,
                } => {
                    window.global::<OutputState>().set_busy(false);
                    window.set_recognition_running(false);
                    let outcome = (*outcome).and_then(|recognized| {
                        let RecognizedOutcome {
                            request,
                            result,
                            units_remaining,
                        } = recognized;
                        if let Some(units) = units_remaining {
                            let est = units / druckwandel_native::recognition::wsystem::UNITS_PER_EXTRACTION;
                            window.set_wsystem_usage_status(
                                format!("Restkontingent laut letzter Antwort: {units} Einheiten (ca. {est} Rechnungen).")
                                    .into(),
                            );
                        }
                        let is_wsystem = result.method == RecognitionMethod::WSystem;
                        store.save_recognition_source(&id, &request)?;
                        store.save_recognition(&id, &result)?;
                        if current.borrow().as_ref() != Some(&id) {
                            return Ok(());
                        }
                        let mut invoice = if initial {
                            result.invoice_draft()
                        } else {
                            collect_form(&window, current_draft.borrow().clone())
                        };
                        if !initial {
                            druckwandel_native::recognition::fill_missing(&mut invoice, &result);
                        }
                        saved_company(&store)?.fill_missing(&mut invoice);
                        store.save_invoice(&id, &invoice)?;
                        set_form(&window, &invoice);
                        *current_draft.borrow_mut() = invoice;
                        *saved.borrow_mut() = None;
                        window.global::<OutputState>().set_can_email(false);
                        window.set_source_text(source_text(&request).into());
                        window.set_recognition_text(result.review_text().into());
                        window.set_recognition_notice(
                            if is_wsystem {
                                "Die Rechnungsdaten wurden über W System Cloud erkannt. Bitte prüfen Sie alle Felder, Beträge, Steuersätze und Positionen sorgfältig anhand der Original-PDF, bevor Sie die E-Rechnung erstellen oder versenden."
                            } else {
                                "Die Rechnungsdaten wurden automatisch aus der PDF erkannt. Bitte prüfen Sie alle Felder, Beträge, Steuersätze und Positionen sorgfältig anhand der Original-PDF, bevor Sie die E-Rechnung erstellen oder versenden."
                            }
                            .into(),
                        );
                        window.set_status_text(
                            if is_wsystem {
                                "W System Cloud-Erkennung abgeschlossen. Alle Vorschläge, Zahlen und Positionen bitte am Original prüfen."
                            } else {
                                "Erkennung abgeschlossen. Vorschläge, Zahlen und Positionen bitte am Original prüfen."
                            }
                            .into(),
                        );
                        Ok(())
                    });
                    if let Err(error) = outcome {
                        window.set_recognition_text(
                            format!(
                                "Erkennung fehlgeschlagen: {error}\nDie manuelle Erfassung und PDF-Vorschau bleiben verfügbar."
                            )
                            .into(),
                        );
                        window.set_status_text(error.into());
                    }
                }
                Completion::Learned(result) => {
                    window.global::<OutputState>().set_busy(false);
                    window.set_recognition_running(false);
                    match result {
                        Ok(message) => {
                            window.set_status_text(message.clone().into());
                            window.set_settings_status(message.into());
                        }
                        Err(error) => window.set_status_text(error.into()),
                    }
                    if let Err(error) = refresh_profiles(&window, store.root()) {
                        window.set_settings_status(error.into());
                    }
                }
                Completion::Failed(error) => {
                    window.global::<OutputState>().set_busy(false);
                    window.set_recognition_running(false);
                    window.set_status_text(error.into());
                }
            }
        },
    );
    timer
}

struct TrackedWSystem {
    inner: WSystemRecognizer,
    remaining: Arc<std::sync::Mutex<Option<u64>>>,
}

impl druckwandel_native::recognition::RecognitionProvider for TrackedWSystem {
    fn method(&self) -> RecognitionMethod {
        self.inner.method()
    }

    fn recognize(&self, request: &RecognitionRequest) -> Result<RecognitionResult, String> {
        let result = self.inner.recognize(request)?;
        if let Ok(mut guard) = self.remaining.lock() {
            *guard = self.inner.last_units_remaining();
        }
        Ok(result)
    }
}
