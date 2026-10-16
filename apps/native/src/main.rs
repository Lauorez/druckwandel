#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use druckwandel_native::extraction::extract_pages;
use druckwandel_native::invoice::{Contact, ElectronicAddress, InvoiceInput};
use druckwandel_native::output::{SavedOutput, save_xrechnung};
use druckwandel_native::print_inbox::{self, PrintJob};
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

fn set_form(window: &MainWindow, invoice: &InvoiceInput) {
    window.set_invoice_number(invoice.invoice_number.clone().into());
    window.set_issue_date(invoice.issue_date.clone().into());
    window.set_due_date(invoice.due_date.clone().unwrap_or_default().into());
    window.set_buyer_reference(invoice.buyer_reference.clone().into());
    window.set_seller_name(invoice.seller.name.clone().into());
    window.set_seller_street(invoice.seller.address.line1.clone().into());
    window.set_seller_postal(invoice.seller.address.postal_code.clone().into());
    window.set_seller_city(invoice.seller.address.city.clone().into());
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
    window.set_buyer_endpoint(
        invoice
            .buyer
            .electronic_address
            .as_ref()
            .map(|x| x.value.clone())
            .unwrap_or_default()
            .into(),
    );
    let line = invoice.lines.first();
    window.set_line_name(line.map(|x| x.name.clone()).unwrap_or_default().into());
    window.set_line_quantity(line.map(|x| x.quantity.clone()).unwrap_or_default().into());
    window.set_line_price(
        line.map(|x| x.net_unit_price.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_line_tax_rate(line.map(|x| x.tax.rate.clone()).unwrap_or_default().into());
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
    invoice.seller.address.country_code = "DE".into();
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
    invoice.buyer.address.country_code = "DE".into();
    invoice.buyer.electronic_address = endpoint(window.get_buyer_endpoint().to_string());
    if let Some(line) = invoice.lines.first_mut() {
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
    let invoice = store.load_invoice(&document.id)?;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = dirs::data_local_dir()
        .ok_or("Kein lokaler Datenordner verfügbar")?
        .join("Druckwandel")
        .join("native");
    let workspace = Rc::new(Workspace::new(root)?);
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
        let Some(path) = rfd::FileDialog::new()
            .add_filter("XRechnung", &["xml"])
            .set_file_name(format!(
                "{}.xml",
                invoice.invoice_number.trim().replace(['/', '\\'], "-")
            ))
            .save_file()
        else {
            return;
        };
        window.set_status_text("XRechnung wird unabhängig geprüft ...".into());
        *saved.borrow_mut() = None;
        window.global::<OutputState>().set_can_email(false);
        let result =
            ValidatorBundle::discover().and_then(|bundle| save_xrechnung(invoice, &path, &bundle));
        match result {
            Ok(output) => {
                window.set_status_text(
                    format!(
                        "Geprüfte XRechnung gespeichert: {}",
                        output.path().display()
                    )
                    .into(),
                );
                *saved.borrow_mut() = Some(output);
                window.global::<OutputState>().set_can_email(true);
            }
            Err(error) => {
                window.set_status_text(format!("XRechnung wurde nicht gespeichert: {error}").into())
            }
        }
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
            .set_subject("XRechnung")
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
        if let Err(error) = save_current(&window, &store, &selected, &draft) {
            window.set_status_text(
                format!("Entwurf konnte nicht gespeichert werden: {error}").into(),
            );
            return CloseRequestResponse::KeepWindowShown;
        }
        CloseRequestResponse::HideWindow
    });

    for arg in std::env::args_os().skip(1) {
        if let Some(uri) = arg
            .to_str()
            .filter(|value| value.starts_with("erechnung-review:"))
        {
            let result = print_inbox::job_from_uri(uri).and_then(|job| {
                receive_print_job(
                    &window,
                    &workspace,
                    &documents,
                    &current_id,
                    &current_draft,
                    &saved_output,
                    &job,
                )
            });
            match result {
                Ok(()) => window.set_status_text("Druckauftrag aus der Übergabe geöffnet.".into()),
                Err(error) => {
                    window.set_status_text(format!("Druckübergabe fehlgeschlagen: {error}").into())
                }
            }
            continue;
        }
        let path = PathBuf::from(arg);
        match workspace.import_pdf(&path) {
            Ok(document) => {
                window.global::<OutputState>().set_can_email(false);
                match open_document(&window, &workspace, &document, &current_id, &current_draft) {
                    Ok(()) => window.set_status_text("PDF aus der Übergabe importiert.".into()),
                    Err(error) => window.set_status_text(format!("PDF importiert. {error}").into()),
                }
            }
            Err(error) => {
                window.set_status_text(format!("Übergabe fehlgeschlagen: {error}").into())
            }
        }
    }
    *documents.borrow_mut() = refresh(&window, &workspace)?;
    window.run()?;
    Ok(())
}
