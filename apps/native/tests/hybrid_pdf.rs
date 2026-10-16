use druckwandel_native::{
    cii::generate_cii,
    hybrid_pdf::{extract_invoice_xml, prepare_hybrid_pdf},
    invoice::InvoiceInput,
};
use lopdf::{Document, Object, dictionary};
const SOURCE: &[u8] = include_bytes!("fixtures/source-invoice.pdf");
fn xml() -> Vec<u8> {
    generate_cii(
        serde_json::from_str::<InvoiceInput>(include_str!("fixtures/export/standard.json"))
            .unwrap(),
    )
    .unwrap()
    .into_bytes()
}
#[test]
fn preserves_page_contents_and_embeds_exact_invoice_once() {
    let xml = xml();
    let output = prepare_hybrid_pdf(SOURCE, &xml).unwrap();
    let second = prepare_hybrid_pdf(&output, &xml).unwrap();
    assert_eq!(extract_invoice_xml(&second).unwrap(), xml);
    let source = Document::load_mem(SOURCE).unwrap();
    let result = Document::load_mem(&second).unwrap();
    assert_eq!(source.get_pages().len(), result.get_pages().len());
    for ((_, a), (_, b)) in source.get_pages().iter().zip(result.get_pages().iter()) {
        assert_eq!(
            source.get_page_content(*a).unwrap(),
            result.get_page_content(*b).unwrap()
        );
    }
    let count = result
        .objects
        .values()
        .filter(|o| {
            o.as_dict()
                .ok()
                .and_then(|d| d.get(b"F").ok())
                .is_some_and(|f| f.as_str().ok() == Some(b"factur-x.xml"))
        })
        .count();
    assert_eq!(count, 1);
    assert!(
        prepare_hybrid_pdf(&output, b"different invoice")
            .unwrap_err()
            .contains("bereits andere")
    );
}
#[test]
fn rejects_unembedded_fonts_and_interactive_forms() {
    let mut doc = Document::load_mem(SOURCE).unwrap();
    doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    assert!(
        prepare_hybrid_pdf(&bytes, &xml())
            .unwrap_err()
            .contains("nicht eingebettete")
    );
    let mut doc = Document::load_mem(SOURCE).unwrap();
    let form =
        doc.add_object(dictionary! {"Fields"=>vec![Object::Dictionary(dictionary! {"FT"=>"Tx"})]});
    doc.catalog_mut().unwrap().set("AcroForm", form);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    assert!(
        prepare_hybrid_pdf(&bytes, &xml())
            .unwrap_err()
            .contains("PDF-Formulare")
    );
}
