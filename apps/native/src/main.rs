#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use druckwandel_native::file_io::read_verified_pdf;
use druckwandel_native::invoice::{
    Contact, ElectronicAddress, InvoiceInput, InvoiceLine, InvoiceTotals,
};
use druckwandel_native::output::{SavedOutput, save_xrechnung, save_zugferd};
use druckwandel_native::print_inbox::{self, PrintJob};
use druckwandel_native::recognition::{RecognitionMethod, RecognitionResult, application_registry};
use druckwandel_native::settings::{CompanyProfile, Settings};
use druckwandel_native::validation::ValidatorBundle;
use druckwandel_native::workspace::{Document, Workspace};
use slint::{CloseRequestResponse, ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

slint::include_modules!();
mod extended_form;
#[cfg(test)]
mod form_tests;
mod recognition_ui;
mod update_ui;

fn set_company_form(window: &MainWindow, company: &CompanyProfile) {
    window.set_company_name(company.name.clone().into());
    window.set_company_street(company.street.clone().into());
    window.set_company_postal(company.postal.clone().into());
    window.set_company_city(company.city.clone().into());
    window.set_company_country(company.country.clone().into());
    window.set_company_vat(company.vat.clone().into());
    window.set_company_tax_number(company.tax_number.clone().into());
    window.set_company_endpoint(company.endpoint.clone().into());
    window.set_company_contact_name(company.contact_name.clone().into());
    window.set_company_contact_phone(company.contact_phone.clone().into());
    window.set_company_contact_email(company.contact_email.clone().into());
    window.set_company_iban(company.iban.clone().into());
}

fn collect_company_form(window: &MainWindow) -> CompanyProfile {
    CompanyProfile {
        name: window.get_company_name().to_string().trim().to_owned(),
        street: window.get_company_street().to_string().trim().to_owned(),
        postal: window.get_company_postal().to_string().trim().to_owned(),
        city: window.get_company_city().to_string().trim().to_owned(),
        country: window
            .get_company_country()
            .to_string()
            .trim()
            .to_uppercase(),
        vat: window.get_company_vat().to_string().trim().to_owned(),
        tax_number: window
            .get_company_tax_number()
            .to_string()
            .trim()
            .to_owned(),
        endpoint: window.get_company_endpoint().to_string().trim().to_owned(),
        contact_name: window
            .get_company_contact_name()
            .to_string()
            .trim()
            .to_owned(),
        contact_phone: window
            .get_company_contact_phone()
            .to_string()
            .trim()
            .to_owned(),
        contact_email: window
            .get_company_contact_email()
            .to_string()
            .trim()
            .to_owned(),
        iban: window.get_company_iban().to_string().trim().to_owned(),
    }
}

fn selected_recognition(window: &MainWindow) -> RecognitionMethod {
    match window.get_recognition_method() {
        1 => RecognitionMethod::LocalAi,
        2 => RecognitionMethod::WSystem,
        _ => RecognitionMethod::Schema,
    }
}

fn saved_company(store: &Workspace) -> Result<CompanyProfile, String> {
    Ok(Settings::load(store.root())?.company)
}

fn optional(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn endpoint(value: String, existing: Option<&ElectronicAddress>) -> Option<ElectronicAddress> {
    if let Some(existing) = existing
        && existing.value == value
    {
        return Some(existing.clone());
    }
    optional(value).map(|value| ElectronicAddress {
        scheme_id: if value.contains('@') { "EM" } else { "0204" }.into(),
        value,
    })
}

fn set_line_form(window: &MainWindow, line: Option<&InvoiceLine>) {
    extended_form::set_line(window, line);
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
                        name.chars().take(22).collect::<String>()
                    )
                    .into()
                }
            })
            .collect::<Vec<SharedString>>(),
    )));
}

