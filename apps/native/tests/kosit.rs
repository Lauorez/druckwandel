use druckwandel_native::invoice::InvoiceInput;
use druckwandel_native::output::save_xrechnung;
use druckwandel_native::ubl::generate_ubl;
use druckwandel_native::validation::ValidatorBundle;

#[test]
#[ignore = "requires the local validator bundle; run with --ignored during development"]
fn standard_invoice_passes_kosit() {
    let input: InvoiceInput =
        serde_json::from_str(include_str!("fixtures/standard-invoice.json")).unwrap();
    let xml = generate_ubl(input).unwrap();
    let validator = ValidatorBundle::discover().unwrap();
    validator.validate_xrechnung(xml.as_bytes()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rechnung.xml");
    let input: InvoiceInput =
        serde_json::from_str(include_str!("fixtures/standard-invoice.json")).unwrap();
    let saved = save_xrechnung(input, &path, &validator).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), xml);
    saved.verify_unchanged().unwrap();
    std::fs::write(&path, b"changed").unwrap();
    assert!(saved.verify_unchanged().is_err());
}
