//! Mutable work in progress. Finished invoice evidence remains in archive.rs.
use base64::Engine;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;
use uuid::Uuid;

const MAX_PDF: usize = 100 * 1024 * 1024;
const MAX_SNAPSHOT: usize = 20 * 1024 * 1024;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentSummary {
    pub(crate) id: String,
    name: String,
    source_key: Option<String>,
    job_id: Option<String>,
    original_sha256: String,
    pub(crate) revision: i64,
    status: String,
    updated_at_ms: i64,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentDetail {
    pub(crate) document: DocumentSummary,
    pdf_base64: String,
    snapshot: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentPage {
    entries: Vec<DocumentSummary>,
    total: i64,
    last_opened_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxCandidate {
    key: String,
    job: super::PrintJob,
    legacy: bool,
}

fn root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(super::paths::app_data(app, true)?.join("workspace"))
}

pub(crate) fn export_source(app: &AppHandle, id: &str, revision: i64) -> Result<String, String> {
    let detail = read_from(&root(app)?, id)?;
    if detail.document.revision != revision || detail.snapshot.is_none() {
        return Err("Der Entwurf wurde inzwischen geändert. Bitte erneut speichern.".into());
    }
    Ok(detail.document.original_sha256)
}

pub(crate) fn datev_profile(
    app: &AppHandle,
    contents: Option<&str>,
) -> Result<Option<String>, String> {
    let db = database(&root(app)?)?;
    if let Some(value) = contents {
        if value.len() > 1024 * 1024 {
            return Err("Die Kanzleiangaben sind zu groß.".into());
        }
        let data: Value = serde_json::from_str(value).map_err(|e| e.to_string())?;
        if data["schemaVersion"] != 1 {
            return Err("Unbekanntes Format der Kanzleiangaben.".into());
        }
        db.execute("INSERT INTO settings(key,value) VALUES('datev_profile',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [value]).map_err(|e| e.to_string())?;
    }
    db.query_row(
        "SELECT value FROM settings WHERE key='datev_profile'",
        [],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn now() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .map_err(|e| e.to_string())
}

fn database(root: &Path) -> Result<Connection, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut db = Connection::open(root.join("workspace.sqlite3")).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
        .map_err(|e| e.to_string())?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let version: i64 = tx
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version > 2 {
        return Err("Die Entwürfe benötigen eine neuere Programmversion.".into());
    }
    if version == 0 {
        tx.execute_batch(
            "CREATE TABLE documents (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, source_key TEXT UNIQUE, job_id TEXT,
            original_sha256 TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'new' CHECK(status IN ('new','draft','done','error')),
            updated_at_ms INTEGER NOT NULL, error TEXT, snapshot TEXT);
            CREATE INDEX documents_updated ON documents(updated_at_ms DESC);
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE inbox_seen (source_key TEXT PRIMARY KEY, legacy INTEGER NOT NULL);
            CREATE TABLE dismissed_inbox (source_key TEXT PRIMARY KEY);
            PRAGMA user_version=2;",
        )
        .map_err(|e| e.to_string())?;
    } else if version == 1 {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS dismissed_inbox (source_key TEXT PRIMARY KEY);
            PRAGMA user_version=2;",
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(db)
}

const COLUMNS: &str =
    "id,name,source_key,job_id,original_sha256,revision,status,updated_at_ms,error";
fn summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentSummary> {
    Ok(DocumentSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        source_key: row.get(2)?,
        job_id: row.get(3)?,
        original_sha256: row.get(4)?,
        revision: row.get(5)?,
        status: row.get(6)?,
        updated_at_ms: row.get(7)?,
        error: row.get(8)?,
    })
}

fn get(db: &Connection, id: &str) -> Result<DocumentSummary, String> {
    if Uuid::parse_str(id).is_err() {
        return Err("Ungültige Rechnungskennung.".into());
    }
    db.query_row(
        &format!("SELECT {COLUMNS} FROM documents WHERE id=?1"),
        [id],
        summary,
    )
    .map_err(|_| "Die Rechnung wurde nicht gefunden.".into())
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn import_to(
    root: &Path,
    name: &str,
    pdf: &[u8],
    source_key: Option<&str>,
    job_id: Option<&str>,
) -> Result<DocumentSummary, String> {
    if pdf.len() > MAX_PDF || !pdf.starts_with(b"%PDF-") {
        return Err("Bitte wählen Sie eine PDF-Datei bis 100 MB.".into());
    }
    if name.trim().is_empty() || name.len() > 1000 {
        return Err("Der Dateiname ist leer oder zu lang.".into());
    }
    let hash = digest(pdf);
    let mut db = database(root)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if let Some(key) = source_key {
        let existing = tx
            .query_row(
                &format!("SELECT {COLUMNS} FROM documents WHERE source_key=?1"),
                [key],
                summary,
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(doc) = existing {
            if doc.original_sha256 != hash {
                return Err("Der Inhalt dieses Druckauftrags wurde nachträglich verändert.".into());
            }
            return Ok(doc);
        }
    }
    let id = Uuid::new_v4().to_string();
    let originals = root.join("originals");
    fs::create_dir_all(&originals).map_err(|e| e.to_string())?;
    let path = originals.join(format!("{id}.pdf"));
    // A DB row becomes visible only after the original is durable. A hard crash
    // before commit may leave an unreferenced file, never a half-written document.
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(pdf)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        tx.execute("INSERT INTO documents(id,name,source_key,job_id,original_sha256,updated_at_ms) VALUES(?1,?2,?3,?4,?5,?6)",
            params![id,name,source_key,job_id,hash,now()?]).map_err(|e| e.to_string())?;
        let doc = get(&tx, &id)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(doc)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&path);
    }
    result
}

pub(crate) fn read_from(root: &Path, id: &str) -> Result<DocumentDetail, String> {
    let db = database(root)?;
    let document = get(&db, id)?;
    let originals = root
        .join("originals")
        .canonicalize()
        .map_err(|_| "Die Originaldatei fehlt.".to_string())?;
    let path = originals
        .join(format!("{id}.pdf"))
        .canonicalize()
        .map_err(|_| "Die Originaldatei fehlt. Ihr Entwurf bleibt gespeichert.".to_string())?;
    if path.parent() != Some(originals.as_path()) {
        return Err("Ungültiger Speicherort der Originaldatei.".into());
    }
    let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_PDF as u64 {
        return Err("Die Originaldatei ist ungültig.".into());
    }
    let pdf = fs::read(path).map_err(|e| e.to_string())?;
    if digest(&pdf) != document.original_sha256 {
        return Err(
            "Die Originaldatei wurde verändert. Der Entwurf wird nicht mit dieser Datei geöffnet."
                .into(),
        );
    }
    let snapshot = db
        .query_row("SELECT snapshot FROM documents WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?;
    Ok(DocumentDetail {
        document,
        pdf_base64: base64::engine::general_purpose::STANDARD.encode(pdf),
        snapshot,
    })
}

fn validate_snapshot(contents: &str) -> Result<Value, String> {
    if contents.len() > MAX_SNAPSHOT {
        return Err("Der Entwurf ist zu groß.".into());
    }
    let value: Value =
        serde_json::from_str(contents).map_err(|_| "Der Entwurf ist beschädigt.".to_string())?;
    if value["schemaVersion"].as_u64() != Some(1)
        || value["extractionVersion"].as_str() != Some("text-layout-v1")
        || !value["draft"].is_object()
        || !value["initialDraft"].is_object()
        || !value["sourceSelections"].is_object()
        || !value["pendingSourceFields"].is_array()
        || !value["completed"].is_boolean()
    {
        return Err("Der Entwurf hat ein unbekanntes Format.".into());
    }
    for key in ["sourceExtraction", "extraction"] {
        if value[key]["pages"]
            .as_array()
            .is_none_or(|p| p.is_empty() || p.len() > 1000)
            || !value[key]["fields"].is_object()
            || !value[key]["lines"].is_array()
            || !value[key]["lineItems"].is_array()
            || !value[key]["warnings"].is_array()
        {
            return Err("Die gespeicherten Textstellen sind unvollständig.".into());
        }
    }
    Ok(value)
}

pub(crate) fn save_to(
    root: &Path,
    id: &str,
    expected_revision: i64,
    contents: &str,
) -> Result<DocumentSummary, String> {
    let value = validate_snapshot(contents)?;
    let mut db = database(root)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let current = get(&tx, id)?;
    if current.revision != expected_revision {
        return Err("Dieser Entwurf wurde inzwischen geändert. Bitte erneut öffnen; Ihre Eingaben bleiben hier sichtbar.".into());
    }
    let status = if value["completed"] == true {
        "done"
    } else {
        "draft"
    };
    tx.execute("UPDATE documents SET snapshot=?1,revision=revision+1,status=?2,error=NULL,updated_at_ms=?3 WHERE id=?4",
        params![contents,status,now()?,id]).map_err(|e| e.to_string())?;
    let doc = get(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(doc)
}

fn job_key(job: &super::PrintJob) -> String {
    job.job_id
        .as_ref()
        .map(|id| format!("print:{}", id.to_lowercase()))
        .unwrap_or_else(|| {
            format!(
                "file:{}:{}:{}",
                digest(job.path.to_lowercase().as_bytes()),
                job.modified_ms,
                job.size
            )
        })
}

fn scan_to(root: &Path, jobs: Vec<super::PrintJob>) -> Result<Vec<InboxCandidate>, String> {
    let mut db = database(root)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let initialized = tx
        .query_row(
            "SELECT value FROM settings WHERE key='inbox_initialized'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some();
    let mut candidates = Vec::new();
    for job in jobs {
        let key = job_key(&job);
        let dismissed: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM dismissed_inbox WHERE source_key=?1)",
                [&key],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if dismissed {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO inbox_seen(source_key,legacy) VALUES(?1,?2)",
            params![key, !initialized],
        )
        .map_err(|e| e.to_string())?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE source_key=?1)",
                [&key],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !exists {
            let legacy = tx
                .query_row(
                    "SELECT legacy FROM inbox_seen WHERE source_key=?1",
                    [&key],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            candidates.push(InboxCandidate { key, job, legacy });
        }
    }
    tx.execute(
        "INSERT OR IGNORE INTO settings(key,value) VALUES('inbox_initialized','1')",
        [],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(candidates)
}

fn delete_from(root: &Path, id: &str) -> Result<(), String> {
    if Uuid::parse_str(id).is_err() {
        return Err("Ungültige Rechnungskennung.".into());
    }
    let mut db = database(root)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let document = get(&tx, id)?;
    if let Some(key) = &document.source_key {
        tx.execute(
            "INSERT OR IGNORE INTO dismissed_inbox(source_key) VALUES(?1)",
            [key],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute("DELETE FROM documents WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM settings WHERE key='last_opened' AND value=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    let _ = fs::remove_file(root.join("originals").join(format!("{id}.pdf")));
    Ok(())
}

fn dismiss_from(root: &Path, key: &str) -> Result<(), String> {
    if key.is_empty() || key.len() > 2000 {
        return Err("Ungültige Kennung im Druckeingang.".into());
    }
    let mut db = database(root)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO dismissed_inbox(source_key) VALUES(?1)",
        [key],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn workspace_import(
    app: AppHandle,
    name: String,
    pdf_base64: String,
) -> Result<DocumentSummary, String> {
    if pdf_base64.len() > MAX_PDF * 4 / 3 + 4 {
        return Err("Die PDF-Datei ist zu groß.".into());
    }
    let _lock = crate::guard::exclusive();
    let pdf = base64::engine::general_purpose::STANDARD
        .decode(pdf_base64)
        .map_err(|_| "Die PDF-Datei konnte nicht gelesen werden.".to_string())?;
    import_to(&root(&app)?, &name, &pdf, None, None)
}

#[tauri::command]
pub(crate) fn workspace_import_print(
    app: AppHandle,
    path: String,
) -> Result<DocumentSummary, String> {
    let _lock = crate::guard::exclusive();
    let path = super::checked_job_path(&path)?;
    let job = super::print_job_from_path(path.clone(), None)?;
    if job.size > MAX_PDF as u64 {
        return Err("Die PDF-Datei ist zu groß.".into());
    }
    let pdf = fs::read(path).map_err(|e| e.to_string())?;
    import_to(
        &root(&app)?,
        &job.name,
        &pdf,
        Some(&job_key(&job)),
        job.job_id.as_deref(),
    )
}

#[tauri::command]
pub(crate) fn workspace_read(app: AppHandle, id: String) -> Result<DocumentDetail, String> {
    read_from(&root(&app)?, &id)
}

#[tauri::command]
pub(crate) fn workspace_save(
    app: AppHandle,
    id: String,
    expected_revision: i64,
    contents: String,
) -> Result<DocumentSummary, String> {
    let _lock = crate::guard::exclusive();
    save_to(&root(&app)?, &id, expected_revision, &contents)
}

#[tauri::command]
pub(crate) fn workspace_list(app: AppHandle, offset: Option<i64>) -> Result<DocumentPage, String> {
    let db = database(&root(&app)?)?;
    let total = db
        .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let mut statement = db
        .prepare(&format!(
            "SELECT {COLUMNS} FROM documents ORDER BY updated_at_ms DESC,id LIMIT 100 OFFSET ?1"
        ))
        .map_err(|e| e.to_string())?;
    let entries = statement
        .query_map([offset.unwrap_or(0).max(0)], summary)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let last_opened_id = db
        .query_row(
            "SELECT value FROM settings WHERE key='last_opened'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(DocumentPage {
        entries,
        total,
        last_opened_id,
    })
}

#[tauri::command]
pub(crate) fn workspace_activate(app: AppHandle, id: String) -> Result<(), String> {
    let _lock = crate::guard::exclusive();
    let mut db = database(&root(&app)?)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    get(&tx, &id)?;
    tx.execute("UPDATE documents SET error=NULL WHERE id=?1", [&id])
        .map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO settings(key,value) VALUES('last_opened',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [&id]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn workspace_error(app: AppHandle, id: String, message: String) -> Result<(), String> {
    let _lock = crate::guard::exclusive();
    let db = database(&root(&app)?)?;
    get(&db, &id)?;
    db.execute("UPDATE documents SET error=?1,status=CASE WHEN snapshot IS NULL THEN 'error' ELSE status END WHERE id=?2",
        params![message.chars().take(2000).collect::<String>(),id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn workspace_scan_inbox(app: AppHandle) -> Result<Vec<InboxCandidate>, String> {
    let _lock = crate::guard::exclusive();
    scan_to(&root(&app)?, super::list_print_jobs()?)
}

#[tauri::command]
pub(crate) fn workspace_delete(app: AppHandle, id: String) -> Result<(), String> {
    let _lock = crate::guard::exclusive();
    delete_from(&root(&app)?, &id)
}

#[tauri::command]
pub(crate) fn workspace_dismiss_inbox(app: AppHandle, key: String) -> Result<(), String> {
    let _lock = crate::guard::exclusive();
    dismiss_from(&root(&app)?, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestRoot(PathBuf);
    impl TestRoot {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("erechnung-workspace-test-{}", Uuid::new_v4())))
        }
    }
    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot() -> String {
        let extraction =
            serde_json::json!({"pages":[{}],"fields":{},"lines":[],"lineItems":[],"warnings":[]});
        serde_json::json!({"schemaVersion":1,"extractionVersion":"text-layout-v1","draft":{},"initialDraft":{},"sourceSelections":{},
            "pendingSourceFields":[],"completed":false,"sourceExtraction":extraction,"extraction":extraction}).to_string()
    }
    fn job(id: &str) -> super::super::PrintJob {
        super::super::PrintJob {
            job_id: Some(id.into()),
            path: format!("{id}.pdf"),
            name: id.into(),
            source_application: None,
            pages: None,
            modified_ms: 1,
            size: 6,
        }
    }
    #[test]
    fn imports_ten_jobs_idempotently_but_preserves_intentional_copies() {
        let t = TestRoot::new();
        for n in 0..10 {
            let key = format!("print:{n}");
            let a = import_to(&t.0, "invoice.pdf", b"%PDF-a", Some(&key), None).unwrap();
            let b = import_to(&t.0, "invoice.pdf", b"%PDF-a", Some(&key), None).unwrap();
            assert_eq!(a.id, b.id);
            assert!(import_to(&t.0, "invoice.pdf", b"%PDF-b", Some(&key), None).is_err());
        }
        let db = database(&t.0).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            10
        );
    }
    #[test]
    fn preserves_committed_revision_and_rejects_stale_or_invalid_writes() {
        let t = TestRoot::new();
        let a = import_to(&t.0, "a.pdf", b"%PDF-a", None, None).unwrap();
        assert_eq!(save_to(&t.0, &a.id, 0, &snapshot()).unwrap().revision, 1);
        assert!(save_to(&t.0, &a.id, 0, &snapshot()).is_err());
        assert!(save_to(&t.0, &a.id, 1, "{}").is_err());
        let reopened = read_from(&t.0, &a.id).unwrap();
        assert_eq!(reopened.snapshot, Some(snapshot()));
        assert_eq!(reopened.document.revision, 1);
        assert_eq!(reopened.document.status, "draft");
    }
    #[test]
    fn detects_missing_modified_and_escaping_originals() {
        let t = TestRoot::new();
        let a = import_to(&t.0, "a.pdf", b"%PDF-a", None, None).unwrap();
        let path = t.0.join("originals").join(format!("{}.pdf", a.id));
        fs::write(&path, b"%PDF-b").unwrap();
        assert!(read_from(&t.0, &a.id).err().unwrap().contains("verändert"));
        fs::remove_file(path).unwrap();
        assert!(read_from(&t.0, &a.id).is_err());
        assert!(read_from(&t.0, "../escape").is_err());
        assert!(import_to(&t.0, "a.pdf", b"not pdf", None, None).is_err());
    }
    #[test]
    fn baseline_is_durable_and_jobs_arriving_while_closed_are_not_history() {
        let t = TestRoot::new();
        assert!(scan_to(&t.0, vec![job("old")]).unwrap()[0].legacy);
        let rows = scan_to(&t.0, vec![job("old"), job("new")]).unwrap();
        assert!(rows[0].legacy);
        assert!(!rows[1].legacy);
        import_to(&t.0, "new.pdf", b"%PDF-a", Some("print:new"), Some("new")).unwrap();
        assert_eq!(
            scan_to(&t.0, vec![job("old"), job("new")]).unwrap().len(),
            1
        );
    }
    #[test]
    fn deleted_print_jobs_stay_out_of_the_inbox() {
        let t = TestRoot::new();
        assert!(scan_to(&t.0, vec![job("old")]).unwrap()[0].legacy);
        let imported = import_to(&t.0, "new.pdf", b"%PDF-a", Some("print:new"), Some("new")).unwrap();
        let original = t.0.join("originals").join(format!("{}.pdf", imported.id));
        assert!(original.is_file());
        delete_from(&t.0, &imported.id).unwrap();
        assert!(!original.exists());
        assert!(scan_to(&t.0, vec![job("new")]).unwrap().is_empty());
        let db = database(&t.0).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(get(&db, &imported.id).is_err());
        dismiss_from(&t.0, "print:old").unwrap();
        let remaining = scan_to(&t.0, vec![job("old"), job("later")]).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].key, "print:later");
    }
    #[test]
    fn failed_storage_never_acknowledges_a_revision() {
        let t = TestRoot::new();
        let a = import_to(&t.0, "a.pdf", b"%PDF-a", None, None).unwrap();
        save_to(&t.0, &a.id, 0, &snapshot()).unwrap();
        let db = database(&t.0).unwrap();
        db.execute_batch("CREATE TRIGGER deny_save BEFORE UPDATE ON documents BEGIN SELECT RAISE(ABORT,'test write failure'); END;").unwrap();
        assert!(save_to(&t.0, &a.id, 1, &snapshot()).is_err());
        assert_eq!(read_from(&t.0, &a.id).unwrap().document.revision, 1);
    }
}
