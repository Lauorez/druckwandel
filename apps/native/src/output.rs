use crate::invoice::InvoiceInput;
use crate::ubl::generate_ubl;
use crate::validation::ValidatorBundle;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct SavedOutput {
    path: PathBuf,
    sha256: String,
}

impl SavedOutput {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn verify_unchanged(&self) -> Result<(), String> {
        let bytes = fs::read(&self.path)
            .map_err(|_| "Die gespeicherte Datei ist nicht mehr verfügbar.".to_string())?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
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
    let parent = destination.parent().ok_or("Ungültiger Speicherort.")?;
    if !parent.is_dir() {
        return Err("Der Zielordner existiert nicht.".into());
    }
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Ungültiger Dateiname.")?;
    if !name.to_ascii_lowercase().ends_with(".xml") {
        return Err("XRechnung muss als XML gespeichert werden.".into());
    }
    let temporary = parent.join(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        output
            .write_all(xml.as_bytes())
            .map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        fs::rename(&temporary, destination).map_err(|e| e.to_string())?;
        Ok(SavedOutput {
            path: destination.to_owned(),
            sha256: format!("{:x}", Sha256::digest(xml.as_bytes())),
        })
    })();
    let _ = fs::remove_file(temporary);
    result
}
