#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use druckwandel_native::extraction::extract_pages;
use druckwandel_native::invoice::{Contact, ElectronicAddress, InvoiceInput, InvoiceLine};
use druckwandel_native::output::{SavedOutput, save_xrechnung, save_zugferd};
use druckwandel_native::print_inbox::{self, PrintJob};
use druckwandel_native::recognition::{
    RecognitionMethod, RecognitionRequest, application_registry,
};
use druckwandel_native::validation::ValidatorBundle;
use druckwandel_native::workspace::{Document, Workspace};
use slint::{CloseRequestResponse, ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

slint::include_modules!();

fn optional(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn endpoint(value: String) -> Option<ElectronicAddress> {
    optional(value).map(|value| ElectronicAddress {
        scheme_id: if value.contains('@') { "EM" } else { "0204" }.into(),
        value,
    })
}

fn set_line_form(window: &MainWindow, line: Option<&InvoiceLine>) {
    window.set_line_name(line.map(|x| x.name.clone()).unwrap_or_default().into());
    window.set_line_quantity(line.map(|x| x.quantity.clone()).unwrap_or_default().into());
    window.set_line_price(
        line.map(|x| x.net_unit_price.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_line_tax_rate(line.map(|x| x.tax.rate.clone()).unwrap_or_default().into());
}

fn refresh_line_labels(window: &MainWindow, invoice: &InvoiceInput) {
    window.set_line_labels(ModelRc::new(VecModel::from(
        invoice
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let name = line.name.trim();
                if name.is_empty() {
                    format!("{}", index + 1).into()
                } else {
                    format!(
                        "{}: {}",
                        index + 1,
                        name.chars().take(12).collect::<String>()
                    )
                    .into()
                }
            })
            .collect::<Vec<SharedString>>(),
    )));
}

fn set_form(window: &MainWindow, invoice: &InvoiceInput) {
    window.set_invoice_number(invoice.invoice_number.clone().into());
    window.set_issue_date(invoice.issue_date.clone().into());
    window.set_due_date(invoice.due_date.clone().unwrap_or_default().into());
    window.set_buyer_reference(invoice.buyer_reference.clone().into());
    window.set_seller_name(invoice.seller.name.clone().into());
    window.set_seller_street(invoice.seller.address.line1.clone().into());
    window.set_seller_postal(invoice.seller.address.postal_code.clone().into());
    window.set_seller_city(invoice.seller.address.city.clone().into());
    window.set_seller_country(invoice.seller.address.country_code.clone().into());
    window.set_seller_vat(invoice.seller.vat_id.clone().unwrap_or_default().into());
    window.set_seller_endpoint(
        invoice
            .seller
            .electronic_address
            .as_ref()
            .map(|x| x.value.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_seller_contact_name(
        invoice
            .seller
            .contact
            .as_ref()
            .and_then(|x| x.name.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_seller_contact_phone(
        invoice
            .seller
            .contact
            .as_ref()
            .and_then(|x| x.phone.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_seller_contact_email(
        invoice
            .seller
            .contact
            .as_ref()
            .and_then(|x| x.email.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_buyer_name(invoice.buyer.name.clone().into());
    window.set_buyer_street(invoice.buyer.address.line1.clone().into());
    window.set_buyer_postal(invoice.buyer.address.postal_code.clone().into());
    window.set_buyer_city(invoice.buyer.address.city.clone().into());
    window.set_buyer_country(invoice.buyer.address.country_code.clone().into());
    window.set_buyer_endpoint(
        invoice
            .buyer
            .electronic_address
            .as_ref()
            .map(|x| x.value.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_selected_line(0);
    set_line_form(window, invoice.lines.first());
    refresh_line_labels(window, invoice);
    window.set_payment_iban(invoice.payment.iban.clone().unwrap_or_default().into());
}

fn collect_form(window: &MainWindow, mut invoice: InvoiceInput) -> InvoiceInput {
    invoice.invoice_number = window.get_invoice_number().to_string();
    invoice.issue_date = window.get_issue_date().to_string();
    invoice.due_date = optional(window.get_due_date().to_string());
    invoice.buyer_reference = window.get_buyer_reference().to_string();
    invoice.seller.name = window.get_seller_name().to_string();
    invoice.seller.address.line1 = window.get_seller_street().to_string();
    invoice.seller.address.postal_code = window.get_seller_postal().to_string();
    invoice.seller.address.city = window.get_seller_city().to_string();
    invoice.seller.address.country_code = window.get_seller_country().to_string();
    invoice.seller.vat_id = optional(window.get_seller_vat().to_string());
    invoice.seller.electronic_address = endpoint(window.get_seller_endpoint().to_string());
    invoice.seller.contact = Some(Contact {
        name: optional(window.get_seller_contact_name().to_string()),
        phone: optional(window.get_seller_contact_phone().to_string()),
        email: optional(window.get_seller_contact_email().to_string()),
    });
    invoice.buyer.name = window.get_buyer_name().to_string();
    invoice.buyer.address.line1 = window.get_buyer_street().to_string();
    invoice.buyer.address.postal_code = window.get_buyer_postal().to_string();
    invoice.buyer.address.city = window.get_buyer_city().to_string();
    invoice.buyer.address.country_code = window.get_buyer_country().to_string();
    invoice.buyer.electronic_address = endpoint(window.get_buyer_endpoint().to_string());
    if let Some(line) = invoice.lines.get_mut(window.get_selected_line() as usize) {
        line.name = window.get_line_name().to_string();
        line.quantity = window.get_line_quantity().to_string();
        line.net_unit_price = window.get_line_price().to_string();
        line.tax.rate = window.get_line_tax_rate().to_string();
    }
    invoice.payment.iban = optional(window.get_payment_iban().to_string());
    invoice
}

fn open_document(
    window: &MainWindow,
    store: &Workspace,
    document: &Document,
    current_id: &RefCell<Option<String>>,
    current_draft: &RefCell<InvoiceInput>,
) -> Result<(), String> {
    if !store.has_invoice(&document.id)? {
        let request = RecognitionRequest::from_pdf(document.path.clone(), &document.sha256)?;
        let result = application_registry().recognize(RecognitionMethod::Schema, &request)?;
        store.save_invoice(&document.id, &result.invoice_draft())?;
        store.save_recognition(&document.id, &result)?;
    }
    let invoice = store.load_invoice(&document.id)?;
    window.set_recognition_text(
        store
            .load_recognition(&document.id)?
            .map(|result| result.review_text())
            .unwrap_or_default()
            .into(),
    );
    set_form(window, &invoice);
    *current_id.borrow_mut() = Some(document.id.clone());
    *current_draft.borrow_mut() = invoice;
    show_document(window, document)
}

fn save_current(
    window: &MainWindow,
    store: &Workspace,
    current_id: &RefCell<Option<String>>,
    current_draft: &RefCell<InvoiceInput>,
) -> Result<(), String> {
    let Some(id) = current_id.borrow().clone() else {
        return Ok(());
    };
    let invoice = collect_form(window, current_draft.borrow().clone());
    store.save_invoice(&id, &invoice)?;
    *current_draft.borrow_mut() = invoice;
    Ok(())
}

fn show_document(window: &MainWindow, document: &Document) -> Result<(), String> {
    window.set_selected_name(document.name.clone().into());
    window.set_selected_path(document.path.to_string_lossy().into_owned().into());
    window.set_selected_hash(document.sha256.clone().into());
    window.set_source_text("".into());
    let pages = extract_pages(&document.path)?;
    let no_text = pages.iter().all(|page| page.trim().is_empty());
    window.set_source_text(pages.join("\n\n--- Seite ---\n\n").into());
    if no_text {
        return Err(
            "Die PDF enthält keinen lesbaren Text. Eine manuelle Erfassung wird benötigt.".into(),
        );
    }
    Ok(())
}

fn refresh(window: &MainWindow, workspace: &Workspace) -> Result<Vec<Document>, String> {
    let documents = workspace.list()?;
    let names: Vec<SharedString> = documents.iter().map(|d| d.name.clone().into()).collect();
    window.set_documents(ModelRc::new(VecModel::from(names)));
    Ok(documents)
}

fn receive_print_job(
    window: &MainWindow,
    workspace: &Workspace,
    documents: &RefCell<Vec<Document>>,
    current_id: &RefCell<Option<String>>,
    current_draft: &RefCell<InvoiceInput>,
    saved_output: &RefCell<Option<SavedOutput>>,
    job: &PrintJob,
) -> Result<(), String> {
    save_current(window, workspace, current_id, current_draft)?;
    let document = print_inbox::import_job(workspace, job)?;
    *documents.borrow_mut() = refresh(window, workspace)?;
    *saved_output.borrow_mut() = None;
    window.global::<OutputState>().set_can_email(false);
    open_document(window, workspace, &document, current_id, current_draft)
}

fn receive_argument(
    window: &MainWindow,
    workspace: &Workspace,
    documents: &RefCell<Vec<Document>>,
    current_id: &RefCell<Option<String>>,
    current_draft: &RefCell<InvoiceInput>,
    saved_output: &RefCell<Option<SavedOutput>>,
    argument: &str,
) {
    let result = if argument
        .to_ascii_lowercase()
        .starts_with("erechnung-review:")
    {
        print_inbox::job_from_uri(argument).and_then(|job| {
            receive_print_job(
                window,
                workspace,
                documents,
                current_id,
                current_draft,
                saved_output,
                &job,
            )
        })
    } else {
        (|| {
            save_current(window, workspace, current_id, current_draft)?;
            let document = workspace.import_pdf(std::path::Path::new(argument))?;
            *documents.borrow_mut() = refresh(window, workspace)?;
            *saved_output.borrow_mut() = None;
            window.global::<OutputState>().set_can_email(false);
            open_document(window, workspace, &document, current_id, current_draft)
        })()
    };
    match result {
        Ok(()) => window.set_status_text("Dokument aus der Übergabe geöffnet.".into()),
        Err(error) => window.set_status_text(format!("Übergabe fehlgeschlagen: {error}").into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = dirs::data_local_dir()
        .ok_or("Kein lokaler Datenordner verfügbar")?
        .join("Druckwandel")
        .join("native");
    let workspace = Rc::new(Workspace::new(root.clone())?);
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let instance_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("app.lock"))?;
    match fs2::FileExt::try_lock_exclusive(&instance_file) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            workspace.enqueue_handoff(&arguments)?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }
    let (output_sender, output_receiver) =
        std::sync::mpsc::channel::<Result<SavedOutput, String>>();
    let window = MainWindow::new()?;
    let documents = Rc::new(RefCell::new(refresh(&window, &workspace)?));
    let current_id = Rc::new(RefCell::new(None::<String>));
    let current_draft = Rc::new(RefCell::new(InvoiceInput::blank()));
    let saved_output = Rc::new(RefCell::new(None::<SavedOutput>));
    window.set_status_text("PDF importieren oder gespeicherten Entwurf auswählen.".into());

    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let list = Rc::clone(&documents);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    let saved = Rc::clone(&saved_output);
    window.on_import_pdf(move || {
        let Some(window) = weak.upgrade() else { return };
        if let Err(error) = save_current(&window, &store, &selected, &draft) {
            window.set_status_text(
                format!("Entwurf konnte nicht gespeichert werden: {error}").into(),
            );
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .pick_file()
        else {
            return;
        };
        match store.import_pdf(&path).and_then(|document| {
            *list.borrow_mut() = refresh(&window, &store)?;
            Ok(document)
        }) {
            Ok(document) => {
                *saved.borrow_mut() = None;
                window.global::<OutputState>().set_can_email(false);
                match open_document(&window, &store, &document, &selected, &draft) {
                    Ok(()) => window.set_status_text("PDF als Entwurf gespeichert.".into()),
                    Err(error) => {
                        window.set_status_text(format!("PDF gespeichert. {error}").into())
                    }
                }
            }
            Err(error) => window.set_status_text(format!("Import fehlgeschlagen: {error}").into()),
        }
    });

    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let list = Rc::clone(&documents);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    let saved = Rc::clone(&saved_output);
    window.on_import_print_job(move || {
        let Some(window) = weak.upgrade() else { return };
        match print_inbox::pending_jobs(&store) {
            Ok(jobs) if jobs.is_empty() => {
                window.set_status_text("Keine neuen Druckaufträge im Druckeingang.".into());
            }
            Ok(jobs) => {
                let job = &jobs[0];
                match receive_print_job(&window, &store, &list, &selected, &draft, &saved, job) {
                    Ok(()) => window.set_status_text(
                        format!(
                            "Druckauftrag geöffnet: {}. Weitere Aufträge: {}",
                            job.name,
                            jobs.len() - 1
                        )
                        .into(),
                    ),
                    Err(error) => window.set_status_text(
                        format!("Druckauftrag konnte nicht geöffnet werden: {error}").into(),
                    ),
                }
            }
            Err(error) => window.set_status_text(
                format!("Druckeingang konnte nicht gelesen werden: {error}").into(),
            ),
        }
    });

    let selection_list = Rc::clone(&documents);
    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    let saved = Rc::clone(&saved_output);
    window.on_select_document(move |index| {
        let Some(window) = weak.upgrade() else { return };
        if let Err(error) = save_current(&window, &store, &selected, &draft) {
            window.set_status_text(
                format!("Entwurf konnte nicht gespeichert werden: {error}").into(),
            );
            return;
        }
        let Some(document) = selection_list.borrow().get(index as usize).cloned() else {
            return;
        };
        *saved.borrow_mut() = None;
        window.global::<OutputState>().set_can_email(false);
        match open_document(&window, &store, &document, &selected, &draft) {
            Ok(()) => window.set_status_text(format!("Entwurf {} geöffnet.", document.id).into()),
            Err(error) => window.set_status_text(error.into()),
        }
    });

    let weak = window.as_weak();
    let draft = Rc::clone(&current_draft);
    window.on_select_line(move |index| {
        let Some(window) = weak.upgrade() else { return };
        let invoice = collect_form(&window, draft.borrow().clone());
        let Some(line) = invoice.lines.get(index as usize) else {
            return;
        };
        window.set_selected_line(index);
        set_line_form(&window, Some(line));
        refresh_line_labels(&window, &invoice);
        *draft.borrow_mut() = invoice;
    });

    let weak = window.as_weak();
    let draft = Rc::clone(&current_draft);
    window.on_add_line(move || {
        let Some(window) = weak.upgrade() else { return };
        let mut invoice = collect_form(&window, draft.borrow().clone());
        let mut line = InvoiceInput::blank().lines.remove(0);
        line.id = (invoice.lines.len() + 1).to_string();
        invoice.lines.push(line);
        window.set_selected_line((invoice.lines.len() - 1) as i32);
        set_line_form(&window, invoice.lines.last());
        refresh_line_labels(&window, &invoice);
        window.global::<OutputState>().set_can_email(false);
        *draft.borrow_mut() = invoice;
    });

    let weak = window.as_weak();
    let draft = Rc::clone(&current_draft);
    window.on_remove_line(move || {
        let Some(window) = weak.upgrade() else { return };
        let mut invoice = collect_form(&window, draft.borrow().clone());
        if invoice.lines.len() <= 1 {
            return;
        }
        let index = (window.get_selected_line() as usize).min(invoice.lines.len() - 1);
        invoice.lines.remove(index);
        for (index, line) in invoice.lines.iter_mut().enumerate() {
            line.id = (index + 1).to_string();
        }
        let next = index.min(invoice.lines.len() - 1);
        window.set_selected_line(next as i32);
        set_line_form(&window, invoice.lines.get(next));
        refresh_line_labels(&window, &invoice);
        window.global::<OutputState>().set_can_email(false);
        *draft.borrow_mut() = invoice;
    });

    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    let saved = Rc::clone(&saved_output);
    window.on_recognize(move || {
        let Some(window)=weak.upgrade() else {return};
        let Some(id)=selected.borrow().clone() else {return};
        let method=if window.get_recognition_method()==1 {RecognitionMethod::LocalAi} else {RecognitionMethod::Schema};
        let result=(|| {
            let registry=application_registry();
            // Check availability before reading a PDF or touching the existing draft.
            if !registry.available(method) {return Err("Lokale KI-Erkennung ist noch nicht installiert. Der lokale Adapter aus dem separaten KI-Projekt kann später ergänzt werden.".to_string());}
            let request=RecognitionRequest::from_pdf(PathBuf::from(window.get_selected_path().as_str()),window.get_selected_hash().as_str())?;
            registry.recognize(method,&request)
        })();
        match result {
            Ok(result)=>{
                let mut invoice=collect_form(&window,draft.borrow().clone());
                // Re-running recognition fills missing values only. Existing corrections remain.
                druckwandel_native::recognition::fill_missing(&mut invoice,&result);
                if let Err(error)=store.save_invoice(&id,&invoice).and_then(|_|store.save_recognition(&id,&result)) {window.set_status_text(error.into());return;}
                set_form(&window,&invoice);*draft.borrow_mut()=invoice;*saved.borrow_mut()=None;window.global::<OutputState>().set_can_email(false);
                window.set_recognition_text(result.review_text().into());window.set_status_text("Erkennungsvorschläge übernommen. Bitte Fundstellen und Rechnungsdaten prüfen.".into());
            },
            Err(error)=>window.set_status_text(error.into()),
        }
    });

    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    window.on_save_draft(move || {
        let Some(window) = weak.upgrade() else { return };
        let Some(id) = selected.borrow().clone() else {
            return;
        };
        let invoice = collect_form(&window, draft.borrow().clone());
        match store.save_invoice(&id, &invoice) {
            Ok(()) => {
                *draft.borrow_mut() = invoice;
                refresh_line_labels(&window, &draft.borrow());
                window.set_status_text("Rechnungsentwurf gespeichert.".into());
            }
            Err(error) => window.set_status_text(
                format!("Entwurf konnte nicht gespeichert werden: {error}").into(),
            ),
        }
    });

    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    let saved = Rc::clone(&saved_output);
    window.on_save_xml(move || {
        let Some(window) = weak.upgrade() else { return };
        let Some(id) = selected.borrow().clone() else {
            return;
        };
        let invoice = collect_form(&window, draft.borrow().clone());
        if let Err(error) = store.save_invoice(&id, &invoice) {
            window.set_status_text(
                format!("Entwurf konnte nicht gespeichert werden: {error}").into(),
            );
            return;
        }
        *draft.borrow_mut() = invoice.clone();
        let zugferd = window.get_output_format() == 1;
        let extension = if zugferd { "pdf" } else { "xml" };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(if zugferd { "ZUGFeRD" } else { "XRechnung" }, &[extension])
            .set_file_name(format!(
                "{}.{}",
                invoice.invoice_number.trim().replace(['/', '\\'], "-"),
                extension
            ))
            .save_file()
        else {
            return;
        };
        let source = PathBuf::from(window.get_selected_path().as_str());
        if source == path
            || source
                .canonicalize()
                .ok()
                .zip(path.canonicalize().ok())
                .is_some_and(|(a, b)| a == b)
        {
            window.set_status_text(
                "Bitte einen anderen Speicherort als die Original-PDF wählen.".into(),
            );
            return;
        }
        window.set_status_text("Ausgabe wird erzeugt und unabhängig geprüft ...".into());
        *saved.borrow_mut() = None;
        window.global::<OutputState>().set_can_email(false);
        window.global::<OutputState>().set_busy(true);
        let sender = output_sender.clone();
        std::thread::spawn(move || {
            let result = ValidatorBundle::discover().and_then(|bundle| {
                if zugferd {
                    let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
                    save_zugferd(invoice, &bytes, &path, &bundle)
                } else {
                    save_xrechnung(invoice, &path, &bundle)
                }
            });
            let _ = sender.send(result);
        });
    });

    let weak = window.as_weak();
    let saved = Rc::clone(&saved_output);
    window.on_send_email(move || {
        let Some(window) = weak.upgrade() else { return };
        let binding = saved.borrow();
        let Some(output) = binding.as_ref() else { return };
        if let Err(error) = output.verify_unchanged() {
            window.global::<OutputState>().set_can_email(false);
            window.set_status_text(error.into());
            return;
        }
        let path = output.path().to_owned();
        drop(binding);
        let result = robius_share::ShareSheet::new()
            .set_title("Rechnung per E-Mail senden")
            .set_subject("E-Rechnung")
            .add_file(&path)
            .share();
        match result {
            Ok(()) => window.set_status_text("Datei an die Systemfreigabe übergeben. Bitte E-Mail-Ziel auswählen und selbst versenden.".into()),
            Err(error) => window.set_status_text(format!("Kein E-Mail-Ziel verfügbar ({error}). Datei zum manuellen Anhängen: {}", path.display()).into()),
        }
    });

    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    window.window().on_close_requested(move || {
        let Some(window) = weak.upgrade() else {
            return CloseRequestResponse::HideWindow;
        };
        if window.global::<OutputState>().get_busy() {
            return CloseRequestResponse::KeepWindowShown;
        }
        if let Err(error) = save_current(&window, &store, &selected, &draft) {
            window.set_status_text(
                format!("Entwurf konnte nicht gespeichert werden: {error}").into(),
            );
            return CloseRequestResponse::KeepWindowShown;
        }
        CloseRequestResponse::HideWindow
    });

    for argument in arguments {
        receive_argument(
            &window,
            &workspace,
            &documents,
            &current_id,
            &current_draft,
            &saved_output,
            &argument,
        );
    }
    *documents.borrow_mut() = refresh(&window, &workspace)?;
    let timer = slint::Timer::default();
    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    let list = Rc::clone(&documents);
    let saved = Rc::clone(&saved_output);
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(500),
        move || {
            let Some(window) = weak.upgrade() else { return };
            if let Ok(result) = output_receiver.try_recv() {
                window.global::<OutputState>().set_busy(false);
                match result {
                    Ok(output) => {
                        window.set_status_text(
                            format!("Geprüfte Ausgabe gespeichert: {}", output.path().display())
                                .into(),
                        );
                        *saved.borrow_mut() = Some(output);
                        window.global::<OutputState>().set_can_email(true);
                    }
                    Err(error) => window.set_status_text(
                        format!("Ausgabe wurde nicht gespeichert: {error}").into(),
                    ),
                }
            }
            if !window.global::<OutputState>().get_busy() {
                match store.take_handoffs() {
                    Ok(batches) => {
                        for arguments in batches {
                            let _ = window.show();
                            for argument in arguments {
                                receive_argument(
                                    &window, &store, &list, &selected, &draft, &saved, &argument,
                                );
                            }
                        }
                    }
                    Err(error) => window
                        .set_status_text(format!("Druckübergabe fehlgeschlagen: {error}").into()),
                }
            }
        },
    );
    window.run()?;
    Ok(())
}