fn set_form(window: &MainWindow, invoice: &InvoiceInput) {
    extended_form::set_fields(window, invoice);
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
    window.set_seller_tax_number(
        invoice
            .seller
            .tax_registration_id
            .clone()
            .unwrap_or_default()
            .into(),
    );
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
    extended_form::collect_fields(window, &mut invoice);
    invoice.invoice_number = window.get_invoice_number().to_string();
    invoice.issue_date = window.get_issue_date().to_string();
    invoice.due_date = optional(window.get_due_date().to_string());
    invoice.buyer_reference = window.get_buyer_reference().to_string();
    invoice.seller.name = window.get_seller_name().to_string();
    invoice.seller.address.line1 = window.get_seller_street().to_string();
    invoice.seller.address.postal_code = window.get_seller_postal().to_string();
    invoice.seller.address.city = window.get_seller_city().to_string();
    invoice.seller.address.country_code = window
        .get_seller_country()
        .to_string()
        .trim()
        .to_uppercase();
    invoice.seller.vat_id = optional(window.get_seller_vat().to_string());
    invoice.seller.tax_registration_id = optional(window.get_seller_tax_number().to_string());
    invoice.seller.electronic_address = endpoint(
        window.get_seller_endpoint().to_string(),
        invoice.seller.electronic_address.as_ref(),
    );
    let contact = Contact {
        name: optional(window.get_seller_contact_name().to_string()),
        phone: optional(window.get_seller_contact_phone().to_string()),
        email: optional(window.get_seller_contact_email().to_string()),
    };
    invoice.seller.contact =
        (contact.name.is_some() || contact.phone.is_some() || contact.email.is_some())
            .then_some(contact);
    invoice.buyer.name = window.get_buyer_name().to_string();
    invoice.buyer.address.line1 = window.get_buyer_street().to_string();
    invoice.buyer.address.postal_code = window.get_buyer_postal().to_string();
    invoice.buyer.address.city = window.get_buyer_city().to_string();
    invoice.buyer.address.country_code =
        window.get_buyer_country().to_string().trim().to_uppercase();
    invoice.buyer.electronic_address = endpoint(
        window.get_buyer_endpoint().to_string(),
        invoice.buyer.electronic_address.as_ref(),
    );
    if let Some(line) = invoice.lines.get_mut(window.get_selected_line() as usize) {
        extended_form::collect_line(window, line);
    }
    invoice.payment.iban = optional(window.get_payment_iban().to_string());
    extended_form::normalize_decimals(&mut invoice);
    invoice
}

fn open_document(
    window: &MainWindow,
    store: &Workspace,
    document: &Document,
    current_id: &RefCell<Option<String>>,
    current_draft: &RefCell<InvoiceInput>,
) -> Result<(), String> {
    let source_has_xml = {
        let bytes = read_verified_pdf(&document.path, &document.sha256)?;
        druckwandel_native::hybrid_pdf::extract_invoice_xml(&bytes).is_ok()
    };
    let initial = !store.has_invoice(&document.id)?;
    if initial {
        let mut invoice = InvoiceInput::blank();
        saved_company(store)?.fill_missing(&mut invoice);
        store.save_invoice(&document.id, &invoice)?;
    }
    let invoice = store.load_invoice(&document.id)?;
    let source_text = store
        .load_recognition_source(&document.id)?
        .map(|request| recognition_ui::source_text(&request))
        .unwrap_or_else(|| "Erkennung starten, um PDF-Text und Fundstellen zu laden.".into());
    window.set_recognition_text(
        store
            .load_recognition(&document.id)?
            .map(|result| result.review_text())
            .unwrap_or_default()
            .into(),
    );
    set_form(window, &invoice);
    window.set_export_issue("".into());
    window.set_export_review_required(false);
    window.set_recognition_notice("".into());
    *current_id.borrow_mut() = Some(document.id.clone());
    *current_draft.borrow_mut() = invoice;
    show_document(window, document, source_text, source_has_xml);
    if initial {
        window.invoke_start_recognition(true);
    }
    Ok(())
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

fn show_document(
    window: &MainWindow,
    document: &Document,
    source_text: String,
    source_has_xml: bool,
) {
    window.set_selected_document_id(document.id.clone().into());
    window.set_selected_name(document.name.clone().into());
    // Clear hash before changing path so preview fires once with the matching pair.
    window.set_selected_hash("".into());
    window.set_selected_path(document.path.to_string_lossy().into_owned().into());
    window.set_selected_hash(document.sha256.clone().into());
    window.set_source_has_xml(source_has_xml);
    window.set_source_text(source_text.into());
}

fn refresh(window: &MainWindow, workspace: &Workspace) -> Result<Vec<Document>, String> {
    let documents = workspace.list()?;
    let entries = documents
        .iter()
        .map(|document| DraftEntry {
            id: document.id.clone().into(),
            name: document.name.clone().into(),
        })
        .collect::<Vec<_>>();
    window.set_documents(ModelRc::new(VecModel::from(entries)));
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

fn workspace_root() -> Result<PathBuf, &'static str> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("DRUCKWANDEL_DEV_WORKSPACE") {
        let path = PathBuf::from(path);
        return path
            .is_absolute()
            .then_some(path)
            .ok_or("Testdatenordner muss absolut sein.");
    }
    Ok(dirs::data_local_dir()
        .ok_or("Kein lokaler Datenordner verfügbar")?
        .join("Druckwandel")
        .join("native"))
}

