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
