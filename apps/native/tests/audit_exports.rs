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

#[test]
fn ubl_preserves_payment_means_without_iban_and_sanitizes_empty_and_control_chars() {
    let mut input = fixture();
    input.invoice_type = "381".into();
    input.due_date = Some("2026-09-01".into());
    input.service_date = Some("".into());
    input.preceding_invoices = vec![invoice::PrecedingInvoice {
        invoice_number: "RE-OLD-1".into(),
        issue_date: None,
        paid_amount: None,
    }];
    input.payment.means_code = "10".into();
    input.payment.iban = None;
    input.payment.payment_reference = Some("Bar-Ref\u{000C}\u{0000}123".into());
    input.seller.contact = Some(invoice::Contact::default());

    let ubl = generate_ubl(input.clone()).unwrap();
    let ubl_doc = roxmltree::Document::parse(&ubl).unwrap();
    assert!(
        ubl_doc
            .descendants()
            .any(|n| n.tag_name().name() == "PaymentMeansCode" && n.text() == Some("10"))
    );
    assert!(
        ubl_doc
            .descendants()
            .any(|n| n.tag_name().name() == "PaymentID" && n.text() == Some("Bar-Ref123"))
    );
    assert!(
        ubl_doc
            .descendants()
            .any(|n| n.tag_name().name() == "PaymentDueDate" && n.text() == Some("2026-09-01"))
    );
    assert!(
        !ubl_doc
            .descendants()
            .any(|n| n.tag_name().name() == "Contact")
    );
    assert!(
        !ubl_doc
            .descendants()
            .any(|n| n.tag_name().name() == "ActualDeliveryDate")
    );

    let cii = generate_cii(input).unwrap();
    let cii_doc = roxmltree::Document::parse(&cii).unwrap();
    assert!(
        !cii_doc
            .descendants()
            .any(|n| n.tag_name().name() == "DefinedTradeContact")
    );
    assert!(
        !cii_doc
            .descendants()
            .any(|n| n.tag_name().name() == "OccurrenceDateTime")
    );
    assert!(
        cii_doc
            .descendants()
            .any(|n| n.tag_name().name() == "PaymentReference" && n.text() == Some("Bar-Ref123"))
    );
}

#[test]
fn validates_delivery_country_and_trimmed_exemption_reason_and_empty_company_contact() {
    let mut input = fixture();
    let mut delivery = input.buyer.address.clone();
    delivery.country_code = "invalid".into();
    input.delivery_address = Some(delivery);
    assert!(
        invoice::validate_en16931(&input)
            .iter()
            .any(|e| e.contains("Lieferadresse"))
    );

    let mut exempt = fixture();
    exempt.lines[0].tax = invoice::Tax {
        category_code: "E".into(),
        rate: "0".into(),
        exemption_reason: Some("   ".into()),
        exemption_reason_code: None,
    };
    assert!(
        invoice::validate_en16931(&exempt)
            .iter()
            .any(|e| e.contains("Begründung"))
    );

    let empty_profile = druckwandel_native::settings::CompanyProfile::default();
    assert!(empty_profile.party().contact.is_none());
    let mut blank = InvoiceInput::blank();
    empty_profile.fill_missing(&mut blank);
    assert!(blank.seller.contact.is_none());
}

#[test]
fn hybrid_pdf_rejects_oversized_compressed_attachment_and_credit_note_xml() {
    use flate2::{Compression, write::ZlibEncoder};
    use lopdf::{Document, Object, Stream, dictionary};
    use std::io::Write;

    let base = include_bytes!("fixtures/source-invoice.pdf");
    let mut doc = Document::load_mem(base).unwrap();
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    let chunk = vec![b'A'; 1024 * 1024];
    for _ in 0..22 {
        encoder.write_all(&chunk).unwrap();
    }
    let compressed = encoder.finish().unwrap();
    let embedded = doc.add_object(Stream::new(
        dictionary! { "Type" => "EmbeddedFile", "Filter" => "FlateDecode" },
        compressed,
    ));
    doc.add_object(dictionary! {
        "Type" => "Filespec",
        "F" => Object::string_literal("factur-x.xml"),
        "UF" => Object::string_literal("factur-x.xml"),
        "EF" => dictionary! { "F" => embedded }
    });
    let mut bomb = Vec::new();
    doc.save_to(&mut bomb).unwrap();
    assert!(
        druckwandel_native::hybrid_pdf::extract_invoice_xml(&bomb)
            .unwrap_err()
            .contains("20 MB")
    );

    let mut credit_doc = Document::load_mem(base).unwrap();
    let embedded_cn = credit_doc.add_object(Stream::new(
        dictionary! { "Type" => "EmbeddedFile" },
        b"<ubl:CreditNote xmlns:ubl=\"urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2\"/>"
            .to_vec(),
    ));
    credit_doc.add_object(dictionary! {
        "Type" => "Filespec",
        "F" => Object::string_literal("other.xml"),
        "EF" => dictionary! { "F" => embedded_cn }
    });
    let mut credit_pdf = Vec::new();
    credit_doc.save_to(&mut credit_pdf).unwrap();
    let cii = generate_cii(fixture()).unwrap();
    assert!(
        druckwandel_native::hybrid_pdf::prepare_hybrid_pdf(&credit_pdf, cii.as_bytes())
            .unwrap_err()
            .contains("bereits andere maschinenlesbare")
    );
}