fn suggested_output_name(invoice_number: &str, zugferd: bool) -> String {
    let stem: String = invoice_number
        .trim()
        .replace(
            |ch: char| ch.is_control() || "/\\:\"*?<>|".contains(ch),
            "-",
        )
        .chars()
        .take(100)
        .collect();
    let stem = stem.trim_matches([' ', '.']);
    let format = if zugferd {
        "ZUGFeRD.pdf"
    } else {
        "XRechnung.xml"
    };
    format!("Rechnung-{stem}-{format}")
}

fn export_path(path: PathBuf, extension: &str, store: &Workspace) -> Result<PathBuf, String> {
    let path = match path.extension() {
        None => {
            let with_extension = path.with_extension(extension);
            if with_extension.exists() {
                return Err(format!(
                    "Die Datei existiert bereits. Bitte den vollständigen Dateinamen mit .{extension} auswählen, um sie zu ersetzen."
                ));
            }
            with_extension
        }
        Some(value) if value.eq_ignore_ascii_case(extension) => path,
        _ => {
            return Err(format!(
                "Die Ausgabe muss als .{extension} gespeichert werden."
            ));
        }
    };
    let originals = store
        .root()
        .join("documents")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let parent = path.parent().ok_or("Ungültiger Speicherort.")?;
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("Zielordner nicht erreichbar: {e}"))?;
    if parent.starts_with(&originals)
        || path
            .canonicalize()
            .is_ok_and(|target| target.starts_with(&originals))
    {
        return Err(
            "Bitte einen Speicherort außerhalb des internen Ordners für Original-PDFs wählen."
                .into(),
        );
    }
    Ok(path)
}

fn show_export_issue(window: &MainWindow, message: String) {
    window.set_export_review_required(false);
    window.set_status_text("Ausgabe wurde nicht gespeichert.".into());
    window.set_export_issue(message.into());
}

fn desktop_export_errors(invoice: &InvoiceInput, zugferd: bool) -> Vec<String> {
    use druckwandel_native::invoice::{validate, validate_en16931};
    let mut errors = if zugferd {
        validate_en16931(invoice)
    } else {
        validate(invoice)
    };
    let contact = invoice.seller.contact.as_ref();
    if !zugferd {
        for (value, label) in [
            (
                invoice
                    .seller
                    .electronic_address
                    .as_ref()
                    .map(|address| address.value.as_str()),
                "E-Adresse des Absenders",
            ),
            (
                invoice
                    .buyer
                    .electronic_address
                    .as_ref()
                    .map(|address| address.value.as_str()),
                "E-Adresse des Empfängers",
            ),
            (
                contact.and_then(|contact| contact.name.as_deref()),
                "Ansprechpartner des Absenders",
            ),
            (
                contact.and_then(|contact| contact.phone.as_deref()),
                "Telefon des Absenders",
            ),
            (
                contact.and_then(|contact| contact.email.as_deref()),
                "E-Mail-Adresse des Absenders",
            ),
        ] {
            if value.is_none_or(|value| value.trim().is_empty()) {
                errors.push(format!("{label} fehlt."));
            }
        }
    }
    if invoice.payment.means_code.trim().is_empty() {
        errors.push("Zahlungsart fehlt.".into());
    }
    if matches!(invoice.payment.means_code.trim(), "30" | "58" | "59")
        && invoice
            .payment
            .iban
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("IBAN für die gewählte Zahlungsart fehlt.".into());
    }
    errors
}

fn recognized_total_differences(
    totals: &InvoiceTotals,
    recognition: &RecognitionResult,
) -> Vec<String> {
    [
        ("lineNet", "Netto", totals.tax_exclusive.as_str()),
        ("taxInclusive", "Brutto", totals.tax_inclusive.as_str()),
    ]
    .into_iter()
    .filter_map(|(key, label, calculated)| {
        let source = recognition.fields.get(key)?;
        let expected = source.value.parse::<rust_decimal::Decimal>().ok()?;
        let actual = calculated.parse::<rust_decimal::Decimal>().ok()?;
        (expected != actual).then(|| {
            format!(
                "{label}: Beleg {} · Formular {}",
                source.value.replace('.', ","),
                calculated.replace('.', ","),
            )
        })
    })
    .collect()
}

