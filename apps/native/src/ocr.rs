//! Offline OCR for pages without usable PDF text. Original PDFs remain untouched.
use crate::extraction::{BoundingBox, DocumentPage, SourceToken};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct OcrSettings {
    pub enabled: bool,
    pub languages: String,
}
impl Default for OcrSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            languages: "deu+eng".into(),
        }
    }
}

pub fn executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DRUCKWANDEL_TESSERACT") {
        return Some(path.into());
    }
    let name = if cfg!(windows) {
        "tesseract.exe"
    } else {
        "tesseract"
    };
    let mut roots = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/ocr")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        roots.insert(0, parent.join("resources/ocr"));
        roots.insert(1, parent.join("../Resources/ocr"));
    }
    for root in roots {
        let path = root.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn status() -> String {
    executable().map_or_else(|| "OCR ist nicht installiert. Das Offline-OCR-Paket muss beim Bauen bereitgestellt werden.".into(), |_| "Lokale OCR verfügbar (Tesseract, Deutsch/Englisch).".into())
}

fn enough_text(page: &DocumentPage) -> bool {
    page.tokens.len() >= 8
        && page
            .tokens
            .iter()
            .map(|t| t.text.chars().filter(|c| c.is_alphanumeric()).count())
            .sum::<usize>()
            >= 40
}

/// Reuse native text on text pages; recognize only text-poor pages, including mixed PDFs.
pub fn read_pages(
    bytes: &[u8],
    hash: &str,
    settings: &OcrSettings,
) -> Result<(Vec<DocumentPage>, Vec<String>), String> {
    // Keep the encrypted-document contract even when the text parser fails.
    let doc = crate::pdf_compat::load_document(bytes)?;
    if doc.is_encrypted() {
        return Err("Verschlüsselte PDF wird nicht unterstützt.".into());
    }
    drop(doc);
    let extracted = crate::extraction::read_document_bytes(bytes);
    if let Ok(pages) = &extracted
        && pages.iter().all(enough_text)
    {
        return Ok((pages.clone(), vec![]));
    }
    if !settings.enabled {
        return extracted.map(|pages| (pages, vec!["Automatische OCR ist ausgeschaltet. Textlose Seiten müssen manuell erfasst werden.".into()]));
    }
    let pdf = crate::preview::PreviewDocument::from_bytes(bytes.to_vec(), hash)?;
    if pdf.page_count() > 40 {
        return Err(
            "OCR unterstützt höchstens 40 Seiten je Dokument. Bitte die Rechnung aufteilen.".into(),
        );
    }
    let mut warnings = vec![];
    let mut pages = match extracted {
        Ok(pages) if pages.len() == pdf.page_count() => pages,
        other => {
            if let Err(error) = other {
                warnings.push(format!(
                    "PDF-Text konnte nicht gelesen werden; OCR wird versucht: {error}"
                ));
            }
            (0..pdf.page_count())
                .map(|index| {
                    let (width, height) = pdf.page_dimensions(index)?;
                    Ok(DocumentPage {
                        page: index as u32 + 1,
                        width,
                        height,
                        tokens: vec![],
                    })
                })
                .collect::<Result<Vec<_>, String>>()?
        }
    };
    let executable = executable().ok_or("Diese PDF benötigt OCR. Tesseract fehlt im Offline-Paket; die manuelle Erfassung bleibt möglich.")?;
    if !["deu+eng", "deu", "eng"].contains(&settings.languages.as_str()) {
        return Err("Nicht unterstützte OCR-Sprache. Bitte Deutsch/Englisch auswählen.".into());
    }
    let work = tempfile::tempdir().map_err(|e| e.to_string())?;
    let started = Instant::now();
    for (index, page) in pages.iter_mut().enumerate() {
        if enough_text(page) {
            continue;
        }
        if started.elapsed() > Duration::from_secs(120) {
            warnings
                .push("OCR-Zeitlimit erreicht. Verbleibende Seiten bitte manuell prüfen.".into());
            break;
        }
        let frame = pdf.render(index, 2000)?;
        let (page_width, page_height) = pdf.page_dimensions(index)?;
        page.width = page_width;
        page.height = page_height;
        let image = work.path().join("page.png");
        let file = File::create(&image).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(file, frame.width, frame.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&frame.pixels))
            .map_err(|e| e.to_string())?;
        let prefix = work.path().join("words");
        let output = prefix.with_extension("tsv");
        // Delete the previous page's result before invoking the next OCR process.
        if output.exists() {
            fs::remove_file(&output).map_err(|e| e.to_string())?;
        }
        let log_path = work.path().join("ocr.log");
        let mut command = Command::new(&executable);
        command
            .arg(&image)
            .arg(&prefix)
            .arg("-l")
            .arg(&settings.languages)
            .args(["--psm", "3"]);
        if let Some(parent) = executable.parent()
            && parent.join("tessdata").is_dir()
        {
            command.arg("--tessdata-dir").arg(parent.join("tessdata"));
        }
        command
            .arg("tsv")
            .env("OMP_THREAD_LIMIT", "2")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                File::create(&log_path).map_err(|e| e.to_string())?,
            ));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Lokale OCR konnte nicht gestartet werden: {e}"))?;
        let page_start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error.to_string());
                }
            }
            if page_start.elapsed() > Duration::from_secs(30)
                || started.elapsed() > Duration::from_secs(120)
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Die lokale OCR hat das Zeitlimit erreicht. Bitte die Rechnung aufteilen oder manuell erfassen.".into());
            }
            std::thread::sleep(Duration::from_millis(40));
        };
        if !status.success() {
            let log = limited_text(&log_path, 4096).unwrap_or_default();
            return Err(format!(
                "OCR fehlgeschlagen. Bitte Sprachpaket prüfen: {}",
                log.trim()
            ));
        }
        let tsv = limited_text(&output, 4 * 1024 * 1024)?;
        let tokens = parse_tsv(
            &tsv,
            page.page,
            frame.width,
            frame.height,
            page.width,
            page.height,
        );
        if tokens.iter().map(|t| t.text.len()).sum::<usize>()
            >= page.tokens.iter().map(|t| t.text.len()).sum::<usize>()
        {
            page.tokens = tokens;
        }
        warnings.push(format!("Seite {}: lokale OCR ({}) verwendet. Zahlen, Zuordnung und Vollständigkeit am Original prüfen.", page.page, settings.languages));
        if page.tokens.is_empty() {
            warnings.push(format!(
                "Seite {} enthält auch nach OCR keinen erkennbaren Text.",
                page.page
            ));
        }
    }
    Ok((pages, warnings))
}

