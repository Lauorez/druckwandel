use druckwandel_native::extraction::read_document_bytes;
use lopdf::{Document, Object, Stream, dictionary};

#[test]
fn accepts_indirect_calibrated_color_parameters_without_changing_source() {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let parameters = doc.add_object(
        dictionary! { "WhitePoint" => vec![Object::Real(0.9505), 1.into(), Object::Real(1.089)] },
    );
    let font = doc.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let resources = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font },
        "ColorSpace" => dictionary! { "CS1" => vec![Object::Name(b"CalRGB".to_vec()), Object::Reference(parameters)] }
    });
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"/CS1 cs 0 0 0 sc BT /F1 12 Tf 50 750 Td (Rechnung 123) Tj ET".to_vec(),
    ));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),595.into(),842.into()], "Resources" => resources, "Contents" => content });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    let mut source = Vec::new();
    doc.save_to(&mut source).unwrap();
    let original = source.clone();
    let pages = read_document_bytes(&source).unwrap();
    let text = pages[0]
        .tokens
        .iter()
        .map(|token| token.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("Rechnung"));
    assert!(text.contains("123"));
    assert_eq!(source, original);
}