fn acquire_instance_lock(path: &std::path::Path) -> Result<Option<std::fs::File>, std::io::Error> {
    let instance_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    match instance_file.try_lock() {
        Ok(()) => Ok(Some(instance_file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let workspace = Rc::new(Workspace::new(root.clone())?);
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some(_instance_file) = acquire_instance_lock(&root.join("app.lock"))? else {
        workspace.enqueue_handoff(&arguments)?;
        return Ok(());
    };
    let (output_sender, output_receiver) =
        std::sync::mpsc::channel::<(String, Result<SavedOutput, String>)>();
    let settings_result = Settings::load(&root);
    let settings_error = settings_result.as_ref().err().cloned();
    let settings = Rc::new(RefCell::new(settings_result.unwrap_or_default()));
    let appearance = settings.borrow().appearance;
    let initial_theme = match appearance {
        1 => Some(slint::winit_030::winit::window::Theme::Light),
        2 => Some(slint::winit_030::winit::window::Theme::Dark),
        _ => None,
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(move |attributes| {
            let attributes = attributes.with_transparent(false).with_theme(initial_theme);
            #[cfg(target_os = "macos")]
            {
                use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
                attributes
                    .with_titlebar_transparent(false)
                    .with_fullsize_content_view(false)
            }
            #[cfg(not(target_os = "macos"))]
            {
                attributes
            }
        })
        .select()?;
    let window = MainWindow::new()?;
    window.set_app_version(env!("CARGO_PKG_VERSION").into());
    extended_form::wire_collections(&window);
    window
        .global::<FieldRules>()
        .on_is_blank(|value| value.trim().is_empty());
    window.set_appearance(appearance);
    window.set_recognition_method(settings.borrow().recognition_method);
    set_company_form(&window, &settings.borrow().company);
    if let Some(error) = settings_error {
        window.set_settings_status(error.into());
    }
    let preferences = Rc::clone(&settings);
    let settings_root = root.clone();
    let weak = window.as_weak();
    window.on_appearance_changed(move || {
        let Some(window) = weak.upgrade() else { return };
        use slint::winit_030::{WinitWindowAccessor, winit::window::Theme};
        let theme = match window.get_appearance() {
            1 => Some(Theme::Light),
            2 => Some(Theme::Dark),
            _ => None,
        };
        window
            .window()
            .with_winit_window(|native| native.set_theme(theme));
        preferences.borrow_mut().appearance = window.get_appearance();
        if let Err(error) = preferences.borrow().save(&settings_root) {
            window.set_settings_status(
                format!("Einstellungen konnten nicht gespeichert werden: {error}").into(),
            );
        }
    });
    let weak = window.as_weak();
    let preferences = Rc::clone(&settings);
    let settings_root = root.clone();
    window.on_recognition_method_changed(move || {
        let Some(window) = weak.upgrade() else { return };
        preferences.borrow_mut().recognition_method = window.get_recognition_method();
        if let Err(error) = preferences.borrow().save(&settings_root) {
            window.set_settings_status(
                format!("Einstellungen konnten nicht gespeichert werden: {error}").into(),
            );
        }
    });
    let weak = window.as_weak();
    let preferences = Rc::clone(&settings);
    let settings_root = root.clone();
    window.on_save_settings(move || {
        let Some(window) = weak.upgrade() else { return };
        let mut updated = preferences.borrow().clone();
        updated.company = collect_company_form(&window);
        match updated.save(&settings_root) {
            Ok(()) => {
                *preferences.borrow_mut() = updated;
                window.set_settings_status("Unternehmensdaten gespeichert. Neue Entwürfe verwenden die Vorlage für fehlende Angaben.".into());
            }
            Err(error) => window.set_settings_status(format!("Speichern fehlgeschlagen: {error}").into()),
        }
    });
    let preview = Rc::new(druckwandel_native::preview::PreviewWorker::new()?);
    let worker = Rc::clone(&preview);
    let weak = window.as_weak();
    let preview_source = RefCell::new(String::new());
    window.on_request_preview(move || {
        let Some(window) = weak.upgrade() else { return };
        let path = window.get_selected_path();
        let hash = window.get_selected_hash();
        if path.is_empty() || hash.is_empty() {
            return;
        }
        if preview_source.borrow().as_str() != path.as_str() {
            window.set_preview_page_count(0);
            *preview_source.borrow_mut() = path.to_string();
        }
        window.set_preview_loading(true);
        window.set_preview_error("".into());
        window.set_preview_image(slint::Image::default());
        let width = match window.get_preview_zoom() {
            1 => 1250,
            2 => 1500,
            3 => 2000,
            _ => 1000,
        };
        worker.request(
            PathBuf::from(path.as_str()),
            hash.to_string(),
            window.get_preview_page().max(0) as usize,
            width,
        );
    });
    let preview_timer = slint::Timer::default();
    let weak = window.as_weak();
    preview_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            let Some(window) = weak.upgrade() else { return };
            if let Some(result) = preview.take_response() {
                window.set_preview_loading(false);
                match result {
                    Ok(frame) => {
                        window.set_preview_page_count(frame.page_count as i32);
                        window.set_preview_aspect(frame.height as f32 / frame.width as f32);
                        let buffer =
                            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                &frame.pixels,
                                frame.width,
                                frame.height,
                            );
                        window.set_preview_image(slint::Image::from_rgba8_premultiplied(buffer));
                    }
                    Err(error) => {
                        window.set_preview_error(error.into());
                        window.set_preview_page_count(0);
                    }
                }
            }
        },
    );
    let documents = Rc::new(RefCell::new(refresh(&window, &workspace)?));
    let current_id = Rc::new(RefCell::new(None::<String>));
    let current_draft = Rc::new(RefCell::new(InvoiceInput::blank()));
    let saved_output = Rc::new(RefCell::new(None::<SavedOutput>));
    let _recognition_ui = recognition_ui::wire(
        &window,
        &workspace,
        &current_id,
        &current_draft,
        &saved_output,
        &settings,
    );
    let _update_ui = update_ui::wire(&window, &workspace, &current_id, &current_draft, &settings);
    window.set_status_text("PDF importieren oder gespeicherten Entwurf auswählen.".into());

    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    let saved = Rc::clone(&saved_output);
    let preferences = Rc::clone(&settings);
    window.on_apply_company(move || {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
        let Some(id) = selected.borrow().clone() else {
            return;
        };
        let mut invoice = collect_form(&window, draft.borrow().clone());
        let company = &preferences.borrow().company;
        if company.name.trim().is_empty() {
            window.set_settings_status(
                "Bitte zuerst Unternehmensdaten mit einem Namen speichern.".into(),
            );
            return;
        }
        invoice.seller = company.party();
        invoice.payment.iban = optional(company.iban.clone());
        match store.save_invoice(&id, &invoice) {
            Ok(()) => {
                set_form(&window, &invoice);
                *draft.borrow_mut() = invoice;
                *saved.borrow_mut() = None;
                window.global::<OutputState>().set_can_email(false);
                window.set_settings_status(
                    "Gespeicherte Unternehmensdaten in die aktuelle Rechnung übernommen.".into(),
                );
            }
            Err(error) => window.set_settings_status(error.into()),
        }
    });

    let weak = window.as_weak();
    window.on_open_source(move || {
        let Some(window) = weak.upgrade() else { return };
        let raw = window.get_selected_path();
        if raw.trim().is_empty() {
            return;
        }
        let path = PathBuf::from(raw.as_str());
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(&path).spawn();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("explorer.exe")
            .arg(&path)
            .spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let result = std::process::Command::new("xdg-open").arg(&path).spawn();
        if let Err(error) = result {
            window.set_status_text(
                format!("Original-PDF konnte nicht geöffnet werden: {error}").into(),
            );
        }
    });

    let weak = window.as_weak();
    let store = Rc::clone(&workspace);
    let list = Rc::clone(&documents);
    let selected = Rc::clone(&current_id);
    let draft = Rc::clone(&current_draft);
    let saved = Rc::clone(&saved_output);
    window.on_import_pdf(move || {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
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
        if window.global::<OutputState>().get_busy() {
            return;
        }
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
        if window.global::<OutputState>().get_busy() {
            return;
        }
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(document) = selection_list.borrow().get(index).cloned() else {
            return;
        };
        if selected.borrow().as_ref() == Some(&document.id) {
            return;
        }
        if let Err(error) = save_current(&window, &store, &selected, &draft) {
            window.set_status_text(
                format!("Entwurf konnte nicht gespeichert werden: {error}").into(),
            );
            return;
        }
        *saved.borrow_mut() = None;
        window.global::<OutputState>().set_can_email(false);
        match open_document(&window, &store, &document, &selected, &draft) {
            Ok(()) => window.set_status_text(format!("{} geöffnet.", document.name).into()),
            Err(error) => window.set_status_text(error.into()),
        }
    });

    let weak = window.as_weak();
    let draft = Rc::clone(&current_draft);
    window.on_select_line(move |index| {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
        let Ok(line_index) = usize::try_from(index) else {
            return;
        };
        let invoice = collect_form(&window, draft.borrow().clone());
        let Some(line) = invoice.lines.get(line_index) else {
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
        if window.global::<OutputState>().get_busy() {
            return;
        }
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
        if window.global::<OutputState>().get_busy() {
            return;
        }
        let mut invoice = collect_form(&window, draft.borrow().clone());
        if invoice.lines.len() <= 1 {
            return;
        }
        let Ok(selected_line) = usize::try_from(window.get_selected_line()) else {
            return;
        };
        let index = selected_line.min(invoice.lines.len() - 1);
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
    window.on_save_draft(move || {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
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
    window.on_save_output(move |reviewed| {
        let Some(window) = weak.upgrade() else { return };
        if window.global::<OutputState>().get_busy() {
            return;
        }
        let reviewed = reviewed && window.get_export_review_required();
        let Some(id) = selected.borrow().clone() else {
            return;
        };
        window.set_export_issue("".into());
        window.set_export_review_required(false);
        let invoice = collect_form(&window, draft.borrow().clone());
        if let Err(error) = store.save_invoice(&id, &invoice) {
            window.set_status_text(
                format!("Entwurf konnte nicht gespeichert werden: {error}").into(),
            );
            return;
        }
        *draft.borrow_mut() = invoice.clone();
        let zugferd = window.get_output_format() == 0;
        let errors = desktop_export_errors(&invoice, zugferd);
        if !errors.is_empty() {
            show_export_issue(
                &window,
                format!("Bitte die Rechnungsdaten prüfen und ergänzen.\n\n• {}", errors.join("\n• ")),
            );
            return;
        }
        if !reviewed {
            let differences = (|| {
                let calculated = druckwandel_native::invoice::calculate(invoice.clone())?;
                Ok::<_, String>(store
                    .load_recognition(&id)?
                    .map(|recognition| recognized_total_differences(&calculated.totals, &recognition))
                    .unwrap_or_default())
            })();
            match differences {
                Ok(differences) if !differences.is_empty() => {
                    show_export_issue(
                        &window,
                        format!(
                            "Die berechneten Summen weichen von den erkannten Belegwerten ab. Bitte Einzelpreise, Mengen und Steuern mit der PDF vergleichen.\n\n{}",
                            differences.join("\n\n")
                        ),
                    );
                    window.set_export_review_required(true);
                    return;
                }
                Err(error) => {
                    show_export_issue(&window, error);
                    return;
                }
                Ok(_) => {}
            }
        }
        let extension = if zugferd { "pdf" } else { "xml" };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(if zugferd { "ZUGFeRD" } else { "XRechnung" }, &[extension])
            .set_file_name(suggested_output_name(&invoice.invoice_number, zugferd))
            .save_file()
        else {
            return;
        };
        let path = match export_path(path, extension, &store) {
            Ok(path) => path,
            Err(error) => {
                show_export_issue(&window, error);
                return;
            }
        };
        let source = PathBuf::from(window.get_selected_path().as_str());
        let source_hash = window.get_selected_hash().to_string();
        if let Err(error) = druckwandel_native::output::ensure_distinct_paths(&source, &path) {
            show_export_issue(&window, error);
            return;
        }
        window.set_status_text("Ausgabe wird erzeugt und unabhängig geprüft ...".into());
        *saved.borrow_mut() = None;
        window.global::<OutputState>().set_can_email(false);
        window.global::<OutputState>().set_busy(true);
        let sender = output_sender.clone();
        let exported_id = id.clone();
        std::thread::spawn(move || {
            let result = ValidatorBundle::discover().and_then(|bundle| {
                // Recheck at export time, including XML-only exports. Recognition
                // and preview checks do not cover subsequent external edits.
                let bytes = read_verified_pdf(&source, &source_hash)?;
                if zugferd {
                    save_zugferd(invoice, &bytes, &path, &bundle).map_err(|error| {
                        format!("{error}\n\nAlternativ kann die Rechnung als XRechnung (XML) ausgegeben werden.")
                    })
                } else {
                    save_xrechnung(invoice, &path, &bundle)
                }
            });
            let _ = sender.send((exported_id, result));
        });
    });

    let weak = window.as_weak();
    let saved = Rc::clone(&saved_output);
    window.on_send_email(move || {
        let Some(window) = weak.upgrade() else { return };
        if !window.global::<OutputState>().get_can_email()
            || window.global::<OutputState>().get_busy()
        {
            return;
        }
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
            if let Ok((exported_id, result)) = output_receiver.try_recv() {
                window.global::<OutputState>().set_busy(false);
                if selected.borrow().as_deref() == Some(exported_id.as_str()) {
                    match result {
                        Ok(output) => {
                            window.set_status_text(
                                format!(
                                    "Geprüfte Ausgabe gespeichert: {}",
                                    output.path().display()
                                )
                                .into(),
                            );
                            *saved.borrow_mut() = Some(output);
                            window.global::<OutputState>().set_can_email(true);
                        }
                        Err(error) => show_export_issue(&window, error),
                    }
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
    window.show()?;
    // Wait until winit has actually created the native window and finished its initial
    // ShownFirstTime resize before maximizing. Setting maximized earlier (via attributes
    // or a 0ms timer in StartCause::Init) causes Slint's ShownFirstTime resize to run on
    // top of a WS_MAXIMIZE HWND on Windows, pinning it to the top-left corner at the
    // preferred size and shrinking it when dragged.
    {
        use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
        let maximized_once = std::rc::Rc::new(std::cell::Cell::new(false));
        let done_event = std::rc::Rc::clone(&maximized_once);
        let weak_event = window.as_weak();
        window.window().on_winit_window_event(move |_win, event| {
            if !done_event.get()
                && let WindowEvent::Resized(size) = event
                && size.width > 0
                && size.height > 0
            {
                done_event.set(true);
                let weak = weak_event.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        window.window().set_maximized(true);
                    }
                });
            }
            EventResult::Propagate
        });
        let done_fallback = maximized_once;
        let weak_fallback = window.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(150), move || {
            if !done_fallback.get()
                && let Some(window) = weak_fallback.upgrade()
                && window.window().has_winit_window()
            {
                done_fallback.set(true);
                window.window().set_maximized(true);
            }
        });
    }
    window.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unedited_form_roundtrip_preserves_invoice_data() {
        struct Offscreen;
        impl slint::platform::Platform for Offscreen {
            fn create_window_adapter(
                &self,
            ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
                Ok(
                    slint::platform::software_renderer::MinimalSoftwareWindow::new(
                        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
                    ),
                )
            }
        }
        slint::platform::set_platform(Box::new(Offscreen)).unwrap();
        let window = MainWindow::new().unwrap();
        let mut input: InvoiceInput =
            serde_json::from_str(include_str!("../tests/fixtures/export/standard.json")).unwrap();
        input.seller.contact = None;
        input.buyer.electronic_address = Some(ElectronicAddress {
            value: "1234567890123".into(),
            scheme_id: "0088".into(),
        });
        set_form(&window, &input);
        let output = collect_form(&window, input.clone());
        assert_eq!(
            serde_json::to_value(output).unwrap(),
            serde_json::to_value(input).unwrap()
        );
    }

    #[test]
    fn instance_lock_detects_contention_on_second_handle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.lock");
        let first = acquire_instance_lock(&path).unwrap();
        assert!(first.is_some());
        assert!(acquire_instance_lock(&path).unwrap().is_none());
        drop(first);
        assert!(acquire_instance_lock(&path).unwrap().is_some());
    }

    #[test]
    fn unchanged_endpoint_preserves_its_scheme() {
        let existing = ElectronicAddress {
            value: "1234567890123".into(),
            scheme_id: "0088".into(),
        };
        assert_eq!(
            endpoint(existing.value.clone(), Some(&existing))
                .unwrap()
                .scheme_id,
            "0088"
        );
        assert_eq!(
            endpoint("mail@example.org".into(), Some(&existing))
                .unwrap()
                .scheme_id,
            "EM"
        );
        assert!(endpoint("".into(), Some(&existing)).is_none());
    }
}
