use druckwandel_native::{
    invoice::InvoiceInput,
    learning::ProfileStore,
    recognition::{RecognitionMethod, RecognitionRegistry, RecognitionRequest},
};
use sha2::{Digest, Sha256};
fn fixture() -> (RecognitionRequest, InvoiceInput) {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/source-invoice.pdf");
    let hash = format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()));
    let mut request = RecognitionRequest::from_pdf(path, &hash).unwrap();
    for token in request.pages.iter_mut().flat_map(|p| &mut p.tokens) {
        token.text = token.text.replace("Rechnungsnummer", "Vorgangskennung");
        for (from, to) in [
            ("Bezeichnung", "SpalteA"),
            ("Beschreibung", "SpalteA"),
            ("Menge", "SpalteB"),
            ("Einzelpreis", "SpalteC"),
            ("Gesamt", "SpalteD"),
        ] {
            if token.text == from {
                token.text = to.into();
            }
        }
    }
    let invoice = serde_json::from_str(include_str!("fixtures/export/standard.json")).unwrap();
    (request, invoice)
}
#[test]
fn profiles_read_future_values_from_sources_without_replaying_invoice_constants() {
    let (request, invoice) = fixture();
    let mut store = ProfileStore::default();
    store.create("Lieferant A").unwrap();
    store.learn(&request, &invoice).unwrap();
    assert!(
        store
            .active()
            .memory
            .fields
            .iter()
            .any(|r| r.field == "invoiceNumber")
    );
    assert!(!store.active().memory.tables.is_empty());
    let mut next = request.clone();
    next.document_sha256 = "next-document".into();
    for token in next.pages.iter_mut().flat_map(|p| &mut p.tokens) {
        token.text = token
            .text
            .replace("RE-2026-0001", "RE-2026-0099")
            .replace("20.08.2026", "21.08.2026");
    }
    let mut result = RecognitionRegistry::default()
        .recognize(RecognitionMethod::Schema, &next)
        .unwrap();
    store.apply(&next, &mut result);
    assert_eq!(result.fields["invoiceNumber"].value, "RE-2026-0099");
    assert!(!result.lines.is_empty());
    for field in result.fields.values() {
        assert!(field.source_token_ids.iter().all(|id| {
            next.pages
                .iter()
                .flat_map(|p| &p.tokens)
                .any(|t| &t.id == id)
        }));
    }
    assert!(result.warnings.iter().any(|w| w.contains("Lieferant A")));
    let rule = store
        .active()
        .memory
        .fields
        .iter()
        .find(|r| r.field == "invoiceNumber")
        .unwrap()
        .id
        .clone();
    store.edit_rule(&rule, false).unwrap();
    let mut disabled = RecognitionRegistry::default()
        .recognize(RecognitionMethod::Schema, &next)
        .unwrap();
    store.apply(&next, &mut disabled);
    assert!(!disabled.fields.contains_key("invoiceNumber"));
    store.undo().unwrap();
    let mut other = next.clone();
    for token in other.pages.iter_mut().flat_map(|p| &mut p.tokens) {
        token.text = token
            .text
            .replace("Muster", "Fremd")
            .chars()
            .map(|c| if c.is_ascii_digit() { '9' } else { c })
            .collect();
    }
    let mut result = RecognitionRegistry::default()
        .recognize(RecognitionMethod::Schema, &other)
        .unwrap();
    store.apply(&other, &mut result);
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.contains("Lernprofil Lieferant A"))
    );
    let dir = tempfile::tempdir().unwrap();
    store.save(dir.path()).unwrap();
    let loaded = ProfileStore::load(dir.path()).unwrap();
    assert_eq!(loaded.active().name, "Lieferant A");
    store.clear();
    assert!(store.active().memory.fields.is_empty());
    store.undo().unwrap();
    assert!(!store.active().memory.fields.is_empty());
    store.delete().unwrap();
    assert_eq!(store.active().name, "Standard");
    assert!(store.delete().is_err());
}

#[test]
fn learning_ignores_edge_and_zero_dimension_pages_without_failing_save() {
    let (mut request, invoice) = fixture();
    let page_width = request.pages[0].width;
    for token in &mut request.pages[0].tokens {
        if token.text.contains("RE-2026-0001") {
            token.bounds.x = page_width + 50.0;
        }
    }
    let mut store = ProfileStore::default();
    let _ = store.learn(&request, &invoice);
    let dir = tempfile::tempdir().unwrap();
    store.save(dir.path()).unwrap();

    request.pages[0].width = 0.0;
    let mut zero_store = ProfileStore::default();
    assert!(zero_store.learn(&request, &invoice).is_err());
    zero_store.save(dir.path()).unwrap();
}
