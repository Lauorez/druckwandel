//! User-exported diagnostic report without invoice contents or personal file paths.
use crate::paths;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticReport {
    schema_version: u32,
    created_at_ms: i64,
    app_version: String,
    os: String,
    arch: String,
    components: Vec<DiagnosticComponent>,
    archive_entries: i64,
    signed_entries: i64,
    signing_enabled: bool,
    workspace_documents: i64,
    datev_profile_present: bool,
    backup_reminder_due: bool,
    pending_restore: bool,
    notes: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DiagnosticComponent {
    name: String,
    version: String,
    status: String,
}

fn now_ms() -> Result<i64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64)
}

fn count(path: &Path, sql: &str) -> i64 {
    if !path.exists() {
        return 0;
    }
    Connection::open(path)
        .ok()
        .and_then(|db| db.query_row(sql, [], |row| row.get(0)).ok())
        .unwrap_or(0)
}

fn exists_flag(path: &Path, sql: &str) -> bool {
    if !path.exists() {
        return false;
    }
    Connection::open(path)
        .ok()
        .and_then(|db| {
            db.query_row(sql, [], |row| row.get::<_, Option<String>>(0))
                .ok()
        })
        .flatten()
        .is_some()
}

fn component(name: &str, version: &str, present: bool) -> DiagnosticComponent {
    DiagnosticComponent {
        name: name.into(),
        version: version.into(),
        status: if present { "vorhanden".into() } else { "fehlt".into() },
    }
}

