use druckwandel_native::{
    file_io::sha256_hex,
    ocr::{OcrSettings, read_pages},
    preview::PreviewDocument,
    recognition::{RecognitionMethod, RecognitionRegistry, RecognitionRequest},
};
use lopdf::{Document, Object, Stream, dictionary};

fn mixed_scan() -> Vec<u8> {
    let bytes = include_bytes!("fixtures/source-invoice.pdf").to_vec();
    let hash = sha256_hex(&bytes);
    let preview = PreviewDocument::from_bytes(bytes.clone(), &hash).unwrap();
    let (width, height) = preview.page_dimensions(0).unwrap();
    let frame = preview.render(0, 2000).unwrap();
    let pixels = frame
        .pixels
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect::<Vec<_>>();
    let mut doc = Document::load_mem(&bytes).unwrap();
    let mut image = Stream::new(
        dictionary! {"Type"=>"XObject", "Subtype"=>"Image", "Width"=>frame.width, "Height"=>frame.height, "ColorSpace"=>"DeviceRGB", "BitsPerComponent"=>8},
        pixels,
    );
    image.compress().unwrap();
    let image_id = doc.add_object(image);
    let content = doc.add_object(Stream::new(
        dictionary! {},
        format!("q {width} 0 0 {height} 0 0 cm /Scan Do Q").into_bytes(),
    ));
    let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let pages = doc
        .get_object(root)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Pages")
        .unwrap()
        .as_reference()
        .unwrap();
    let page = doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![Object::Integer(0),Object::Integer(0),Object::Real(width as f32),Object::Real(height as f32)], "Resources"=>dictionary! {"XObject"=>dictionary! {"Scan"=>image_id}}, "Contents"=>content});
    let tree = doc.get_object_mut(pages).unwrap().as_dict_mut().unwrap();
    tree.get_mut(b"Kids")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(Object::Reference(page));
    tree.set("Count", 2);
    let mut output = vec![];
    doc.save_to(&mut output).unwrap();
    output
}

#[test]
fn text_pages_do_not_need_ocr_and_disabled_ocr_preserves_native_pages() {
    let bytes = include_bytes!("fixtures/source-invoice.pdf");
    let hash = sha256_hex(bytes);
    let (pages, warnings) = read_pages(
        bytes,
        &hash,
        &OcrSettings {
            enabled: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!pages[0].tokens.is_empty());
    assert!(warnings.is_empty());
    let scan = mixed_scan();
    let hash = sha256_hex(&scan);
    let (pages, warnings) = read_pages(
        &scan,
        &hash,
        &OcrSettings {
            enabled: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(pages.len(), 2);
    assert!(!pages[0].tokens.is_empty());
    assert!(pages[1].tokens.is_empty());
    assert!(!warnings.is_empty());
}

#[test]
#[ignore = "requires the pinned local Tesseract runtime and deu+eng language data"]
fn mixed_document_recognizes_image_only_page_locally_with_source_coordinates() {
    let scan = mixed_scan();
    let hash = sha256_hex(&scan);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gemischte-rechnung.pdf");
    std::fs::write(&path, &scan).unwrap();
    let request = RecognitionRequest::from_pdf(path.clone(), &hash).unwrap();
    assert_eq!(request.pages.len(), 2);
    assert!(
        request.pages[0]
            .tokens
            .iter()
            .all(|t| !t.id.contains("-ocr"))
    );
    let page = &request.pages[1];
    assert!(page.tokens.len() > 50);
    assert!(page.tokens.iter().all(|t| t.id.starts_with("p2-ocr")
        && t.bounds.x >= 0.0
        && t.bounds.y >= 0.0
        && t.bounds.x + t.bounds.width <= page.width + 1.0));
    let words = page
        .tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(words.contains("RE-2026-0001"), "{words}");
    assert!(request.warnings.iter().any(|w| w.contains("OCR")));
    let mut scanned_only = request.clone();
    scanned_only.pages.remove(0);
    let result = RecognitionRegistry::default()
        .recognize(RecognitionMethod::Schema, &scanned_only)
        .unwrap();
    assert_eq!(result.fields["invoiceNumber"].value, "RE-2026-0001");
    assert!(result.fields["invoiceNumber"].confidence <= 0.80);
    assert!(
        !result.lines.is_empty(),
        "OCR invoice positions: {:?}",
        result.warnings
    );
    assert_eq!(std::fs::read(path).unwrap(), scan);
}
