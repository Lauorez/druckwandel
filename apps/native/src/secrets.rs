//! Secret storage for optional cloud API credentials.
//! API keys are kept in the OS credential store (Windows Credential Manager /
//! macOS Keychain), never in settings.json, SQLite drafts, or logs.

use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use zeroize::Zeroize;

const KEYRING_SERVICE: &str = "de.druckwandel.wsystem";
const MAX_API_KEY_LEN: usize = 512;

/// Validated secret API key that redacts itself in Debug output and zeroizes on drop.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("Bitte einen W-System-API-Schlüssel eingeben.".into());
        }
        if trimmed.len() > MAX_API_KEY_LEN {
            return Err("Der API-Schlüssel ist ungewöhnlich lang und wurde abgelehnt.".into());
        }
        if !trimmed.bytes().all(|b| (0x21..=0x7E).contains(&b)) {
            return Err(
                "Der API-Schlüssel darf nur sichtbare ASCII-Zeichen ohne Leer- oder Steuerzeichen enthalten."
                    .into(),
            );
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey([REDACTED])")
    }
}

impl Drop for ApiKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

pub trait SecretStore: Send + Sync {
    fn load_wsystem_key(&self) -> Result<Option<ApiKey>, String>;
    fn save_wsystem_key(&self, key: &ApiKey) -> Result<(), String>;
    fn delete_wsystem_key(&self) -> Result<(), String>;

    fn has_wsystem_key(&self) -> Result<bool, String> {
        Ok(self.load_wsystem_key()?.is_some())
    }
}

/// OS-native secret store backed by Windows Credential Manager or macOS Keychain.
/// Scoped per workspace root so dev/test profiles do not collide with production keys.
#[derive(Clone, Debug)]
pub struct OsSecretStore {
    account: String,
}

impl OsSecretStore {
    pub fn for_workspace(root: &Path) -> Self {
        let canonical = root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf())
            .to_string_lossy()
            .into_owned();
        let hash = crate::file_io::sha256_hex(canonical.as_bytes());
        Self {
            account: format!("api-key-{}", &hash[..16]),
        }
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, &self.account)
            .map_err(|_| "System-Schlüsselbund konnte nicht geöffnet werden.".to_owned())
    }
}

impl SecretStore for OsSecretStore {
    fn load_wsystem_key(&self) -> Result<Option<ApiKey>, String> {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            match self.entry()?.get_password() {
                Ok(secret) => ApiKey::parse(&secret).map(Some),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(
                    "API-Schlüssel konnte nicht aus dem System-Schlüsselbund gelesen werden."
                        .into(),
                ),
            }
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = (&self.account, KEYRING_SERVICE);
            Err(
                "Auf diesem Betriebssystem ist kein sicherer System-Schlüsselbund konfiguriert."
                    .into(),
            )
        }
    }

    fn save_wsystem_key(&self, key: &ApiKey) -> Result<(), String> {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            self.entry()?.set_password(key.expose()).map_err(|_| {
                "API-Schlüssel konnte nicht im System-Schlüsselbund gespeichert werden.".to_owned()
            })
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = (key, &self.account, KEYRING_SERVICE);
            Err(
                "Auf diesem Betriebssystem ist kein sicherer System-Schlüsselbund konfiguriert."
                    .into(),
            )
        }
    }

    fn delete_wsystem_key(&self) -> Result<(), String> {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            match self.entry()?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err(
                    "API-Schlüssel konnte nicht aus dem System-Schlüsselbund entfernt werden."
                        .into(),
                ),
            }
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = (&self.account, KEYRING_SERVICE);
            Ok(())
        }
    }
}

/// In-memory secret store for tests without touching the OS keychain.
#[derive(Default)]
pub struct MemorySecretStore {
    key: Mutex<Option<ApiKey>>,
}

impl MemorySecretStore {
    pub fn with_key(key: ApiKey) -> Self {
        Self {
            key: Mutex::new(Some(key)),
        }
    }
}

impl SecretStore for MemorySecretStore {
    fn load_wsystem_key(&self) -> Result<Option<ApiKey>, String> {
        self.key
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| "Interner Sperrfehler beim Lesen des API-Schlüssels.".into())
    }

    fn save_wsystem_key(&self, key: &ApiKey) -> Result<(), String> {
        self.key
            .lock()
            .map(|mut guard| *guard = Some(key.clone()))
            .map_err(|_| "Interner Sperrfehler beim Speichern des API-Schlüssels.".into())
    }

    fn delete_wsystem_key(&self) -> Result<(), String> {
        self.key
            .lock()
            .map(|mut guard| *guard = None)
            .map_err(|_| "Interner Sperrfehler beim Entfernen des API-Schlüssels.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_validates_and_redacts_debug_output() {
        let key = ApiKey::parse("  wsk_test_12345  ").unwrap();
        assert_eq!(key.expose(), "wsk_test_12345");
        let debug = format!("{key:?}");
        assert!(!debug.contains("wsk_test_12345"));
        assert!(debug.contains("REDACTED"));
        assert!(ApiKey::parse("   ").is_err());
        assert!(ApiKey::parse("bad\r\nheader").is_err());
        assert!(ApiKey::parse("bad key").is_err());
    }

    #[test]
    fn memory_secret_store_roundtrips_and_deletes() {
        let store = MemorySecretStore::default();
        assert!(!store.has_wsystem_key().unwrap());
        let key = ApiKey::parse("wsk_mock_key").unwrap();
        store.save_wsystem_key(&key).unwrap();
        assert!(store.has_wsystem_key().unwrap());
        assert_eq!(
            store.load_wsystem_key().unwrap().unwrap().expose(),
            "wsk_mock_key"
        );
        store.delete_wsystem_key().unwrap();
        assert!(!store.has_wsystem_key().unwrap());
    }
}
