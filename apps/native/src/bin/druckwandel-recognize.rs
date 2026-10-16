use druckwandel_native::{
    extraction::read_document,
    recognition::{CONTRACT_VERSION, RecognitionMethod, RecognitionRegistry, RecognitionRequest},
};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Aufruf: druckwandel-recognize quelle.pdf")?,
    );
    let request = RecognitionRequest {
        schema_version: CONTRACT_VERSION,
        document_sha256: format!("{:x}", Sha256::digest(fs::read(&path)?)),
        source_pdf: path.clone(),
        pages: read_document(&path)?,
    };
    let result = RecognitionRegistry::default().recognize(RecognitionMethod::Schema, &request)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
