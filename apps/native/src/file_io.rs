//! Bounded reads for untrusted input; verify the exact PDF bytes used downstream.
use sha2::{Digest, Sha256};
use std::{fmt::Write, fs::File, io::Read, path::Path};

pub const MAX_PDF_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_DRAFT_BYTES: u64 = 1024 * 1024;

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    encode_hex(Sha256::digest(bytes).as_slice())
}

pub fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(format!(
            "Datei ist keine reguläre Datei oder überschreitet {limit} Bytes."
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("Datei überschreitet {limit} Bytes."));
    }
    Ok(bytes)
}

pub fn read_verified_pdf(path: &Path, expected_sha256: &str) -> Result<Vec<u8>, String> {
    let bytes = read_limited(path, MAX_PDF_BYTES)?;
    if sha256_hex(&bytes) != expected_sha256 {
        return Err("Die Original-PDF wurde außerhalb der Anwendung verändert.".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_files_and_changed_pdfs() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("invoice.pdf");
        let original = b"%PDF-original";
        std::fs::write(&path, original).unwrap();
        let hash = sha256_hex(original);
        assert_eq!(read_verified_pdf(&path, &hash).unwrap(), original);
        assert!(read_limited(&path, 5).is_err());
        assert!(read_limited(directory.path(), 100).is_err());
        std::fs::write(&path, b"%PDF-modified").unwrap();
        assert!(
            read_verified_pdf(&path, &hash)
                .unwrap_err()
                .contains("verändert")
        );
    }
}
