use druckwandel_native::{
    cii::generate_cii,
    invoice::{self, InvoiceInput},
    ubl::generate_ubl,
};

fn fixture() -> InvoiceInput {
    serde_json::from_str(include_str!("fixtures/export/standard.json")).unwrap()
}

#[test]
fn preserves_tax_registration_payment_reference_and_delivery_address() {
    let mut input = fixture();
    input.seller.vat_id = None;
    input.seller.tax_registration_id = Some("12/345/67890".into());
    input.payment.payment_reference = Some("Referenz & 123".into());
    let mut delivery = input.buyer.address.clone();
    delivery.line2 = Some("Hinterhaus & Lager".into());
    input.delivery_address = Some(delivery);
    for (xml, reference_tag, address_tag) in [
        (
            generate_ubl(input.clone()).unwrap(),
            "PaymentID",
            "AdditionalStreetName",
        ),
        (generate_cii(input).unwrap(), "PaymentReference", "LineTwo"),
    ] {
        let document = roxmltree::Document::parse(&xml).unwrap();
        assert!(document.descendants().any(|node| node.is_element()
            && node.tag_name().name() == reference_tag
            && node.text() == Some("Referenz & 123")));
        assert!(document.descendants().any(|node| node.is_element()
            && node.tag_name().name() == address_tag
            && node.text() == Some("Hinterhaus & Lager")));
        assert!(
            document
                .descendants()
                .any(|node| node.is_element() && node.text() == Some("12/345/67890"))
        );
    }
}

#[test]
fn distinct_tax_rates_are_not_merged_or_rounded_in_cii() {
    let mut input = fixture();
    input.lines.truncate(1);
    input.lines[0].quantity = "1".into();
    input.lines[0].net_unit_price = "10000".into();
    input.lines[0].tax.rate = "19.001".into();
    let mut second = input.lines[0].clone();
    second.id = "2".into();
    second.tax.rate = "19.004".into();
    input.lines.push(second);
    let calculated = invoice::calculate(input.clone()).unwrap();
    assert_eq!(calculated.taxes.len(), 2);
    assert_eq!(calculated.totals.tax_total, "3800.50");
    let xml = generate_cii(input).unwrap();
    let document = roxmltree::Document::parse(&xml).unwrap();
    let rates: Vec<_> = document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "ApplicableTradeTax")
        .filter(|node| {
            node.parent()
                .is_some_and(|parent| parent.tag_name().name() == "ApplicableHeaderTradeSettlement")
        })
        .flat_map(|node| node.children())
        .filter(|node| node.is_element() && node.tag_name().name() == "RateApplicablePercent")
        .filter_map(|node| node.text())
        .collect();
    assert_eq!(rates, ["19.001", "19.004"]);
}

#[test]
fn rejects_negative_or_subcent_prepayments_and_duplicate_line_ids() {
    for amount in ["-1.00", "0.005", "not-a-number"] {
        let mut input = fixture();
        input.prepaid_amount = Some(amount.into());
        assert!(generate_ubl(input.clone()).is_err());
        assert!(generate_cii(input.clone()).is_err());
        assert!(invoice::calculate(input).is_err());
    }
    let mut input = fixture();
    input.prepaid_amount = None;
    input.preceding_invoices = vec![invoice::PrecedingInvoice {
        invoice_number: "prior".into(),
        issue_date: Some("2026-02-30".into()),
        paid_amount: Some("-1".into()),
    }];
    assert!(generate_cii(input).is_err());
    let mut duplicate = fixture();
    duplicate.lines = vec![duplicate.lines[0].clone(); 2];
    assert!(generate_cii(duplicate).unwrap_err().contains("mehrfach"));
}
