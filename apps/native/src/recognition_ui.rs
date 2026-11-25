use super::*;
use druckwandel_native::{
    extraction::reconstruct_lines,
    learning::ProfileStore,
    recognition::{RecognitionRequest, RecognitionResult},
};
use std::{path::Path, sync::mpsc};

enum Completion {
    Recognized {
        id: String,
        initial: bool,
        outcome: Box<Result<(RecognitionRequest, RecognitionResult), String>>,
    },
    Learned(Result<String, String>),
    Failed(String),
}

pub fn source_text(request: &RecognitionRequest) -> String {
    reconstruct_lines(&request.pages)
        .iter()
        .map(|line| format!("Seite {}: {}", line.page, line.text))
        .collect::<Vec<_>>()
        .join("\n")
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
    if let Err(error)=std::thread::Builder::new().name("invoice-recognition".into()).spawn(move || {
        let completed=std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or_else(|_|Completion::Failed("Diese PDF konnte nicht sicher erkannt werden. Die manuelle Erfassung bleibt verfügbar.".into()));
        let _=sender.send(completed);
    }) {
        window.global::<OutputState>().set_busy(false); window.set_recognition_running(false); window.set_status_text(error.to_string().into());
    }
}

pub fn wire(
    window: &MainWindow,
    store: &Rc<Workspace>,
    selected: &Rc<RefCell<Option<String>>>,
    draft: &Rc<RefCell<InvoiceInput>>,
    saved: &Rc<RefCell<Option<SavedOutput>>>,
    settings: &Rc<RefCell<Settings>>,
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
    let weak = window.as_weak();
    let current = Rc::clone(selected);
    let preferences = Rc::clone(settings);
    let root = store.root().to_owned();
    let recognition_sender = sender.clone();
    window.on_start_recognition(move |initial| {
        let Some(window)=weak.upgrade() else {return};
        if window.global::<OutputState>().get_busy(){return;}
        let Some(id)=current.borrow().clone() else {return};
        let source=PathBuf::from(window.get_selected_path().as_str()); let hash=window.get_selected_hash().to_string();
        let method=selected_recognition(&window); let options=preferences.borrow().ocr.clone(); let root=root.clone();
        spawn(&window,recognition_sender.clone(),move || {
            let outcome=(|| {
                let registry=application_registry();
                if !registry.available(method){return Err("Lokale KI-Erkennung ist noch nicht installiert. Bitte Schemaerkennung wählen oder den separaten lokalen Adapter ergänzen.".into());}
                let request=RecognitionRequest::from_pdf_with_ocr(source,&hash,&options)?;
                let mut result=registry.recognize(method,&request)?;
                match ProfileStore::load(&root) { Ok(profiles)=>profiles.apply(&request,&mut result),Err(error)=>result.warnings.push(error) }
                Ok((request,result))
            })();
            Completion::Recognized {id,initial,outcome: Box::new(outcome)}
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
        let Some(window)=weak.upgrade() else {return};
        if window.global::<OutputState>().get_busy(){return;}
        let Some(id)=current.borrow().clone() else {return};
        if let Err(error)=save_current(&window,&workspace,&current,&current_draft){window.set_status_text(error.into());return;}
        let invoice=collect_form(&window,current_draft.borrow().clone()); let root=root.clone();
        let path=PathBuf::from(window.get_selected_path().as_str()); let hash=window.get_selected_hash().to_string();
        spawn(&window,learning_sender.clone(),move || Completion::Learned((|| {
            let store = Workspace::new(root.clone())?;
            let request = store.load_recognition_source(&id)?.ok_or("Bitte zuerst ‚Fehlende Felder erkennen‘ ausführen, damit aktuelle Fundstellen verfügbar sind.")?;
            if request.document_sha256 != hash || read_verified_pdf(&path, &hash).is_err() {
                return Err("Die Quelle wurde verändert. Erkennung erneut starten.".into());
            }
            let mut profiles = ProfileStore::load(&root)?;
            let message=profiles.learn(&request,&invoice)?; profiles.save(&root)?; Ok(message)
        })()));
    });
    let weak = window.as_weak();
    let store = Rc::clone(store);
    let current = Rc::clone(selected);
    let current_draft = Rc::clone(draft);
    let saved = Rc::clone(saved);
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,std::time::Duration::from_millis(100),move || {
        let Some(window)=weak.upgrade() else {return};
        let Ok(completed)=receiver.try_recv() else {return};
        window.global::<OutputState>().set_busy(false); window.set_recognition_running(false);
        match completed {
            Completion::Recognized {id,initial,outcome} => {
                let outcome=(*outcome).and_then(|(request,result)| {
                    store.save_recognition_source(&id,&request)?; store.save_recognition(&id,&result)?;
                    if current.borrow().as_ref()!=Some(&id){return Ok(());}
                    let mut invoice=if initial {result.invoice_draft()}else{collect_form(&window,current_draft.borrow().clone())};
                    if !initial {druckwandel_native::recognition::fill_missing(&mut invoice,&result);}
                    saved_company(&store)?.fill_missing(&mut invoice);
                    store.save_invoice(&id,&invoice)?; set_form(&window,&invoice); *current_draft.borrow_mut()=invoice;
                    *saved.borrow_mut()=None; window.global::<OutputState>().set_can_email(false);
                    window.set_source_text(source_text(&request).into()); window.set_recognition_text(result.review_text().into());
                    window.set_status_text("Erkennung abgeschlossen. Vorschläge, Zahlen und Positionen bitte am Original prüfen.".into()); Ok(())
                });
                if let Err(error)=outcome {window.set_recognition_text(format!("Erkennung fehlgeschlagen: {error}\nDie manuelle Erfassung und PDF-Vorschau bleiben verfügbar.").into());window.set_status_text(error.into());}
            }
            Completion::Learned(result)=> {
                match result {Ok(message)=>{window.set_status_text(message.clone().into());window.set_settings_status(message.into());},Err(error)=>window.set_status_text(error.into())}
                if let Err(error)=refresh_profiles(&window,store.root()){window.set_settings_status(error.into());}
            }
            Completion::Failed(error)=>window.set_status_text(error.into()),
        }
    });
    timer
}