fn limited_text(path: &Path, maximum: u64) -> Result<String, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > maximum {
        return Err("OCR-Ausgabe überschreitet die Größenbegrenzung.".into());
    }
    let mut text = String::new();
    file.take(maximum + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() as u64 > maximum {
        return Err("OCR-Ausgabe ist zu groß.".into());
    }
    Ok(text)
}

pub fn parse_tsv(
    tsv: &str,
    page: u32,
    image_width: u32,
    image_height: u32,
    width: f64,
    height: f64,
) -> Vec<SourceToken> {
    if image_width == 0
        || image_height == 0
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return vec![];
    }
    let mut rows: Vec<Vec<SourceToken>> = vec![];
    for (index, row) in tsv.lines().skip(1).take(100_000).enumerate() {
        let columns: Vec<_> = row.splitn(12, '\t').collect();
        if columns.len() != 12 || columns[0] != "5" {
            continue;
        }
        let Some(confidence) = columns[10]
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 35.0 && *v <= 100.0)
        else {
            continue;
        };
        let _ = confidence;
        let numbers = columns[6..10]
            .iter()
            .map(|s| s.parse::<f64>())
            .collect::<Result<Vec<_>, _>>();
        let Ok(b) = numbers else { continue };
        if b.iter().any(|v| !v.is_finite() || *v < 0.0)
            || b[2] <= 0.0
            || b[3] <= 0.0
            || b[0] + b[2] > image_width as f64 + 1.0
            || b[1] + b[3] > image_height as f64 + 1.0
        {
            continue;
        }
        let text = columns[11].trim();
        if text.is_empty() || text.len() > 1000 {
            continue;
        }
        let token = SourceToken {
            id: format!("p{page}-ocr{index}"),
            page,
            text: text.into(),
            bounds: BoundingBox {
                x: b[0] * width / image_width as f64,
                y: b[1] * height / image_height as f64,
                width: b[2] * width / image_width as f64,
                height: b[3] * height / image_height as f64,
            },
        };
        let center = token.bounds.y + token.bounds.height / 2.0;
        if let Some(row) = rows.iter_mut().rev().take(10).find(|row| {
            (center - row[0].bounds.y - row[0].bounds.height / 2.0).abs()
                <= 3.0_f64.max(token.bounds.height * 0.35)
        }) {
            row.push(token);
        } else {
            rows.push(vec![token]);
        }
        if index >= 30_000 {
            break;
        }
    }
    rows.sort_by(|a, b| a[0].bounds.y.total_cmp(&b[0].bounds.y));
    rows.into_iter()
        .flat_map(|mut row| {
            row.sort_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x));
            row
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tsv_words_have_pdf_coordinates_and_reject_bad_boxes_and_confidence() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\t1\t1\t1\t200\t100\t80\t30\t95\tRechnung\n5\t1\t1\t1\t1\t2\t300\t100\t50\t30\t20\tunsicher\n5\t1\t1\t1\t1\t3\tNaN\t1\t3\t4\t90\tdefekt\n";
        let words = parse_tsv(tsv, 2, 1000, 2000, 500.0, 1000.0);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].bounds.x, 100.0);
        assert_eq!(words[0].bounds.y, 50.0);
        assert_eq!(words[0].page, 2);
        assert!(words[0].id.starts_with("p2-ocr"));
    }
}
