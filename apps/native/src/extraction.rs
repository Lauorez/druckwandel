use std::path::Path;

pub fn extract_pages(path: &Path) -> Result<Vec<String>, String> {
    let pages = pdf_extract::extract_text_by_pages(path).map_err(|error| error.to_string())?;
    if pages.is_empty() {
        return Err("Die PDF enthält keine Seiten.".into());
    }
    Ok(pages)
}
