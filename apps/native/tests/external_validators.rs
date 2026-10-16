use druckwandel_native::{
    cii::generate_cii, hybrid_pdf::extract_invoice_xml, invoice::InvoiceInput,
    output::save_zugferd, ubl::generate_ubl, validation::ValidatorBundle,
};
#[test]
#[ignore = "requires the platform-specific offline validator bundle"]
fn all_supported_invoice_cases_pass_independent_xml_validators() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/export");
    let validators = ValidatorBundle::discover().unwrap();
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
        validators
            .validate_xrechnung(generate_ubl(input.clone()).unwrap().as_bytes())
            .unwrap_or_else(|error| panic!("UBL {name}: {error}"));
        validators
            .validate_cii(generate_cii(input).unwrap().as_bytes())
            .unwrap_or_else(|error| panic!("CII {name}: {error}"));
    }
}
#[test]
#[ignore = "requires the platform-specific offline validator bundle"]
fn saved_zugferd_passes_mustang_verapdf_and_attachment_comparison() {
    let input: InvoiceInput =
        serde_json::from_str(include_str!("fixtures/export/standard.json")).unwrap();
    let source = include_bytes!("fixtures/source-invoice.pdf");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invoice.pdf");
    let validators = ValidatorBundle::discover().unwrap();
    let saved = save_zugferd(input.clone(), source, &path, &validators).unwrap();
    assert_eq!(
        extract_invoice_xml(&std::fs::read(&path).unwrap()).unwrap(),
        generate_cii(input).unwrap().as_bytes()
    );
    saved.verify_unchanged().unwrap();
    std::fs::write(&path, b"modified").unwrap();
    assert!(saved.verify_unchanged().is_err());
}
