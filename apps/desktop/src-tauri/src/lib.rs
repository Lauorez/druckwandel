mod archive;
mod backup;
mod datev;
mod diagnose;
mod guard;
mod paths;
mod protect;
mod validator;
mod workspace;

use archive::{
    get_archive_entry, get_archive_status, list_archive_entries, open_archive_entry_file,
    open_archive_folder, open_archive_report, save_and_archive_invoice, set_archive_signing,
    verify_archive,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrintJob {
    job_id: Option<String>,
    path: String,
    name: String,
    source_application: Option<String>,
    pages: Option<u32>,
    modified_ms: u128,
    size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewHandoff {
    schema_version: u32,
    job_id: String,
    handed_off_at: String,
    pdf_file_name: String,
    document_name: String,
    source_application: Option<String>,
    printer_name: String,
    pages: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewAcknowledgement {
    schema_version: u32,
    job_id: String,
    opened_at_ms: u128,
    status: String,
    message: Option<String>,
}

fn inbox() -> Result<PathBuf, String> {
    let base = paths::documents()?;
    let path = base.join("E-Rechnung Druckeingang");
    fs::create_dir_all(&path)
        .map_err(|error| format!("Druckeingang konnte nicht erstellt werden: {error}"))?;
    Ok(path)
}

pub(crate) fn atomic_write(target_path: &Path, bytes: &[u8]) -> Result<(), String> {
    let directory = target_path
        .parent()
        .ok_or_else(|| "Zielordner konnte nicht ermittelt werden.".to_string())?;
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Ungültiger Dateiname.".to_string())?;
    let temporary_path = directory.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<(), String> {
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        // rename replaces the destination atomically, including on Windows.
        // Never delete the last committed draft/memory before the replacement.
        fs::rename(&temporary_path, target_path).map_err(|error| error.to_string())?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary_path);
    result
}

fn write_review_draft_to(
    documents_directory: &Path,
    file_name: &str,
    contents: &str,
) -> Result<PathBuf, String> {
    if contents.len() > 5 * 1024 * 1024 {
        return Err("Die zu speichernde Datei ist zu groß.".to_string());
    }
    let candidate = Path::new(file_name);
    if candidate.file_name().and_then(|value| value.to_str()) != Some(file_name)
        || matches!(file_name, "" | "." | "..")
        || candidate
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("json"))
    {
        return Err("Ungültiger Dateiname oder Dateityp.".to_string());
    }
    serde_json::from_str::<serde_json::Value>(contents)
        .map_err(|error| format!("Der Entwurf ist beschädigt: {error}"))?;

    let directory = documents_directory.join("E-Rechnung Entwürfe");
    let target_path = directory.join(file_name);
    atomic_write(&target_path, contents.as_bytes())?;
    Ok(target_path)
}

#[tauri::command]
fn write_review_draft(file_name: String, contents: String) -> Result<String, String> {
    let _lock = crate::guard::exclusive();
    let documents_directory = paths::documents()?;
    Ok(
        write_review_draft_to(&documents_directory, &file_name, &contents)?
            .to_string_lossy()
            .into_owned(),
    )
}

fn validate_v1_memory(parsed: &serde_json::Value) -> bool {
    parsed
        .get("rules")
        .and_then(|value| value.as_array())
        .is_some_and(|rules| rules.len() <= 250)
        && parsed
            .get("tableRules")
            .is_none_or(|value| value.as_array().is_some_and(|rules| rules.len() <= 50))
}

fn validate_learning_memory(contents: &str) -> Result<(), String> {
    if contents.len() > 1024 * 1024 {
        return Err("Die gemerkten Angaben sind zu groß.".to_string());
    }
    let parsed: serde_json::Value = serde_json::from_str(contents)
        .map_err(|_| "Die gemerkten Angaben sind beschädigt.".to_string())?;
    let version = parsed.get("schemaVersion").and_then(|value| value.as_u64());
    let valid = match version {
        Some(1) => validate_v1_memory(&parsed),
        Some(2) => parsed
            .get("activeProfileId")
            .and_then(|value| value.as_str())
            .is_some_and(|id| !id.is_empty())
            && parsed
                .get("profiles")
                .and_then(|value| value.as_array())
                .is_some_and(|profiles| {
                    (1..=20).contains(&profiles.len())
                        && profiles.iter().all(|profile| {
                            profile.get("id").and_then(|value| value.as_str()).is_some_and(|id| !id.is_empty())
                                && profile
                                    .get("name")
                                    .and_then(|value| value.as_str())
                                    .is_some_and(|name| !name.is_empty() && name.len() <= 80)
                                && profile
                                    .get("memory")
                                    .is_some_and(validate_v1_memory)
                        })
                        && profiles.iter().any(|profile| {
                            profile.get("id").and_then(|value| value.as_str())
                                == parsed.get("activeProfileId").and_then(|value| value.as_str())
                        })
                }),
        _ => false,
    };
    if !valid {
        return Err("Die gemerkten Angaben haben ein unbekanntes Format.".to_string());
    }
    Ok(())
}

fn learning_memory_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(paths::app_data(app, false)?.join("correction-memory.json"))
}

#[tauri::command]
fn read_learning_memory(app: AppHandle) -> Result<Option<String>, String> {
    read_learning_memory_from(&learning_memory_path(&app)?)
}

fn read_learning_memory_from(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err("Die gemerkten Angaben konnten nicht gelesen werden.".to_string());
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn write_learning_memory(app: AppHandle, contents: String) -> Result<(), String> {
    let _lock = crate::guard::exclusive();
    write_learning_memory_to(&learning_memory_path(&app)?, &contents)
}

fn write_learning_memory_to(path: &Path, contents: &str) -> Result<(), String> {
    validate_learning_memory(contents)?;
    atomic_write(path, contents.as_bytes())
}

#[tauri::command]
fn list_print_jobs() -> Result<Vec<PrintJob>, String> {
    let mut jobs = Vec::new();
    for entry in fs::read_dir(inbox()?).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_ascii_lowercase().ends_with(".partial.pdf") {
            continue;
        }
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("pdf"))
        {
            continue;
        }
        jobs.push(print_job_from_path(path, Some(name))?);
    }
    jobs.sort_by(|left, right| right.modified_ms.cmp(&left.modified_ms));
    Ok(jobs)
}