fn validator_root(app: Option<&AppHandle>) -> PathBuf {
    if let Some(app) = app {
        if let Ok(dir) = app.path().resource_dir() {
            let nested = dir.join("resources").join("validators");
            if nested.exists() {
                return nested;
            }
            let flat = dir.join("validators");
            if flat.exists() {
                return flat;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/validators")
}

fn read_manifest(root: &Path) -> Value {
    fs::read(root.join("manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null)
}

pub(crate) fn report_from(
    documents: &Path,
    local: &Path,
    _roaming: &Path,
    validator_dir: &Path,
) -> Result<DiagnosticReport, String> {
    let manifest = read_manifest(validator_dir);
    let relative = manifest["java"]["relativePath"]
        .as_str()
        .unwrap_or("jre/bin/java");
    let mut java = validator_dir.join(relative);
    if cfg!(windows) && java.extension().is_none() {
        java.set_extension("exe");
    }
    let kosit = validator_dir.join(manifest["kosit"]["jar"].as_str().unwrap_or(""));
    let mustang = validator_dir.join(manifest["mustang"]["jar"].as_str().unwrap_or(""));
    let verapdf = validator_dir.join(manifest["verapdf"]["jar"].as_str().unwrap_or(""));
    let archive_db = documents.join("E-Rechnungsarchiv").join("archiv.sqlite3");
    let workspace_db = local.join("workspace").join("workspace.sqlite3");
    let reminder = if local.join("backup-state.json").exists() {
        fs::read(local.join("backup-state.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|value| value["lastBackupAtMs"].as_i64())
            .is_none_or(|at| now_ms().unwrap_or(0) - at > 14 * 24 * 60 * 60 * 1000)
    } else {
        true
    };
    let pending = fs::read(local.join("restore-journal.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| value["state"].as_str().map(str::to_string))
        .is_some_and(|state| state == "replacing");
    let mut notes = vec![
        "Der Bericht enthält keine PDF-, XML- oder Rechnungsinhalte.".into(),
        "DATEV_OFFICIAL_CHECK_PENDING".into(),
    ];
    if !java.exists() {
        notes.push("VALIDATORS_MISSING".into());
    }
    Ok(DiagnosticReport {
        schema_version: 1,
        created_at_ms: now_ms()?,
        app_version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        components: vec![
            component(
                "Java",
                manifest["java"]["version"].as_str().unwrap_or("?"),
                java.exists(),
            ),
            component(
                "KoSIT",
                manifest["kosit"]["engineVersion"].as_str().unwrap_or("?"),
                kosit.exists(),
            ),
            component(
                "Mustang",
                manifest["mustang"]["engineVersion"]
                    .as_str()
                    .unwrap_or("?"),
                mustang.exists(),
            ),
            component(
                "veraPDF",
                manifest["verapdf"]["engineVersion"]
                    .as_str()
                    .unwrap_or("?"),
                verapdf.exists(),
            ),
        ],
        archive_entries: count(&archive_db, "SELECT COUNT(*) FROM archive_entries"),
        signed_entries: count(
            &archive_db,
            "SELECT COALESCE(SUM(signing_key_id IS NOT NULL), 0) FROM archive_entries",
        ),
        signing_enabled: exists_flag(
            &archive_db,
            "SELECT value FROM archive_settings WHERE key='signing_enabled' AND value='1'",
        ),
        workspace_documents: count(&workspace_db, "SELECT COUNT(*) FROM documents"),
        datev_profile_present: exists_flag(
            &workspace_db,
            "SELECT value FROM settings WHERE key='datev_profile'",
        ),
        backup_reminder_due: reminder,
        pending_restore: pending,
        notes,
    })
}

fn haystack(text: &str) -> String {
    text.to_ascii_lowercase().replace("\\\\", "\\").replace('/', "\\")
}

fn forbidden(text: &str) -> bool {
    let lower = haystack(text);
    lower.contains("c:\\users\\")
        || lower.contains("\\users\\")
        || lower.contains("iban")
        || lower.contains("pdfcontents")
        || lower.contains("invoicenumber")
        || text.contains("BT-")
}

pub(crate) fn validate_export(contents: &str) -> Result<(), String> {
    if contents.len() > 256 * 1024 {
        return Err("Der Diagnosebericht ist zu groß.".into());
    }
    let value: Value =
        serde_json::from_str(contents).map_err(|_| "Der Diagnosebericht ist beschädigt.".to_string())?;
    if value["schemaVersion"] != 1 {
        return Err("Unbekanntes Diagnoseformat.".into());
    }
    if forbidden(contents) {
        return Err("Der Diagnosebericht enthält personenbezogene Pfade oder Kontodaten.".into());
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn diagnostic_report(app: AppHandle) -> Result<DiagnosticReport, String> {
    report_from(
        &paths::documents()?,
        &paths::app_data(&app, true)?,
        &paths::app_data(&app, false)?,
        &validator_root(Some(&app)),
    )
}

#[tauri::command]
pub(crate) fn write_diagnostic_report(
    app: AppHandle,
    destination: String,
) -> Result<String, String> {
    let report = report_from(
        &paths::documents()?,
        &paths::app_data(&app, true)?,
        &paths::app_data(&app, false)?,
        &validator_root(Some(&app)),
    )?;
    let contents = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    validate_export(&contents)?;
    let dest = PathBuf::from(&destination);
    if dest.extension().and_then(|value| value.to_str()) != Some("json") || !dest.is_absolute() {
        return Err("Bitte eine vollständige .json-Datei wählen.".into());
    }
    if dest
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("Ungültiger Diagnose-Pfad.".into());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::atomic_write(&dest, contents.as_bytes())?;
    Ok(dest.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn omits_paths_and_invoice_payloads() {
        let root = std::env::temp_dir().join(format!("erechnung-diagnose-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let report = report_from(&root.join("documents"), &root.join("local"), &root.join("roaming"), Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/validators").as_path()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.to_ascii_lowercase().contains("c:\\users\\"));
        assert!(!json.contains("invoiceNumber"));
        assert!(json.contains("DATEV_OFFICIAL_CHECK_PENDING"));
        let export = serde_json::json!({
            "schemaVersion": 1,
            "appVersion": report.app_version,
            "notes": report.notes,
        })
        .to_string();
        validate_export(&export).unwrap();
        assert!(validate_export(r#"{"schemaVersion":1,"path":"C:\\Users\\ada\\secret.pdf"}"#).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
