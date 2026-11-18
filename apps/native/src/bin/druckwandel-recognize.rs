use druckwandel_native::recognition::{RecognitionMethod, RecognitionRegistry, RecognitionRequest};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Aufruf: druckwandel-recognize quelle.pdf")?,
    );
    let hash = format!("{:x}", Sha256::digest(fs::read(&path)?));
    let request = RecognitionRequest::from_pdf(path, &hash)?;
    let result = RecognitionRegistry::default().recognize(RecognitionMethod::Schema, &request)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