fn is_job_id(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }

    value.bytes().enumerate().all(|(index, byte)| match index {
        8 | 13 | 18 | 23 => byte == b'-',
        _ => byte.is_ascii_hexdigit(),
    })
}

fn handoff_for_pdf(path: &Path) -> Option<ReviewHandoff> {
    let stem = path.file_stem()?.to_str()?;
    if !is_job_id(stem) {
        return None;
    }

    let metadata_path = path.with_file_name(format!("{stem}.printjob.json"));
    let metadata = fs::metadata(&metadata_path).ok()?;
    if metadata.len() > 64 * 1024 {
        return None;
    }

    let handoff: ReviewHandoff = serde_json::from_slice(&fs::read(metadata_path).ok()?).ok()?;
    let actual_file_name = path.file_name()?.to_str()?;
    if handoff.schema_version != 1
        || !handoff.job_id.eq_ignore_ascii_case(stem)
        || !handoff.pdf_file_name.eq_ignore_ascii_case(actual_file_name)
        || handoff.document_name.trim().is_empty()
        || handoff.handed_off_at.trim().is_empty()
        || handoff.printer_name.trim().is_empty()
    {
        return None;
    }

    Some(handoff)
}

fn print_job_from_path(path: PathBuf, fallback_name: Option<String>) -> Result<PrintJob, String> {
    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("Druckjob ist keine reguläre Datei.".to_string());
    }

    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_millis());
    let handoff = handoff_for_pdf(&path);
    let name = handoff
        .as_ref()
        .map(|value| value.document_name.clone())
        .or(fallback_name)
        .or_else(|| {
            path.file_name()
                .map(|value| value.to_string_lossy().into_owned())
        })
        .ok_or_else(|| "Dateiname des Druckjobs konnte nicht gelesen werden.".to_string())?;

    Ok(PrintJob {
        job_id: handoff.as_ref().map(|value| value.job_id.clone()),
        path: path.to_string_lossy().into_owned(),
        name,
        source_application: handoff
            .as_ref()
            .and_then(|value| value.source_application.clone()),
        pages: handoff.as_ref().and_then(|value| value.pages),
        modified_ms,
        size: metadata.len(),
    })
}

#[tauri::command]
fn get_print_job(job_id: String) -> Result<PrintJob, String> {
    if !is_job_id(&job_id) {
        return Err("Ungültige Druckjob-ID.".to_string());
    }

    let path = inbox()?.join(format!("{job_id}.pdf"));
    let job = print_job_from_path(path, None)?;
    if job
        .job_id
        .as_deref()
        .is_none_or(|value| !value.eq_ignore_ascii_case(&job_id))
    {
        return Err("Druckjob-Metadaten fehlen oder sind ungültig.".to_string());
    }
    Ok(job)
}

fn checked_job_path(input: &str) -> Result<PathBuf, String> {
    let root = inbox()?.canonicalize().map_err(|error| error.to_string())?;
    let path = Path::new(input)
        .canonicalize()
        .map_err(|error| format!("Druckjob nicht gefunden: {error}"))?;
    if !path.starts_with(root)
        || path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("pdf"))
    {
        return Err("Zugriff außerhalb des Druckeingangs abgelehnt.".to_string());
    }
    Ok(path)
}

