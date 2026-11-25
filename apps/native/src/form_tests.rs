use super::*;
use slint::{
    Model,
    platform::{
        Platform, WindowAdapter,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};

struct Offscreen(Rc<MinimalSoftwareWindow>);
impl Platform for Offscreen {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn export_destinations_protect_originals_and_validate_extensions() {
    let temp = tempfile::tempdir().unwrap();
    let store = Workspace::new(temp.path().join("profile")).unwrap();
    assert_eq!(
        suggested_output_name(" RE:2026/01 ", true),
        "Rechnung-RE-2026-01-ZUGFeRD.pdf"
    );
    let destination = temp.path().join("Ausgabe");
    let expected = destination.with_extension("pdf");
    assert_eq!(
        export_path(destination.clone(), "pdf", &store).unwrap(),
        expected
    );
    std::fs::write(&expected, b"existing file").unwrap();
    assert!(export_path(destination, "pdf", &store).is_err());
    assert_eq!(
        export_path(expected.clone(), "pdf", &store).unwrap(),
        expected
    );
    assert!(export_path(expected, "xml", &store).is_err());
    assert!(export_path(store.root().join("documents/geschuetzt.pdf"), "pdf", &store).is_err());
}

#[test]
fn export_preflight_reports_missing_format_specific_fields() {
    let mut invoice: InvoiceInput =
        serde_json::from_str(include_str!("../tests/fixtures/export/standard.json")).unwrap();
    assert!(desktop_export_errors(&invoice, false).is_empty());
    invoice.buyer.electronic_address = None;
    assert!(desktop_export_errors(&invoice, true).is_empty());
    assert_eq!(
        desktop_export_errors(&invoice, false),
        ["E-Adresse des Empfängers fehlt."]
    );
}

#[test]
fn extended_invoice_form_preserves_export_cases_and_persists_collection_edits() {
    let surface = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Offscreen(surface))).unwrap();
    let ui = MainWindow::new().unwrap();
    ui.global::<FieldRules>()
        .on_is_blank(|value| value.trim().is_empty());
    extended_form::wire_collections(&ui);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/export");
    for name in [
        "standard",
        "adjustments",
        "credit",
        "correction",
        "partial",
        "prepayment",
        "final",
        "reverseCharge",
        "exempt",
        "intraCommunity",
        "zero",
    ] {
        let input: InvoiceInput =
            serde_json::from_slice(&std::fs::read(root.join(format!("{name}.json"))).unwrap())
                .unwrap();
        set_form(&ui, &input);
        let mut edited = input.clone();
        // Simulate switching through every line, committing the previous editor first.
        for index in 0..input.lines.len() {
            edited = collect_form(&ui, edited);
            ui.set_selected_line(index as i32);
            set_line_form(&ui, edited.lines.get(index));
        }
        edited = collect_form(&ui, edited);
        assert_eq!(
            druckwandel_native::cii::generate_cii(input.clone()).unwrap(),
            druckwandel_native::cii::generate_cii(edited.clone()).unwrap(),
            "CII round trip: {name}"
        );
        assert_eq!(
            druckwandel_native::ubl::generate_ubl(input).unwrap(),
            druckwandel_native::ubl::generate_ubl(edited).unwrap(),
            "UBL round trip: {name}"
        );
    }
    let mut input: InvoiceInput =
        serde_json::from_slice(&std::fs::read(root.join("standard.json")).unwrap()).unwrap();
    input.preceding_invoice = Some(druckwandel_native::invoice::PrecedingInvoice {
        invoice_number: "ALT".into(),
        ..Default::default()
    });
    input.lines[1].net_unit_price = "12,345".into();
    set_form(&ui, &input);
    ui.global::<OutputState>().set_can_email(true);
    ui.invoke_remove_reference(0);
    assert!(!ui.global::<OutputState>().get_can_email());
    ui.invoke_add_reference();
    let references = ui.get_references();
    references.set_row_data(
        0,
        ReferenceData {
            number: "NEU".into(),
            date: "2026-07-01".into(),
            paid: "20,00".into(),
        },
    );
    ui.invoke_add_reference();
    let references = ui.get_references();
    references.set_row_data(
        1,
        ReferenceData {
            number: "NEU-2".into(),
            date: "2026-07-02".into(),
            paid: "10.00".into(),
        },
    );
    ui.invoke_add_adjustment(false);
    ui.get_document_adjustments().set_row_data(
        0,
        AdjustmentData {
            amount: "5,00".into(),
            reason: "Rabatt".into(),
            tax_category: "S".into(),
            tax_rate: "19".into(),
            ..Default::default()
        },
    );
    ui.invoke_add_adjustment(true);
    ui.get_line_adjustments().set_row_data(
        0,
        AdjustmentData {
            amount: "2,00".into(),
            reason: "Positionsrabatt".into(),
            tax_category: "S".into(),
            tax_rate: "19".into(),
            ..Default::default()
        },
    );
    ui.set_service_date("2026-08-18".into());
    ui.set_currency("CHF".into());
    ui.set_seller_country("AT".into());
    ui.set_buyer_country(" sg ".into());
    ui.set_line_unit("HUR".into());
    ui.set_line_price("80,50".into());
    ui.set_line_tax_rate("18,5".into());
    let edited = collect_form(&ui, input);
    let workspace = tempfile::tempdir().unwrap();
    let store = Workspace::new(workspace.path().join("data")).unwrap();
    let document = store
        .import_pdf(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/source-invoice.pdf"),
        )
        .unwrap();
    assert!(read_verified_pdf(&document.path, &document.sha256).is_ok());
    assert!(read_verified_pdf(&document.path, "different hash").is_err());
    store.save_invoice(&document.id, &edited).unwrap();
    let saved = store.load_invoice(&document.id).unwrap();
    assert_eq!(saved.currency, "CHF");
    assert_eq!(saved.seller.address.country_code, "AT");
    assert_eq!(saved.buyer.address.country_code, "SG");
    assert_eq!(saved.service_date.as_deref(), Some("2026-08-18"));
    assert_eq!(saved.lines[0].unit_code, "HUR");
    assert_eq!(saved.lines[0].net_unit_price, "80.50");
    assert_eq!(saved.lines[1].net_unit_price, "12.345");
    assert_eq!(saved.lines[0].tax.rate, "18.5");
    assert_eq!(saved.lines[0].allowances.len(), 1);
    assert_eq!(saved.lines[0].allowances[0].tax.rate, "18.5");
    assert_eq!(saved.allowances.len(), 1);
    assert_eq!(saved.preceding_invoices.len(), 2);
    assert!(saved.preceding_invoice.is_none());
    let xml = druckwandel_native::cii::generate_cii(saved.clone()).unwrap();
    assert!(xml.contains("CHF"));
    assert!(xml.contains("NEU-2"));
    set_form(&ui, &saved);
    ui.invoke_remove_reference(1);
    ui.invoke_remove_reference(0);
    ui.invoke_remove_adjustment(true, 0);
    ui.invoke_remove_adjustment(false, 0);
    let cleared = collect_form(&ui, saved);
    assert!(druckwandel_native::invoice::preceding(&cleared).is_empty());
    assert!(cleared.allowances.is_empty());
    assert!(cleared.lines[0].allowances.is_empty());
    set_form(&ui, &cleared);
    ui.invoke_add_reference();
    let incomplete = collect_form(&ui, cleared);
    set_form(&ui, &incomplete);
    let reopened = collect_form(&ui, incomplete);
    assert_eq!(
        reopened.preceding_invoices.len(),
        1,
        "unfinished draft references must remain editable after reopening"
    );
    // The real background pipeline must keep manual edits and persist its source tokens.
    let store = Rc::new(store);
    let selected = Rc::new(RefCell::new(Some(document.id.clone())));
    let mut manual = store.load_invoice(&document.id).unwrap();
    manual.invoice_number = "MANUELL-BEHALTEN".into();
    set_form(&ui, &manual);
    ui.set_selected_name("Rechnung.pdf".into());
    ui.set_selected_path(document.path.to_string_lossy().into_owned().into());
    ui.set_selected_hash(document.sha256.clone().into());
    let draft = Rc::new(RefCell::new(manual));
    let output = Rc::new(RefCell::new(None));
    let preferences = Rc::new(RefCell::new(Settings::default()));
    let _runtime = recognition_ui::wire(&ui, &store, &selected, &draft, &output, &preferences);
    ui.set_learning_name("Prüfprofil".into());
    ui.invoke_create_learning_profile();
    assert_eq!(ui.get_learning_profiles().row_count(), 2);
    ui.set_learning_name("Umbenannt".into());
    ui.invoke_rename_learning_profile();
    assert_eq!(
        druckwandel_native::learning::ProfileStore::load(store.root())
            .unwrap()
            .active()
            .name,
        "Umbenannt"
    );
    ui.invoke_delete_learning_profile();
    assert_eq!(ui.get_learning_profiles().row_count(), 1);
    ui.show().unwrap();
    ui.invoke_start_recognition(false);
    assert!(ui.get_recognition_running());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while ui.get_recognition_running() && std::time::Instant::now() < deadline {
        slint::platform::update_timers_and_animations();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        !ui.get_recognition_running(),
        "background recognition must complete"
    );
    assert_eq!(ui.get_invoice_number().as_str(), "MANUELL-BEHALTEN");
    let source = store
        .load_recognition_source(&document.id)
        .unwrap()
        .unwrap();
    assert_eq!(source.document_sha256, document.sha256);
    assert!(!source.pages[0].tokens.is_empty());
    let recognition = store.load_recognition(&document.id).unwrap().unwrap();
    let mut recognized_invoice = recognition.invoice_draft();
    let totals = druckwandel_native::invoice::calculate(recognized_invoice.clone())
        .unwrap()
        .totals;
    assert!(
        recognized_total_differences(&totals, &recognition)
            .iter()
            .any(|difference| difference.contains("237,04") && difference.contains("237,05"))
    );
    recognized_invoice.lines[1].net_unit_price = "12.345".into();
    let totals = druckwandel_native::invoice::calculate(recognized_invoice)
        .unwrap()
        .totals;
    assert!(recognized_total_differences(&totals, &recognition).is_empty());
    ui.invoke_learn_corrections();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while ui.get_recognition_running() && std::time::Instant::now() < deadline {
        slint::platform::update_timers_and_animations();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!ui.get_recognition_running());
    let count = ui.get_learning_rules().row_count();
    assert!(
        count > 0,
        "learning must use the saved source tokens: {}",
        ui.get_status_text()
    );
    let id = ui.get_learning_rules().row_data(0).unwrap().id;
    ui.invoke_toggle_learning_rule(id.clone());
    assert!(!ui.get_learning_rules().row_data(0).unwrap().enabled);
    ui.invoke_undo_learning_profile();
    assert!(ui.get_learning_rules().row_data(0).unwrap().enabled);
    ui.invoke_remove_learning_rule(id);
    assert_eq!(ui.get_learning_rules().row_count(), count - 1);
    ui.invoke_undo_learning_profile();
    ui.invoke_clear_learning_profile();
    assert_eq!(ui.get_learning_rules().row_count(), 0);
    ui.invoke_undo_learning_profile();
    assert_eq!(ui.get_learning_rules().row_count(), count);
    ui.hide().unwrap();
}
