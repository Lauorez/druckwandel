use druckwandel_native::{extraction::read_document, recognition::*};
fn fixture() -> RecognitionRequest {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/source-invoice.pdf");
    RecognitionRequest {
        schema_version: CONTRACT_VERSION,
        document_sha256: "fixture".into(),
        source_pdf: path.clone(),
        warnings: vec![],
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

#[test]
fn rerunning_recognition_preserves_partially_edited_unnamed_lines() {
    let result = RecognitionRegistry::default()
        .recognize(RecognitionMethod::Schema, &fixture())
        .unwrap();
    for edit in ["price", "quantity", "tax", "description"] {
        let mut invoice = druckwandel_native::invoice::InvoiceInput::blank();
        match edit {
            "price" => invoice.lines[0].net_unit_price = "123.45".into(),
            "quantity" => invoice.lines[0].quantity = "5".into(),
            "tax" => invoice.lines[0].tax.rate = "7".into(),
            _ => invoice.lines[0].description = Some("Manuelle Notiz".into()),
        }
        let existing = invoice.lines.clone();
        fill_missing(&mut invoice, &result);
        assert_eq!(invoice.lines, existing, "must preserve {edit}");
    }
    let mut blank = druckwandel_native::invoice::InvoiceInput::blank();
    fill_missing(&mut blank, &result);
    assert_eq!(blank.lines, result.invoice_draft().lines);
}

fn synthetic(lines: &[Vec<(&str, f64)>]) -> RecognitionRequest {
    use druckwandel_native::extraction::{BoundingBox, DocumentPage, SourceToken};
    RecognitionRequest {
        schema_version: CONTRACT_VERSION,
        document_sha256: "synthetic".into(),
        source_pdf: "synthetic.pdf".into(),
        warnings: vec![],
        pages: vec![DocumentPage {
            page: 1,
            width: 600.0,
            height: 800.0,
            tokens: lines
                .iter()
                .enumerate()
                .flat_map(|(row, words)| {
                    words
                        .iter()
                        .enumerate()
                        .map(move |(column, (text, x))| SourceToken {
                            id: format!("r{row}-c{column}"),
                            page: 1,
                            text: (*text).into(),
                            bounds: BoundingBox {
                                x: *x,
                                y: row as f64 * 20.0,
                                width: 50.0,
                                height: 10.0,
                            },
                        })
                })
                .collect(),
        }],
    }
}

#[test]
fn dates_on_the_same_line_follow_their_own_labels() {
    let request = synthetic(&[vec![(
        "Rechnungsdatum: 01.10.2026 Fällig am: 15.10.2026",
        0.0,
    )]]);
    let result = RecognitionRegistry::default()
        .recognize(RecognitionMethod::Schema, &request)
        .unwrap();
    assert_eq!(result.fields["issueDate"].value, "2026-10-01");
    assert_eq!(result.fields["dueDate"].value, "2026-10-15");
}

#[test]
fn unknown_or_ambiguous_tax_rates_require_manual_entry() {
    let table = vec![
        vec![
            ("Beschreibung", 0.0),
            ("Menge", 200.0),
            ("Preis", 300.0),
            ("Gesamt", 400.0),
        ],
        vec![
            ("Service", 0.0),
            ("1", 200.0),
            ("100,00", 300.0),
            ("100,00", 400.0),
        ],
    ];
    for summaries in [
        vec![],
        vec![vec![("MwSt 7 %", 0.0)], vec![("MwSt 19 %", 0.0)]],
    ] {
        let mut lines = table.clone();
        lines.extend(summaries);
        let result = RecognitionRegistry::default()
            .recognize(RecognitionMethod::Schema, &synthetic(&lines))
            .unwrap();
        assert_eq!(result.lines.len(), 1);
        assert!(result.lines[0].line.tax.rate.is_empty());
        assert!(result.lines[0].confidence <= 0.6);
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("Steuersatz"))
        );
    }
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
