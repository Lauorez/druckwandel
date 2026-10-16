//! Preserve source pages, require embedded fonts, attach exact CII bytes, then let
//! independent validators decide PDF/A-3B and Factur-X compliance before saving.
use lopdf::{Dictionary, Document, Object, Stream, StringFormat, dictionary};
use std::collections::BTreeMap;

fn resolved<'a>(doc: &'a Document, value: &'a Object) -> Result<&'a Object, String> {
    doc.dereference(value)
        .map(|(_, value)| value)
        .map_err(|e| e.to_string())
}

fn dict<'a>(doc: &'a Document, value: &'a Object) -> Result<&'a Dictionary, String> {
    resolved(doc, value)?.as_dict().map_err(|e| e.to_string())
}

fn font_embedded(doc: &Document, font: &Dictionary, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    let subtype = font
        .get(b"Subtype")
        .and_then(Object::as_name)
        .unwrap_or_default();
    if subtype == b"Type3" {
        return true;
    }
    if subtype == b"Type0" {
        return font
            .get(b"DescendantFonts")
            .ok()
            .and_then(|o| resolved(doc, o).ok())
            .and_then(|o| o.as_array().ok())
            .is_some_and(|children| {
                !children.is_empty()
                    && children.iter().all(|child| {
                        dict(doc, child).is_ok_and(|font| font_embedded(doc, font, depth + 1))
                    })
            });
    }
    font.get(b"FontDescriptor")
        .ok()
        .and_then(|o| dict(doc, o).ok())
        .is_some_and(|descriptor| {
            [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"]
                .iter()
                .any(|key| {
                    descriptor
                        .get(key)
                        .ok()
                        .and_then(|o| resolved(doc, o).ok())
                        .is_some_and(|o| o.as_stream().is_ok())
                })
        })
}

fn stream_bytes(doc: &Document, value: &Object) -> Result<Vec<u8>, String> {
    let stream = resolved(doc, value)?
        .as_stream()
        .map_err(|e| e.to_string())?;
    if stream.dict.has(b"Filter") {
        stream.decompressed_content().map_err(|e| e.to_string())
    } else {
        Ok(stream.content.clone())
    }
}

fn filename(spec: &Dictionary) -> Result<String, String> {
    let raw = spec
        .get(b"UF")
        .or_else(|_| spec.get(b"F"))
        .and_then(Object::as_str)
        .map_err(|e| e.to_string())?;
    if raw.starts_with(&[0xfe, 0xff]) {
        let utf16: Vec<_> = raw[2..]
            .chunks_exact(2)
            .map(|p| u16::from_be_bytes([p[0], p[1]]))
            .collect();
        String::from_utf16(&utf16).map_err(|e| e.to_string())
    } else {
        Ok(String::from_utf8_lossy(raw).into_owned())
    }
}

pub fn extract_invoice_xml(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let doc = Document::load_mem(bytes).map_err(|e| e.to_string())?;
    for object in doc.objects.values() {
        if let Ok(spec) = object.as_dict() {
            if filename(spec).is_ok_and(|name| name.eq_ignore_ascii_case("factur-x.xml")) {
                let ef = dict(&doc, spec.get(b"EF").map_err(|e| e.to_string())?)?;
                return stream_bytes(
                    &doc,
                    ef.get(b"F")
                        .or_else(|_| ef.get(b"UF"))
                        .map_err(|e| e.to_string())?,
                );
            }
        }
    }
    Err("Die PDF enthält keine factur-x.xml.".into())
}

pub fn prepare_hybrid_pdf(source: &[u8], cii: &[u8]) -> Result<Vec<u8>, String> {
    if source.len() > 100 * 1024 * 1024 {
        return Err("Die PDF überschreitet 100 MB.".into());
    }
    let mut doc =
        Document::load_mem(source).map_err(|e| format!("PDF kann nicht gelesen werden: {e}"))?;
    if doc.is_encrypted() {
        return Err("Verschlüsselte PDFs können nicht übernommen werden.".into());
    }
    let pages = doc.get_pages().len();
    if pages == 0 {
        return Err("Die PDF enthält keine Seiten.".into());
    }
    let catalog = doc.catalog().map_err(|e| e.to_string())?;
    if let Ok(form) = catalog.get(b"AcroForm") {
        let form = dict(&doc, form)?;
        if form
            .get(b"Fields")
            .ok()
            .and_then(|o| resolved(&doc, o).ok())
            .and_then(|o| o.as_array().ok())
            .is_none_or(|fields| !fields.is_empty())
        {
            return Err(
                "PDF-Formulare müssen zuerst als normale Druck-PDF ausgegeben werden.".into(),
            );
        }
    }
    let mut attachments = BTreeMap::<String, Object>::new();
    let mut associated = Vec::new();
    let mut identical = None;
    // Inspect all objects, including nested form resources and name-tree entries.
    for (id, object) in &doc.objects {
        let Ok(value) = object.as_dict() else {
            continue;
        };
        if value.get(b"Type").and_then(Object::as_name).ok() == Some(b"Font")
            && !font_embedded(&doc, value, 0)
        {
            return Err("Die PDF verwendet nicht eingebettete Schriften. Bitte im Ursprungsprogramm mit eingebetteten Schriften erneut ausgeben.".into());
        }
        if let Ok(ef) = value.get(b"EF") {
            let name = filename(value)?;
            let ef = dict(&doc, ef)?;
            let bytes = stream_bytes(
                &doc,
                ef.get(b"F")
                    .or_else(|_| ef.get(b"UF"))
                    .map_err(|e| e.to_string())?,
            )?;
            let lower = name.to_lowercase();
            let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
            let invoice_like = lower == "factur-x.xml"
                || lower == "xrechnung.xml"
                || lower.contains("zugferd")
                || head.contains("CrossIndustryInvoice")
                || head.contains("<Invoice");
            if invoice_like {
                if lower != "factur-x.xml" || bytes != cii || identical.is_some() {
                    return Err("Die PDF enthält bereits andere maschinenlesbare Rechnungsdaten. Diese werden nicht überschrieben oder verdoppelt.".into());
                }
                identical = Some(*id);
            }
            attachments.insert(name, Object::Reference(*id));
        }
    }
    if let Ok(af) = doc.catalog().map_err(|e| e.to_string())?.get(b"AF") {
        associated = resolved(&doc, af)?
            .as_array()
            .map_err(|e| e.to_string())?
            .clone();
    }
    let spec_id = if let Some(id) = identical {
        doc.get_object_mut(id)
            .and_then(Object::as_dict_mut)
            .map_err(|e| e.to_string())?
            .set("AFRelationship", Object::Name(b"Alternative".to_vec()));
        id
    } else {
        let embedded = doc.add_object(Stream::new(dictionary! {
            "Type" => "EmbeddedFile", "Subtype" => Object::Name(b"text/xml".to_vec()),
            "Params" => dictionary! { "Size" => cii.len() as i64, "ModDate" => Object::string_literal("D:19700101000000Z") }
        }, cii.to_vec()));
        doc.add_object(dictionary! {
            "Type" => "Filespec", "F" => Object::string_literal("factur-x.xml"), "UF" => Object::string_literal("factur-x.xml"),
            "Desc" => Object::string_literal("Factur-X/ZUGFeRD EN 16931 invoice data"),
            "AFRelationship" => "Alternative", "EF" => dictionary! { "F" => embedded, "UF" => embedded }
        })
    };
    attachments.insert("factur-x.xml".into(), spec_id.into());
    if !associated
        .iter()
        .any(|o| o.as_reference().ok() == Some(spec_id))
    {
        associated.push(spec_id.into());
    }
    let mut names = Vec::new();
    for (name, reference) in attachments {
        names.push(Object::string_literal(name));
        names.push(reference);
    }
    let embedded_names = doc.add_object(dictionary! { "Names" => names });
    let mut names_dict = doc
        .catalog()
        .map_err(|e| e.to_string())?
        .get(b"Names")
        .ok()
        .and_then(|o| dict(&doc, o).ok())
        .cloned()
        .unwrap_or_default();
    names_dict.set("EmbeddedFiles", embedded_names);
    let names_id = doc.add_object(names_dict);
    let xmp = doc.add_object(Stream::new(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        include_bytes!("../assets/factur-x.xmp").to_vec(),
    ));
    if !doc
        .catalog()
        .map_err(|e| e.to_string())?
        .has(b"OutputIntents")
    {
        let icc = doc.add_object(Stream::new(
            dictionary! { "N" => 3 },
            include_bytes!("../assets/srgb.icc").to_vec(),
        ));
        let intent = doc.add_object(dictionary! {
            "Type" => "OutputIntent", "S" => "GTS_PDFA1", "OutputConditionIdentifier" => Object::string_literal("sRGB IEC61966-2.1"),
            "RegistryName" => Object::string_literal("http://www.color.org"), "Info" => Object::string_literal("sRGB IEC61966-2.1"), "DestOutputProfile" => icc
        });
        doc.catalog_mut()
            .map_err(|e| e.to_string())?
            .set("OutputIntents", vec![Object::Reference(intent)]);
    }
    let catalog = doc.catalog_mut().map_err(|e| e.to_string())?;
    catalog.set("Names", names_id);
    catalog.set("AF", associated);
    catalog.set("Metadata", xmp);
    catalog.set("Lang", Object::string_literal("de-DE"));
    let info = doc.add_object(dictionary! { "Title" => Object::string_literal("Druckwandel"), "Producer" => Object::string_literal("Druckwandel") });
    doc.trailer.set("Info", info);
    let new_id = Object::String(
        uuid::Uuid::new_v4().as_bytes().to_vec(),
        StringFormat::Hexadecimal,
    );
    let original = doc
        .trailer
        .get(b"ID")
        .ok()
        .and_then(|o| o.as_array().ok())
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or_else(|| new_id.clone());
    doc.trailer.set("ID", vec![original, new_id]);
    doc.version = "1.7".into();
    let mut output = Vec::new();
    doc.save_to(&mut output).map_err(|e| e.to_string())?;
    let reopened = Document::load_mem(&output).map_err(|e| e.to_string())?;
    if reopened.get_pages().len() != pages || extract_invoice_xml(&output)? != cii {
        return Err("Seitenzahl oder eingebettete Rechnungsdaten wurden verändert.".into());
    }
    Ok(output)
}
