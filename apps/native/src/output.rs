use crate::invoice::InvoiceInput;
use crate::ubl::generate_ubl;
use crate::validation::ValidatorBundle;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct SavedOutput {
    path: PathBuf,
    sha256: String,
    size: u64,
}

impl SavedOutput {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn verify_unchanged(&self) -> Result<(), String> {
        let mut file = File::open(&self.path)
            .map_err(|_| "Die gespeicherte Datei ist nicht mehr verfügbar.".to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() != self.size {
            return Err("Die gespeicherte Datei wurde verändert.".into());
        }
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        let actual = format!("{:x}", hash.finalize());
        if actual != self.sha256 {
            return Err("Die gespeicherte Datei wurde verändert.".into());
        }
        Ok(())
    }
}

pub fn save_xrechnung(
    input: InvoiceInput,
    destination: &Path,
    validator: &ValidatorBundle,
) -> Result<SavedOutput, String> {
    let xml = generate_ubl(input)?;
    validator.validate_xrechnung(xml.as_bytes())?;
    save_validated(xml.as_bytes(), destination, "xml")
}

pub fn save_zugferd(
    input: InvoiceInput,
    source_pdf: &[u8],
    destination: &Path,
    validator: &ValidatorBundle,
) -> Result<SavedOutput, String> {
    let xml = crate::cii::generate_cii(input)?;
    let pdf = crate::hybrid_pdf::prepare_hybrid_pdf(source_pdf, xml.as_bytes())?;
    validator.validate_zugferd(&pdf, xml.as_bytes())?;
    save_validated(&pdf, destination, "pdf")
}

fn save_validated(
    bytes: &[u8],
    destination: &Path,
    extension: &str,
) -> Result<SavedOutput, String> {
    let parent = destination.parent().ok_or("Ungültiger Speicherort.")?;
    if !parent.is_dir() {
        return Err("Der Zielordner existiert nicht.".into());
    }
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Ungültiger Dateiname.")?;
    if !name
        .to_ascii_lowercase()
        .ends_with(&format!(".{extension}"))
    {
        return Err(format!(
            "Die Ausgabe muss als .{extension} gespeichert werden."
        ));
    }
    let temporary = parent.join(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        output.write_all(bytes).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        fs::rename(&temporary, destination).map_err(|e| e.to_string())?;
        Ok(SavedOutput {
            path: destination.to_owned(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            size: bytes.len() as u64,
        })
    })();
    let _ = fs::remove_file(temporary);
    result
}