#[tauri::command]
fn read_print_job(path: String) -> Result<String, String> {
    let path = checked_job_path(&path)?;
    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > 100 * 1024 * 1024 {
        return Err("PDF überschreitet das Limit von 100 MB.".to_string());
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Druckjob konnte nicht gelesen werden: {error}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
fn acknowledge_print_job(
    job_id: String,
    status: String,
    message: Option<String>,
) -> Result<(), String> {
    if !matches!(status.as_str(), "opened" | "failed") {
        return Err("Ungültiger Review-Status.".to_string());
    }
    let job = get_print_job(job_id.clone())?;
    let root = inbox()?;
    let target_path = root.join(format!("{job_id}.review.json"));
    let acknowledgement = ReviewAcknowledgement {
        schema_version: 1,
        job_id,
        opened_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis(),
        status,
        message: message.map(|value| value.chars().take(2000).collect()),
    };
    let bytes = serde_json::to_vec_pretty(&acknowledgement).map_err(|error| error.to_string())?;
    atomic_write(&target_path, &bytes)?;

    // Keep the validated job alive until the acknowledgement is committed.
    drop(job);
    Ok(())
}

#[tauri::command]
fn open_print_inbox() -> Result<(), String> {
    let path = inbox()?;
    #[cfg(target_os = "windows")]
    Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .setup(|_app| {
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                _app.deep_link().register_all()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            workspace::workspace_import,
            workspace::workspace_import_print,
            workspace::workspace_read,
            workspace::workspace_save,
            workspace::workspace_list,
            workspace::workspace_activate,
            workspace::workspace_error,
            workspace::workspace_scan_inbox,
            workspace::workspace_delete,
            workspace::workspace_dismiss_inbox,
            list_print_jobs,
            get_print_job,
            read_print_job,
            acknowledge_print_job,
            write_review_draft,
            read_learning_memory,
            write_learning_memory,
            open_print_inbox,
            save_and_archive_invoice,
            validator::cancel_invoice_validation,
            validator::validate_prepared_invoice,
            list_archive_entries,
            get_archive_entry,
            get_archive_status,
            set_archive_signing,
            verify_archive,
            open_archive_folder,
            open_archive_entry_file,
            open_archive_report,
            archive::archive_datev_source,
            datev::datev_get_profile,
            datev::datev_save_profile,
            datev::datev_export_status,
            datev::datev_check_duplicates,
            datev::datev_create_export,
            datev::datev_list_exports,
            datev::datev_resume_export,
            datev::datev_open_export,
            backup::backup_status,
            backup::backup_create,
            backup::backup_preview,
            backup::backup_confirm,
            backup::backup_resume,
            diagnose::diagnostic_report,
            diagnose::write_diagnostic_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running Druckwandel");
}

#[cfg(test)]
mod tests {
    use super::{
        is_job_id, read_learning_memory_from, validate_learning_memory, write_learning_memory_to,
        write_review_draft_to,
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn validates_and_atomically_replaces_artifacts() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erechnung-artifact-test-{unique}"));
        let target = write_review_draft_to(&directory, "review.json", r#"{"version":1}"#)
            .expect("first write");
        write_review_draft_to(&directory, "review.json", r#"{"version":2}"#).expect("replacement");

        assert_eq!(
            fs::read_to_string(&target).expect("read artifact"),
            r#"{"version":2}"#
        );
        assert!(write_review_draft_to(&directory, "../escape.json", "{}").is_err());
        assert!(write_review_draft_to(&directory, "review.txt", "{}").is_err());
        fs::remove_dir_all(directory).expect("remove isolated test directory");
    }

    #[test]
    fn validates_bounded_learning_memory() {
        assert!(validate_learning_memory(r#"{"schemaVersion":1,"rules":[]}"#).is_ok());
        assert!(
            validate_learning_memory(r#"{"schemaVersion":1,"rules":[],"tableRules":{}}"#).is_err()
        );
        assert!(validate_learning_memory(r#"{"schemaVersion":2,"rules":[]}"#).is_err());
        assert!(validate_learning_memory(
            r#"{"schemaVersion":2,"activeProfileId":"profile-standard","profiles":[{"id":"profile-standard","name":"Standard","memory":{"schemaVersion":1,"rules":[],"tableRules":[]}}]}"#
        )
        .is_ok());
        assert!(validate_learning_memory(r#"{"schemaVersion":1}"#).is_err());
        assert!(validate_learning_memory("not json").is_err());
    }

    #[test]
    fn persists_learning_memory_atomically() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erechnung-learning-test-{unique}"));
        let target = directory.join("correction-memory.json");
        let first = r#"{"schemaVersion":1,"rules":[],"tableRules":[]}"#;
        let second = r#"{"schemaVersion":1,"rules":[{"id":"test"}],"tableRules":[]}"#;

        assert_eq!(
            read_learning_memory_from(&target).expect("missing memory"),
            None
        );
        write_learning_memory_to(&target, first).expect("first learning write");
        write_learning_memory_to(&target, second).expect("replacement learning write");
        assert_eq!(
            read_learning_memory_from(&target).expect("read learning memory"),
            Some(second.to_string())
        );
        assert!(write_learning_memory_to(&target, "not json").is_err());
        fs::remove_dir_all(directory).expect("remove isolated learning directory");
    }

    #[test]
    fn print_job_ids_reject_paths_and_deep_link_payloads() {
        assert!(is_job_id("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
        assert!(!is_job_id(r"C:\Users\x\secret.pdf"));
        assert!(!is_job_id("../invoice"));
        assert!(!is_job_id("print-job/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
    }
}
