use druckwandel_native::{cii::generate_cii, invoice::InvoiceInput};

// Compare semantic XML content with the frozen version's reference exports.
fn facts(xml: &str) -> Vec<String> {
    let doc = roxmltree::Document::parse(xml).unwrap();
    doc.descendants()
        .filter(|node| node.is_element())
        .map(|node| {
            let mut attrs: Vec<_> = node
                .attributes()
                .map(|a| format!("{}={}", a.name(), a.value()))
                .collect();
            attrs.sort();
            format!(
                "{}:{}:{:?}:{}",
                node.tag_name().namespace().unwrap_or_default(),
                node.tag_name().name(),
                attrs,
                node.children()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .collect::<String>()
            )
        })
        .collect()
}
#[test]
fn cii_matches_every_previously_supported_export_case() {
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
        let expected = std::fs::read_to_string(root.join(format!("{name}.cii.xml"))).unwrap();
        assert_eq!(
            facts(&generate_cii(input).unwrap()),
            facts(&expected),
            "case: {name}"
        );
    }
}

#[test]
fn zugferd_allows_b2b_invoices_without_a_government_buyer_reference() {
    let mut input: InvoiceInput =
        serde_json::from_str(include_str!("fixtures/export/standard.json")).unwrap();
    input.buyer_reference.clear();
    assert!(
        !generate_cii(input.clone())
            .unwrap()
            .contains("<ram:BuyerReference>")
    );
    assert!(druckwandel_native::ubl::generate_ubl(input).is_err());
}
#[test]
fn oversized_arithmetic_returns_an_error_instead_of_panicking() {
    let mut input = InvoiceInput::blank();
    input.lines[0].quantity = rust_decimal::Decimal::MAX.to_string();
    input.lines[0].net_unit_price = "2".into();
    assert!(druckwandel_native::invoice::calculate(input).is_err());
}

#[test]
fn form_details_reject_incomplete_references_units_and_inconsistent_percentages() {
    use druckwandel_native::invoice::{AllowanceCharge, PrecedingInvoice, validate_en16931};
    let mut input: InvoiceInput =
        serde_json::from_str(include_str!("fixtures/export/standard.json")).unwrap();
    input.lines[0].unit_code.clear();
    assert!(!validate_en16931(&input).is_empty());
    input.lines[0].unit_code = "HUR".into();
    input.final_invoice = true;
    assert!(!validate_en16931(&input).is_empty());
    input.preceding_invoices.push(PrecedingInvoice {
        invoice_number: "ALT".into(),
        issue_date: Some("2026-02-30".into()),
        paid_amount: Some("-1".into()),
    });
    assert!(!validate_en16931(&input).is_empty());
    input.preceding_invoices[0].issue_date = Some("2026-02-28".into());
    input.preceding_invoices[0].paid_amount = Some("20.00".into());
    let mut adjustment = AllowanceCharge {
        charge: false,
        amount: "10.00".into(),
        reason: Some("Rabatt".into()),
        reason_code: None,
        percent: Some("10".into()),
        base_amount: Some("100.00".into()),
        tax: input.lines[0].tax.clone(),
    };
    input.allowances.push(adjustment.clone());
    assert!(validate_en16931(&input).is_empty());
    adjustment.base_amount = Some("50.00".into());
    input.allowances[0] = adjustment;
    assert!(!validate_en16931(&input).is_empty());
    input.allowances[0].amount = "5.00".into();
    assert!(validate_en16931(&input).is_empty());
    input.allowances[0].reason = None;
    assert!(!validate_en16931(&input).is_empty());
}
