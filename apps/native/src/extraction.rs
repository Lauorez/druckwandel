use pdf_extract::{MediaBox, OutputDev, OutputError, Transform};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceToken {
    pub id: String,
    pub page: u32,
    pub text: String,
    pub bounds: BoundingBox,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPage {
    pub page: u32,
    pub width: f64,
    pub height: f64,
    pub tokens: Vec<SourceToken>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextLine {
    pub page: u32,
    pub text: String,
    pub tokens: Vec<SourceToken>,
}

#[derive(Default)]
struct Collector {
    pages: Vec<DocumentPage>,
    glyphs: Vec<(String, BoundingBox)>,
    count: usize,
}

impl OutputDev for Collector {
    fn begin_page(
        &mut self,
        page: u32,
        media: &MediaBox,
        _: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.pages.push(DocumentPage {
            page,
            width: media.urx - media.llx,
            height: media.ury - media.lly,
            tokens: Vec::new(),
        });
        Ok(())
    }
    fn end_page(&mut self) -> Result<(), OutputError> {
        let page = self.pages.last_mut().expect("begin_page precedes end_page");
        self.glyphs
            .sort_by(|a, b| a.1.y.total_cmp(&b.1.y).then(a.1.x.total_cmp(&b.1.x)));
        let mut rows: Vec<Vec<(String, BoundingBox)>> = Vec::new();
        for glyph in self.glyphs.drain(..) {
            let center = glyph.1.y + glyph.1.height / 2.0;
            if let Some(row) = rows.iter_mut().rev().take(6).find(|row| {
                let first = &row[0].1;
                (center - first.y - first.height / 2.0).abs() <= 3.0_f64.max(glyph.1.height * 0.35)
            }) {
                row.push(glyph);
            } else {
                rows.push(vec![glyph]);
            }
        }
        for mut row in rows {
            row.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
            let mut word = String::new();
            let mut bounds = BoundingBox::default();
            let flush = |word: &mut String, bounds: &BoundingBox, tokens: &mut Vec<SourceToken>| {
                if !word.is_empty() {
                    tokens.push(SourceToken {
                        id: format!("p{}-t{}", page.page, tokens.len()),
                        page: page.page,
                        text: std::mem::take(word),
                        bounds: bounds.clone(),
                    });
                }
            };
            for (text, glyph) in row {
                if text.trim().is_empty() {
                    flush(&mut word, &bounds, &mut page.tokens);
                    continue;
                }
                let gap = glyph.x - (bounds.x + bounds.width);
                if !word.is_empty() && gap > glyph.height * 0.22 {
                    flush(&mut word, &bounds, &mut page.tokens);
                }
                if word.is_empty() {
                    bounds = glyph.clone();
                } else {
                    bounds.width = (glyph.x + glyph.width - bounds.x).max(bounds.width);
                    bounds.height = bounds.height.max(glyph.height);
                }
                word.push_str(&text);
            }
            flush(&mut word, &bounds, &mut page.tokens);
        }
        Ok(())
    }
    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _: f64,
        font_size: f64,
        text: &str,
    ) -> Result<(), OutputError> {
        self.count += text.len();
        if self.count > 2_000_000 {
            return Err(
                std::io::Error::other("PDF-Text überschreitet die Grenze von 2 MB.").into(),
            );
        }
        let height = (font_size * trm.m21.hypot(trm.m22)).abs().max(1.0);
        let glyph_width = (width * font_size * trm.m11.hypot(trm.m12)).abs();
        let page_height = self.pages.last().map_or(0.0, |page| page.height);
        self.glyphs.push((
            text.to_owned(),
            BoundingBox {
                x: trm.m31,
                y: page_height - trm.m32 - height,
                width: glyph_width,
                height,
            },
        ));
        Ok(())
    }
    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

pub fn read_document(path: &Path) -> Result<Vec<DocumentPage>, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 100 * 1024 * 1024 {
        return Err("PDF überschreitet 100 MB.".into());
    }
    read_document_bytes(&std::fs::read(path).map_err(|e| e.to_string())?)
}

pub fn read_document_bytes(bytes: &[u8]) -> Result<Vec<DocumentPage>, String> {
    if bytes.len() > 100 * 1024 * 1024 {
        return Err("PDF überschreitet 100 MB.".into());
    }
    let mut doc = lopdf::Document::load_mem(bytes).map_err(|e| e.to_string())?;
    if doc.is_encrypted() {
        return Err("Verschlüsselte PDF wird nicht unterstützt.".into());
    }
    normalize_color_parameters(&mut doc);
    crate::pdf_compat::prepare_text_content(&mut doc)?;
    let mut output = Collector::default();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::output_doc(&doc, &mut output)
    }))
    .map_err(|_| "Der PDF-Text konnte nicht sicher gelesen werden.".to_string())?
    .map_err(|e| e.to_string())?;
    if output.pages.is_empty() {
        return Err("Die PDF enthält keine Seiten.".into());
    }
    Ok(output.pages)
}

