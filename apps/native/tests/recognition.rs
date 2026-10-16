use druckwandel_native::{extraction::read_document, recognition::*};
fn fixture() -> RecognitionRequest {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/source-invoice.pdf");
    RecognitionRequest {
        schema_version: CONTRACT_VERSION,
        document_sha256: "fixture".into(),
        source_pdf: path.clone(),
        pages: read_document(&path).unwrap(),
    }
}
#[test]
fn recognizes_pdf_fields_and_positions_with_real_source_tokens() {
    let request = fixture();
    let result = RecognitionRegistry::default()
        .recognize(RecognitionMethod::Schema, &request)
        .unwrap();
    assert_eq!(result.fields["invoiceNumber"].value, "RE-2026-0001");
    assert_eq!(result.fields["issueDate"].value, "2026-08-20");
    assert_eq!(result.fields["iban"].value, "DE02120300000000202051");
    assert!(!result.lines.is_empty(), "{:?}", result.warnings);
    assert!(
        result
            .fields
            .values()
            .all(|field| !field.source_token_ids.is_empty())
    );
    let mut invoice = result.invoice_draft();
    invoice.invoice_number = "MANUELL-KORRIGIERT".into();
    fill_missing(&mut invoice, &result);
    assert_eq!(invoice.invoice_number, "MANUELL-KORRIGIERT");
}
struct WrongDocument;
impl RecognitionProvider for WrongDocument {
    fn method(&self) -> RecognitionMethod {
        RecognitionMethod::LocalAi
    }
    fn recognize(&self, request: &RecognitionRequest) -> Result<RecognitionResult, String> {
        let mut result = SchemaRecognizer.recognize(request)?;
        result.method = RecognitionMethod::LocalAi;
        result.document_sha256 = "other-document".into();
        Ok(result)
    }
}
#[test]
fn future_adapter_must_return_results_for_the_requested_document() {
    let mut registry = RecognitionRegistry::default();
    registry.register(Box::new(WrongDocument)).unwrap();
    assert!(
        registry
            .recognize(RecognitionMethod::LocalAi, &fixture())
            .unwrap_err()
            .contains("nicht zu diesem Dokument")
    );
}
