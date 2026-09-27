//! Windows DPAPI wraps the local archive signing key. Portable backups encrypt it separately.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use uuid::Uuid;

const MAGIC: &[u8] = b"ERDP1\0";
const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

pub(crate) fn load_key(path: &Path) -> Result<[u8; 32], String> {
    let bytes = fs::read(path).map_err(|error| {
        format!("Lokaler Schutzschlüssel konnte nicht gelesen werden: {error}")
    })?;
    if bytes.len() == 32 {
        let secret: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "Der lokale Schutzschlüssel ist beschädigt.".to_string())?;
        migrate_legacy(path, &secret)?;
        return Ok(secret);
    }
    if bytes.len() < MAGIC.len() || !bytes.starts_with(MAGIC) {
        return Err("Der lokale Schutzschlüssel ist beschädigt.".into());
    }
    let plain = unprotect(&bytes[MAGIC.len()..])?;
    let secret: [u8; 32] = plain
        .try_into()
        .map_err(|_| "Der lokale Schutzschlüssel ist beschädigt.".to_string())?;
    Ok(secret)
}

pub(crate) fn store_key(path: &Path, key: &[u8; 32]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Speicherort des Schutzschlüssels ist ungültig.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    if path.exists() {
        return Err("Der lokale Schutzschlüssel ist bereits vorhanden.".into());
    }
    write_wrapped(path, key)
}

fn migrate_legacy(path: &Path, key: &[u8; 32]) -> Result<(), String> {
    write_wrapped(path, key)
}

fn write_wrapped(path: &Path, key: &[u8; 32]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Speicherort des Schutzschlüssels ist ungültig.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut blob = MAGIC.to_vec();
    blob.extend(protect(key)?);
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("key"),
        Uuid::new_v4()
    ));
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&blob).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        fs::rename(&temporary, path).map_err(|error| error.to_string())?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    #[cfg(unix)]
    if result.is_ok() {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    result
}

fn protect(bytes: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        dpapi(bytes, true)
    }
    #[cfg(not(windows))]
    {
        Ok(bytes.to_vec())
    }
}

fn unprotect(bytes: &[u8]) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        dpapi(bytes, false)
    }
    #[cfg(not(windows))]
    {
        Ok(bytes.to_vec())
    }
}

#[cfg(windows)]
fn dpapi(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData,
    };
    if bytes.is_empty() {
        return Err("Der lokale Schutzschlüssel ist beschädigt.".into());
    }
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 || output.pbData.is_null() || output.cbData == 0 {
            return Err(if encrypt {
                "Der Archivschlüssel konnte unter Windows nicht geschützt werden.".into()
            } else {
                "Der Archivschlüssel gehört zu einem anderen Windows-Benutzer oder -Rechner.".into()
            });
        }
        let protected = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Ok(protected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_and_reads_signing_key() {
        let directory = std::env::temp_dir().join(format!("erechnung-key-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("archive-signing-key-v1.bin");
        let key = [7u8; 32];
        store_key(&path, &key).unwrap();
        assert_ne!(fs::read(&path).unwrap(), key);
        assert_eq!(load_key(&path).unwrap(), key);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn migrates_legacy_32_byte_key() {
        let directory = std::env::temp_dir().join(format!("erechnung-key-legacy-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("archive-signing-key-v1.bin");
        let key = [9u8; 32];
        fs::write(&path, key).unwrap();
        assert_eq!(load_key(&path).unwrap(), key);
        assert!(fs::read(&path).unwrap().starts_with(MAGIC));
        assert_eq!(load_key(&path).unwrap(), key);
        fs::remove_dir_all(directory).unwrap();
    }
}