// pdf-extract 0.10 assumes calibrated color-space parameters are inline
// dictionaries. PDF permits indirect dictionaries as well. Resolve just those
// parameters in the extraction copy; never modify or resave the source PDF.
fn normalize_color_parameters(doc: &mut lopdf::Document) {
    use lopdf::Object;
    fn visit(
        value: &mut Object,
        dictionaries: &std::collections::BTreeMap<lopdf::ObjectId, Object>,
        depth: usize,
    ) {
        if depth > 64 {
            return;
        }
        match value {
            Object::Array(values) => {
                if values
                    .first()
                    .and_then(|value| value.as_name().ok())
                    .is_some_and(|name| matches!(name, b"CalRGB" | b"CalGray" | b"Lab"))
                {
                    if let Some(Object::Reference(id)) = values.get(1) {
                        if let Some(parameters) = dictionaries.get(id) {
                            values[1] = parameters.clone();
                        }
                    }
                }
                for value in values {
                    visit(value, dictionaries, depth + 1);
                }
            }
            Object::Dictionary(dict) => {
                for (_, value) in dict.iter_mut() {
                    visit(value, dictionaries, depth + 1);
                }
            }
            Object::Stream(stream) => {
                for (_, value) in stream.dict.iter_mut() {
                    visit(value, dictionaries, depth + 1);
                }
            }
            _ => {}
        }
    }
    // Only small dictionaries can be color parameters. Keep streams and page
    // content out of the lookup copy to avoid duplicating the PDF payload.
    let dictionaries = doc
        .objects
        .iter()
        .filter_map(|(id, value)| {
            let (_, value) = doc.dereference(value).ok()?;
            let dict = value.as_dict().ok()?;
            (dict.has(b"WhitePoint")).then(|| (*id, Object::Dictionary(dict.clone())))
        })
        .collect();
    for value in doc.objects.values_mut() {
        visit(value, &dictionaries, 0);
    }
}

pub fn reconstruct_lines(pages: &[DocumentPage]) -> Vec<TextLine> {
    let mut lines: Vec<TextLine> = Vec::new();
    for page in pages {
        for token in &page.tokens {
            if let Some(line) = lines.last_mut().filter(|line| {
                line.page == page.page
                    && (line.tokens[0].bounds.y - token.bounds.y).abs()
                        <= 3.0_f64.max(token.bounds.height * 0.35)
            }) {
                line.text.push(' ');
                line.text.push_str(&token.text);
                line.tokens.push(token.clone());
            } else {
                lines.push(TextLine {
                    page: page.page,
                    text: token.text.clone(),
                    tokens: vec![token.clone()],
                });
            }
        }
    }
    lines
}

pub fn extract_pages(path: &Path) -> Result<Vec<String>, String> {
    let pages = read_document(path)?;
    let lines = reconstruct_lines(&pages);
    Ok(pages
        .iter()
        .map(|page| {
            lines
                .iter()
                .filter(|line| line.page == page.page)
                .map(|line| line.text.clone())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect())
}
