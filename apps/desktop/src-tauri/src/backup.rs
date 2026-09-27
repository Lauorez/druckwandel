//! Application backup and restore. Private keys travel only inside an age-encrypted package.
use crate::{archive, paths, protect};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;
use uuid::Uuid;

const FORMAT_VERSION: u32 = 1;
const MIN_PASSWORD: usize = 12;
const REMINDER_MS: i64 = 14 * 24 * 60 * 60 * 1000;
const PACK_MAGIC: &[u8] = b"ERBK1\n";

#[derive(Clone)]
struct Roots {
    documents: PathBuf,
    local: PathBuf,
    roaming: PathBuf,
}

impl Roots {
    fn from_app(app: &AppHandle) -> Result<Self, String> {
        Ok(Self {
            documents: paths::documents()?,
            local: paths::app_data(app, true)?,
            roaming: paths::app_data(app, false)?,
        })
    }
    fn archive(&self) -> archive::ArchivePaths {
        archive::ArchivePaths::new(&self.documents, &self.roaming)
    }
    fn workspace(&self) -> PathBuf {
        self.local.join("workspace")
    }
    fn datev(&self) -> PathBuf {
        self.documents
            .join("E-Rechnungsarchiv")
            .join("Steuerkanzlei")
    }
    fn drafts(&self) -> PathBuf {
        self.documents.join("E-Rechnung Entwürfe")
    }
    fn learning(&self) -> PathBuf {
        self.roaming.join("correction-memory.json")
    }
    fn state(&self) -> PathBuf {
        self.local.join("backup-state.json")
    }
    fn journal(&self) -> PathBuf {
        self.local.join("restore-journal.json")
    }
    fn staging(&self) -> PathBuf {
        self.local.join("restore-staging")
    }
    fn replaced(&self) -> PathBuf {
        self.local.join("replaced")
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ManifestFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format_version: u32,
    created_at_ms: i64,
    app_version: String,
    files: Vec<ManifestFile>,
    archive_entries: i64,
    drafts: i64,
    originals: i64,
    datev_exports: i64,
    has_signing_key: bool,
    chain_head: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Journal {
    schema_version: u32,
    id: String,
    state: String,
    created_at_ms: i64,
    staging: String,
    replaced: Option<String>,
    error: Option<String>,
    preview: RestorePreview,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestorePreview {
    archive_entries: i64,
    drafts: i64,
    originals: i64,
    datev_exports: i64,
    has_signing_key: bool,
    chain_head: String,
    created_at_ms: i64,
    issues: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupStatus {
    last_backup_at_ms: Option<i64>,
    last_path: Option<String>,
    same_volume: bool,
    reminder_due: bool,
    pending_restore: bool,
    pending_preview: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_preview: Option<RestorePreview>,
    default_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupResult {
    path: String,
    created_at_ms: i64,
    same_volume: bool,
    archive_entries: i64,
    drafts: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupState {
    schema_version: u32,
    last_backup_at_ms: i64,
    last_path: String,
    same_volume: bool,
}

fn now_ms() -> Result<i64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn checked_pack_path(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 400
        || value.contains('\0')
        || value.contains('\\')
        || value.starts_with('/')
        || value.contains("//")
    {
        return Err("Die Sicherung enthält einen unzulässigen Pfad.".into());
    }
    if Path::new(value)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Die Sicherung enthält einen unzulässigen Pfad.".into());
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD {
        return Err("Das Sicherungskennwort muss mindestens 12 Zeichen haben.".into());
    }
    Ok(())
}

fn volume_of(path: &Path) -> Option<String> {
    path.components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().to_lowercase())
}

fn same_volume(a: &Path, b: &Path) -> bool {
    match (volume_of(a), volume_of(b)) {
        (Some(left), Some(right)) => left == right,
        _ => true,
    }
}

fn default_path(documents: &Path) -> PathBuf {
    let stamp = chrono_like_date();
    documents
        .join("E-Rechnung Sicherungen")
        .join(format!("E-Rechnung-{stamp}.erechnung"))
}

fn chrono_like_date() -> String {
    let ms = now_ms().unwrap_or(0);
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m, d)
}

fn snapshot_sqlite(src: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let src_conn = Connection::open(src).map_err(|e| e.to_string())?;
    src_conn
        .busy_timeout(Duration::from_secs(8))
        .map_err(|e| e.to_string())?;
    let _ = src_conn.execute_batch("PRAGMA wal_checkpoint(FULL);");
    let mut dest_conn = Connection::open(dest).map_err(|e| e.to_string())?;
    {
        let backup = rusqlite::backup::Backup::new(&src_conn, &mut dest_conn)
            .map_err(|e| e.to_string())?;
        backup
            .run_to_completion(64, Duration::from_millis(50), None)
            .map_err(|e| format!("Die Datenbank konnte nicht gesichert werden: {e}"))?;
    }
    Ok(())
}

fn copy_checked(src: &Path, dest: &Path) -> Result<(u64, String), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(src, dest).map_err(|e| format!("Datei konnte nicht kopiert werden: {e}"))?;
    let size = fs::metadata(dest).map_err(|e| e.to_string())?.len();
    Ok((size, sha256_file(dest)?))
}

fn add_file(
    files: &mut Vec<ManifestFile>,
    work: &Path,
    pack_path: &str,
    src: &Path,
) -> Result<(), String> {
    checked_pack_path(pack_path)?;
    if !src.exists() {
        return Err(format!(
            "Die Datei {pack_path} fehlt und kann nicht gesichert werden."
        ));
    }
    let dest = work.join(pack_path);
    let (size, hash) = copy_checked(src, &dest)?;
    files.push(ManifestFile {
        path: pack_path.to_string(),
        sha256: hash,
        size,
    });
    Ok(())
}

fn add_bytes(
    files: &mut Vec<ManifestFile>,
    work: &Path,
    pack_path: &str,
    bytes: &[u8],
) -> Result<(), String> {
    checked_pack_path(pack_path)?;
    let dest = work.join(pack_path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&dest, bytes).map_err(|e| e.to_string())?;
    files.push(ManifestFile {
        path: pack_path.to_string(),
        sha256: sha256_hex(bytes),
        size: bytes.len() as u64,
    });
    Ok(())
}

fn collect(roots: &Roots, work: &Path) -> Result<Manifest, String> {
    let mut files = Vec::new();
    let archive = roots.archive();
    let mut archive_entries = 0;
    let mut chain_head = String::new();
    let mut signed = 0;
    if archive.database.exists() {
        snapshot_sqlite(&archive.database, &work.join("archive/archiv.sqlite3"))?;
        let hash = sha256_file(&work.join("archive/archiv.sqlite3"))?;
        let size = fs::metadata(work.join("archive/archiv.sqlite3"))
            .map_err(|e| e.to_string())?
            .len();
        files.push(ManifestFile {
            path: "archive/archiv.sqlite3".into(),
            sha256: hash,
            size,
        });
        let db = Connection::open(work.join("archive/archiv.sqlite3")).map_err(|e| e.to_string())?;
        archive_entries = db
            .query_row("SELECT COUNT(*) FROM archive_entries", [], |row| row.get(0))
            .unwrap_or(0);
        signed = db
            .query_row(
                "SELECT COALESCE(SUM(signing_key_id IS NOT NULL), 0) FROM archive_entries",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        chain_head = db
            .query_row("SELECT head_hash FROM archive_state WHERE singleton=1", [], |row| {
                row.get(0)
            })
            .unwrap_or_default();
        let mut statement = db
            .prepare("SELECT pdf_path, xml_path, validation_json FROM archive_entries")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for (pdf, xml, validation) in rows {
            for relative in [pdf, xml] {
                add_file(
                    &mut files,
                    work,
                    &format!("archive/{relative}"),
                    &archive.root.join(&relative),
                )?;
            }
            if let Some(report) = validation.as_deref().and_then(report_path) {
                let src = archive.root.join(&report);
                if src.exists() {
                    add_file(&mut files, work, &format!("archive/{report}"), &src)?;
                }
            }
        }
    }
    let mut originals = 0;
    let workspace = roots.workspace();
    let workspace_db = workspace.join("workspace.sqlite3");
    if workspace_db.exists() {
        snapshot_sqlite(&workspace_db, &work.join("workspace/workspace.sqlite3"))?;
        let hash = sha256_file(&work.join("workspace/workspace.sqlite3"))?;
        files.push(ManifestFile {
            path: "workspace/workspace.sqlite3".into(),
            sha256: hash,
            size: fs::metadata(work.join("workspace/workspace.sqlite3"))
                .map_err(|e| e.to_string())?
                .len(),
        });
        let db = Connection::open(work.join("workspace/workspace.sqlite3")).map_err(|e| e.to_string())?;
        let rows = {
            let mut statement = db
                .prepare("SELECT id, original_sha256 FROM documents")
                .map_err(|e| e.to_string())?;
            statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };
        for (id, expected) in rows {
            Uuid::parse_str(&id).map_err(|_| "Ungültige Vorgangskennung in den Entwürfen.".to_string())?;
            let src = workspace.join("originals").join(format!("{id}.pdf"));
            add_file(
                &mut files,
                work,
                &format!("workspace/originals/{id}.pdf"),
                &src,
            )?;
            let actual = sha256_file(&work.join(format!("workspace/originals/{id}.pdf")))?;
            if actual != expected {
                return Err("Eine Originaldatei passt nicht zum Entwurf und wird nicht gesichert.".into());
            }
            originals += 1;
        }
    }
    let mut datev_exports = 0;
    let datev_db = roots.datev().join("exporte.sqlite3");
    if datev_db.exists() {
        snapshot_sqlite(&datev_db, &work.join("datev/exporte.sqlite3"))?;
        files.push(ManifestFile {
            path: "datev/exporte.sqlite3".into(),
            sha256: sha256_file(&work.join("datev/exporte.sqlite3"))?,
            size: fs::metadata(work.join("datev/exporte.sqlite3"))
                .map_err(|e| e.to_string())?
                .len(),
        });
        let db = Connection::open(work.join("datev/exporte.sqlite3")).map_err(|e| e.to_string())?;
        datev_exports = db
            .query_row("SELECT COUNT(*) FROM exports", [], |row| row.get(0))
            .unwrap_or(0);
        let mut statement = db
            .prepare("SELECT export_id, path FROM export_files ORDER BY export_id, path")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for (id, name) in rows {
            let src = roots.datev().join("Dateien").join(&id).join(&name);
            if src.exists() {
                add_file(
                    &mut files,
                    work,
                    &format!("datev/files/{id}/{name}"),
                    &src,
                )?;
            }
        }
    }
    let mut drafts = 0;
    if roots.drafts().is_dir() {
        for entry in fs::read_dir(roots.drafts()).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.ends_with(".json")
                || name.contains('/')
                || name.contains('\\')
                || name.starts_with('.')
            {
                continue;
            }
            add_file(
                &mut files,
                work,
                &format!("drafts/{name}"),
                &entry.path(),
            )?;
            drafts += 1;
        }
    }
    if roots.learning().exists() {
        add_file(
            &mut files,
            work,
            "learning/correction-memory.json",
            &roots.learning(),
        )?;
    }
    let has_signing_key = archive.signing_key.exists();
    if has_signing_key {
        let key = protect::load_key(&archive.signing_key)?;
        add_bytes(&mut files, work, "keys/archive-signing-key.bin", &key)?;
    } else if signed > 0 {
        return Err("Der Archivschlüssel fehlt. Bestätigte Einträge können so nicht gesichert werden.".into());
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Manifest {
        format_version: FORMAT_VERSION,
        created_at_ms: now_ms()?,
        app_version: env!("CARGO_PKG_VERSION").into(),
        files,
        archive_entries,
        drafts,
        originals,
        datev_exports,
        has_signing_key,
        chain_head,
    })
}

fn report_path(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|value| value["reportPath"].as_str().map(str::to_string))
}

fn write_pack(work: &Path, manifest: &Manifest, dest: &Path) -> Result<(), String> {
    let manifest_bytes = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dest)
        .map_err(|e| e.to_string())?;
    file.write_all(PACK_MAGIC).map_err(|e| e.to_string())?;
    write_entry(&mut file, "manifest.json", &manifest_bytes)?;
    for item in &manifest.files {
        let bytes = fs::read(work.join(&item.path)).map_err(|e| e.to_string())?;
        if sha256_hex(&bytes) != item.sha256 {
            return Err("Eine Sicherungsdatei hat sich während des Schreibens verändert.".into());
        }
        write_entry(&mut file, &item.path, &bytes)?;
    }
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(())
}

fn write_entry(file: &mut File, path: &str, bytes: &[u8]) -> Result<(), String> {
    let path_bytes = path.as_bytes();
    file.write_all(&(path_bytes.len() as u16).to_be_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(path_bytes).map_err(|e| e.to_string())?;
    file.write_all(&(bytes.len() as u64).to_be_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    Ok(())
}

fn encrypt_file(src: &Path, dest: &Path, password: &str) -> Result<(), String> {
    let encryptor =
        age::Encryptor::with_user_passphrase(age::secrecy::SecretString::from(password.to_owned()));
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dest)
        .map_err(|e| e.to_string())?;
    let mut writer = encryptor
        .wrap_output(output)
        .map_err(|e| format!("Die Sicherung konnte nicht verschlüsselt werden: {e}"))?;
    let mut input = File::open(src).map_err(|e| e.to_string())?;
    io::copy(&mut input, &mut writer).map_err(|e| e.to_string())?;
    writer
        .finish()
        .map_err(|e| format!("Die Sicherung konnte nicht abgeschlossen werden: {e}"))?;
    Ok(())
}

fn decrypt_file(src: &Path, dest: &Path, password: &str) -> Result<(), String> {
    let input = File::open(src).map_err(|_| "Die Sicherungsdatei konnte nicht gelesen werden.".to_string())?;
    let decryptor = age::Decryptor::new(input)
        .map_err(|_| "Die Sicherungsdatei ist beschädigt oder kein gültiges Sicherungspaket.".to_string())?;
    let mut identity = age::scrypt::Identity::new(age::secrecy::SecretString::from(password.to_owned()));
    identity.set_max_work_factor(30);
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| "Das Sicherungskennwort ist falsch oder die Datei ist beschädigt.".to_string())?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dest)
        .map_err(|e| e.to_string())?;
    io::copy(&mut reader, &mut output).map_err(|_| {
        "Das Sicherungskennwort ist falsch oder die Datei ist beschädigt.".to_string()
    })?;
    output.sync_all().map_err(|e| e.to_string())?;
    Ok(())
}

fn unpack(pack: &Path, staging: &Path) -> Result<Manifest, String> {
    let bytes = fs::read(pack).map_err(|e| e.to_string())?;
    if bytes.len() < PACK_MAGIC.len() || !bytes.starts_with(PACK_MAGIC) {
        return Err("Die Sicherungsdatei hat ein unbekanntes Format.".into());
    }
    let mut offset = PACK_MAGIC.len();
    let mut files = Vec::new();
    while offset < bytes.len() {
        if offset + 2 > bytes.len() {
            return Err("Die Sicherungsdatei ist unvollständig.".into());
        }
        let path_len = u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        if offset + path_len + 8 > bytes.len() {
            return Err("Die Sicherungsdatei ist unvollständig.".into());
        }
        let path = std::str::from_utf8(&bytes[offset..offset + path_len])
            .map_err(|_| "Die Sicherungsdatei enthält einen ungültigen Namen.".to_string())?;
        checked_pack_path(path)?;
        offset += path_len;
        let size = u64::from_be_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        if offset + size > bytes.len() {
            return Err("Die Sicherungsdatei ist unvollständig.".into());
        }
        let content = &bytes[offset..offset + size];
        offset += size;
        let dest = staging.join(path);
        if dest.exists() {
            return Err("Die Sicherung enthält doppelte Dateien.".into());
        }
        if let Some(parent) = dest.parent() {
            if !parent.starts_with(staging) {
                return Err("Die Sicherung enthält einen unzulässigen Pfad.".into());
            }
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&dest, content).map_err(|e| e.to_string())?;
        files.push(path.to_string());
    }
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(staging.join("manifest.json")).map_err(|_| {
            "Die Sicherung enthält kein gültiges Inhaltsverzeichnis.".to_string()
        })?,
    )
    .map_err(|_| "Das Inhaltsverzeichnis der Sicherung ist beschädigt.".to_string())?;
    if manifest.format_version != FORMAT_VERSION {
        return Err("Diese Sicherung benötigt eine neuere Programmversion.".into());
    }
    let mut expected: Vec<_> = manifest.files.iter().map(|item| item.path.clone()).collect();
    expected.push("manifest.json".into());
    expected.sort();
    files.sort();
    if files != expected {
        return Err("Die Dateiliste der Sicherung stimmt nicht mit dem Inhaltsverzeichnis überein.".into());
    }
    for item in &manifest.files {
        checked_pack_path(&item.path)?;
        let actual = sha256_file(&staging.join(&item.path))?;
        if actual != item.sha256 {
            return Err(format!("Die gesicherte Datei {} wurde verändert.", item.path));
        }
    }
    Ok(manifest)
}

fn verify_staging(staging: &Path, manifest: &Manifest) -> Result<Vec<String>, String> {
    let mut issues = Vec::new();
    if manifest.has_signing_key {
        let key_path = staging.join("keys/archive-signing-key.bin");
        if fs::read(&key_path).ok().as_deref().map(protect_key_ok) != Some(true) {
            return Err("Der Archivschlüssel in der Sicherung fehlt oder ist beschädigt.".into());
        }
    }
    let archive_db = staging.join("archive/archiv.sqlite3");
    if archive_db.exists() {
        let documents = staging.join("verify-documents");
        let app_data = staging.join("verify-app-data");
        let live_root = documents.join("E-Rechnungsarchiv");
        copy_tree(&staging.join("archive"), &live_root)?;
        if manifest.has_signing_key {
            let bytes = fs::read(staging.join("keys/archive-signing-key.bin")).map_err(|e| e.to_string())?;
            let secret: [u8; 32] = bytes
                .try_into()
                .map_err(|_| "Der Archivschlüssel in der Sicherung ist beschädigt.".to_string())?;
            protect::store_key(&app_data.join("archive-signing-key-v1.bin"), &secret)?;
        }
        let paths = archive::ArchivePaths::new(&documents, &app_data);
        let report = archive::verify_archive_to(&paths)?;
        if !report.valid {
            issues.extend(report.issues.into_iter().map(|issue| issue.message));
        }
        if report.signed_count > 0 && !manifest.has_signing_key {
            return Err("Der Archivschlüssel fehlt in der Sicherung.".into());
        }
        if report.entry_count != manifest.archive_entries {
            issues.push("Die Anzahl der Archiveinträge weicht vom Inhaltsverzeichnis ab.".into());
        }
    }
    Ok(issues)
}

fn protect_key_ok(bytes: &[u8]) -> bool {
    bytes.len() == 32
}

fn copy_tree(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = dest.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if entry.file_name() != "archiv.sqlite3" {
            fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
        }
    }
    if src.join("archiv.sqlite3").exists() {
        fs::copy(src.join("archiv.sqlite3"), dest.join("archiv.sqlite3")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn path_ok(path: &Path, extension: &str, message: &str) -> Result<(), String> {
    if path.extension().and_then(|value| value.to_str()) != Some(extension) {
        return Err(message.into());
    }
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("Bitte einen vollständigen Pfad ohne übergeordnete Ordner wählen.".into());
    }
    Ok(())
}

fn destination_ok(roots: &Roots, dest: &Path) -> Result<(), String> {
    path_ok(dest, "erechnung", "Sicherungen verwenden die Endung .erechnung.")?;
    let forbidden = [
        roots.archive().root.clone(),
        roots.workspace(),
        roots.datev(),
        roots.drafts(),
        roots.staging(),
        roots.replaced(),
    ];
    if dest.exists() {
        return Err("Diese Sicherungsdatei existiert bereits. Bitte einen neuen Namen wählen.".into());
    }
    if let Some(parent) = dest.parent() {
        if parent.exists() {
            let candidate = parent.canonicalize().map_err(|e| e.to_string())?.join(
                dest.file_name()
                    .ok_or_else(|| "Ungültiger Dateiname für die Sicherung.".to_string())?,
            );
            for root in &forbidden {
                if let Ok(root) = root.canonicalize() {
                    if candidate.starts_with(&root) {
                        return Err("Die Sicherung darf nicht im Arbeits- oder Archivordner liegen.".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn create_to(roots: &Roots, dest: &Path, password: &str) -> Result<BackupResult, String> {
    validate_password(password)?;
    destination_ok(roots, dest)?;
    let work = roots
        .local
        .join("backup-work")
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = (|| {
        let manifest = collect(roots, &work)?;
        let pack = work.join("payload.pack");
        write_pack(&work, &manifest, &pack)?;
        let temporary = dest.with_extension("erechnung.tmp");
        let _ = fs::remove_file(&temporary);
        encrypt_file(&pack, &temporary, password)?;
        fs::rename(&temporary, dest).map_err(|e| e.to_string())?;
        let same = same_volume(&roots.documents, dest);
        let state = BackupState {
            schema_version: 1,
            last_backup_at_ms: manifest.created_at_ms,
            last_path: dest.to_string_lossy().into_owned(),
            same_volume: same,
        };
        fs::create_dir_all(roots.local.join("backup-work")).ok();
        fs::write(roots.state(), serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        Ok(BackupResult {
            path: dest.to_string_lossy().into_owned(),
            created_at_ms: manifest.created_at_ms,
            same_volume: same,
            archive_entries: manifest.archive_entries,
            drafts: manifest.drafts,
        })
    })();
    let _ = fs::remove_dir_all(&work);
    result
}

fn discard_staging(path: &str) {
    let staging = Path::new(path);
    if staging.exists() {
        let _ = fs::remove_dir_all(staging);
    }
}

fn clear_stale_preview(roots: &Roots) -> Result<(), String> {
    if let Some(existing) = read_journal(roots)? {
        if existing.state == "preview" {
            discard_staging(&existing.staging);
            let _ = fs::remove_file(roots.journal());
        }
    }
    Ok(())
}

fn finish_restore_journal(roots: &Roots, mut journal: Journal) -> Result<RestorePreview, String> {
    let staging = journal.staging.clone();
    journal.state = "complete".into();
    write_journal(roots, &journal)?;
    discard_staging(&staging);
    Ok(journal.preview)
}

fn preview_to(roots: &Roots, source: &Path, password: &str) -> Result<RestorePreview, String> {
    path_ok(source, "erechnung", "Sicherungen verwenden die Endung .erechnung.")?;
    validate_password(password)?;
    if let Some(existing) = read_journal(roots)? {
        if existing.state == "replacing" {
            return Err("Eine Wiederherstellung wurde unterbrochen. Bitte zuerst fortsetzen oder die bisherigen Daten prüfen.".into());
        }
    }
    clear_stale_preview(roots)?;
    let id = Uuid::new_v4().to_string();
    let staging = roots.staging().join(&id);
    if staging.exists() {
        return Err("Ein Wiederherstellungsordner ist bereits vorhanden.".into());
    }
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        let pack = staging.join("payload.pack");
        decrypt_file(source, &pack, password)?;
        let unpacked = staging.join("tree");
        fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
        let manifest = unpack(&pack, &unpacked)?;
        let issues = verify_staging(&unpacked, &manifest)?;
        let preview = RestorePreview {
            archive_entries: manifest.archive_entries,
            drafts: manifest.drafts,
            originals: manifest.originals,
            datev_exports: manifest.datev_exports,
            has_signing_key: manifest.has_signing_key,
            chain_head: manifest.chain_head,
            created_at_ms: manifest.created_at_ms,
            issues,
        };
        if !preview.issues.is_empty() {
            return Err(format!(
                "Die Sicherung ist unvollständig: {}",
                preview.issues.join(" ")
            ));
        }
        write_journal(
            roots,
            &Journal {
                schema_version: 1,
                id: id.clone(),
                state: "preview".into(),
                created_at_ms: now_ms()?,
                staging: staging.to_string_lossy().into_owned(),
                replaced: None,
                error: None,
                preview: preview.clone(),
            },
        )?;
        Ok(preview)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_file(roots.journal());
    }
    result
}

fn write_journal(roots: &Roots, journal: &Journal) -> Result<(), String> {
    fs::create_dir_all(&roots.local).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(journal).map_err(|e| e.to_string())?;
    let temporary = roots.journal().with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    fs::rename(temporary, roots.journal()).map_err(|e| e.to_string())?;
    Ok(())
}

fn read_journal(roots: &Roots) -> Result<Option<Journal>, String> {
    if !roots.journal().exists() {
        return Ok(None);
    }
    let parsed = serde_json::from_slice(&fs::read(roots.journal()).map_err(|e| e.to_string())?)
        .map_err(|_| "Das Wiederherstellungsprotokoll ist beschädigt.".to_string())?;
    Ok(Some(parsed))
}

fn move_dir(src: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        return Err("Das Wiederherstellungsziel ist belegt. Die bisherigen Daten wurden nicht gelöscht.".into());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if !src.exists() {
        return Ok(());
    }
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_tree(src, dest)?;
            fs::remove_dir_all(src).map_err(|e| {
                format!("Die bisherigen Daten wurden kopiert, der Ursprungsordner bleibt erhalten: {e}")
            })?;
            Ok(())
        }
    }
}

fn apply_tree(unpacked: &Path, roots: &Roots) -> Result<(), String> {
    let archive_src = unpacked.join("archive");
    if archive_src.exists() {
        let dest = roots.archive().root;
        fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        copy_tree(&archive_src, &dest)?;
    }
    let workspace_src = unpacked.join("workspace");
    if workspace_src.exists() {
        let dest = roots.workspace();
        fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        copy_tree(&workspace_src, &dest)?;
    }
    let datev_src = unpacked.join("datev");
    if datev_src.exists() {
        let dest = roots.datev();
        fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        if datev_src.join("exporte.sqlite3").exists() {
            fs::copy(datev_src.join("exporte.sqlite3"), dest.join("exporte.sqlite3"))
                .map_err(|e| e.to_string())?;
        }
        let files = datev_src.join("files");
        if files.exists() {
            copy_tree(&files, &dest.join("Dateien"))?;
        }
    }
    let drafts_src = unpacked.join("drafts");
    if drafts_src.exists() {
        let dest = roots.drafts();
        fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(&drafts_src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            fs::copy(entry.path(), dest.join(entry.file_name())).map_err(|e| e.to_string())?;
        }
    }
    let learning = unpacked.join("learning/correction-memory.json");
    if learning.exists() {
        fs::create_dir_all(&roots.roaming).map_err(|e| e.to_string())?;
        fs::copy(learning, roots.learning()).map_err(|e| e.to_string())?;
    }
    let key = unpacked.join("keys/archive-signing-key.bin");
    if key.exists() {
        let bytes = fs::read(&key).map_err(|e| e.to_string())?;
        let secret: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "Der Archivschlüssel in der Sicherung ist beschädigt.".to_string())?;
        let target = roots.archive().signing_key;
        if target.exists() {
            fs::remove_file(&target).map_err(|e| e.to_string())?;
        }
        protect::store_key(&target, &secret)?;
    }
    Ok(())
}

fn confirm_to(roots: &Roots) -> Result<RestorePreview, String> {
    let mut journal = read_journal(roots)?
        .ok_or_else(|| "Es liegt keine geprüfte Sicherung zur Wiederherstellung vor.".to_string())?;
    if journal.state != "preview" && journal.state != "replacing" {
        return Err("Diese Wiederherstellung ist bereits abgeschlossen oder ungültig.".into());
    }
    let staging = PathBuf::from(&journal.staging);
    let unpacked = staging.join("tree");
    if !unpacked.join("manifest.json").exists() {
        return Err("Der geprüfte Wiederherstellungsstand fehlt. Die bisherigen Daten bleiben unverändert.".into());
    }
    let stamp = journal.id.clone();
    let replaced = roots.replaced().join(&stamp);
    journal.state = "replacing".into();
    journal.replaced = Some(replaced.to_string_lossy().into_owned());
    write_journal(roots, &journal)?;
    finish_swap(roots, &journal, &unpacked, &replaced)?;
    finish_restore_journal(roots, journal)
}

fn next_overflow_path(replaced: &Path, name: &str) -> PathBuf {
    let overflow = replaced.join(format!("{name}-nach-unterbrechung"));
    if !overflow.exists() {
        return overflow;
    }
    for suffix in 2..1000 {
        let candidate = replaced.join(format!("{name}-nach-unterbrechung-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    replaced.join(format!("{name}-nach-unterbrechung-{}", Uuid::new_v4()))
}

fn relocate_live(src: &Path, replaced: &Path, name: &str) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    let dest = if replaced.join(name).exists() {
        // The first move already succeeded. Keep anything written after the
        // interruption instead of letting apply_tree overwrite it in place.
        next_overflow_path(replaced, name)
    } else {
        replaced.join(name)
    };
    if src.is_file() {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        return fs::rename(src, &dest)
            .or_else(|_| fs::copy(src, &dest).and_then(|_| fs::remove_file(src)))
            .map_err(|e| e.to_string());
    }
    move_dir(src, &dest)
}

fn finish_swap(
    roots: &Roots,
    journal: &Journal,
    unpacked: &Path,
    replaced: &Path,
) -> Result<(), String> {
    fs::create_dir_all(replaced).map_err(|e| e.to_string())?;
    relocate_live(&roots.archive().root, replaced, "E-Rechnungsarchiv")?;
    relocate_live(&roots.workspace(), replaced, "workspace")?;
    relocate_live(&roots.drafts(), replaced, "drafts")?;
    relocate_live(&roots.learning(), replaced, "correction-memory.json")?;
    relocate_live(
        &roots.archive().signing_key,
        replaced,
        "archive-signing-key-v1.bin",
    )?;
    relocate_live(&roots.datev(), replaced, "Steuerkanzlei")?;
    apply_tree(unpacked, roots)?;
    let _ = journal;
    Ok(())
}

fn resume_to(roots: &Roots) -> Result<RestorePreview, String> {
    let journal = read_journal(roots)?
        .ok_or_else(|| "Es liegt kein unterbrochener Wiederherstellungsvorgang vor.".to_string())?;
    if journal.state == "complete" {
        return Ok(journal.preview);
    }
    if journal.state != "replacing" {
        return Err("Bitte die Sicherung zuerst prüfen, bevor sie übernommen wird.".into());
    }
    let staging = PathBuf::from(&journal.staging);
    let replaced = PathBuf::from(
        journal
            .replaced
            .as_ref()
            .ok_or_else(|| "Das Wiederherstellungsprotokoll ist unvollständig.".to_string())?,
    );
    finish_swap(roots, &journal, &staging.join("tree"), &replaced)?;
    finish_restore_journal(roots, journal)
}

fn status_to(roots: &Roots) -> Result<BackupStatus, String> {
    let state = if roots.state().exists() {
        serde_json::from_slice::<BackupState>(&fs::read(roots.state()).map_err(|e| e.to_string())?).ok()
    } else {
        None
    };
    let last = state.as_ref().map(|value| value.last_backup_at_ms);
    let reminder_due = match last {
        None => true,
        Some(at) => now_ms()? - at > REMINDER_MS,
    };
    let journal = read_journal(roots)?;
    let pending_restore = journal
        .as_ref()
        .is_some_and(|value| value.state == "replacing");
    let pending_preview = journal
        .as_ref()
        .is_some_and(|value| value.state == "preview");
    let checked_preview = journal.and_then(|value| {
        (value.state == "preview").then_some(value.preview)
    });
    let default = {
        let mut path = default_path(&roots.documents);
        let mut suffix = 2;
        while path.exists() && suffix < 1000 {
            path = roots.documents.join("E-Rechnung Sicherungen").join(format!(
                "E-Rechnung-{} ({suffix}).erechnung",
                chrono_like_date()
            ));
            suffix += 1;
        }
        path
    };
    Ok(BackupStatus {
        last_backup_at_ms: last,
        last_path: state.as_ref().map(|value| value.last_path.clone()),
        same_volume: state.as_ref().is_some_and(|value| value.same_volume),
        reminder_due,
        pending_restore,
        pending_preview,
        checked_preview,
        default_path: default.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub(crate) fn backup_status(app: AppHandle) -> Result<BackupStatus, String> {
    status_to(&Roots::from_app(&app)?)
}

#[tauri::command]
pub(crate) fn backup_create(app: AppHandle, destination: String, password: String) -> Result<BackupResult, String> {
    let _lock = crate::guard::exclusive();
    create_to(&Roots::from_app(&app)?, Path::new(&destination), &password)
}

#[tauri::command]
pub(crate) fn backup_preview(
    app: AppHandle,
    source: String,
    password: String,
) -> Result<RestorePreview, String> {
    let _lock = crate::guard::exclusive();
    preview_to(&Roots::from_app(&app)?, Path::new(&source), &password)
}

#[tauri::command]
pub(crate) fn backup_confirm(app: AppHandle) -> Result<RestorePreview, String> {
    let _lock = crate::guard::exclusive();
    confirm_to(&Roots::from_app(&app)?)
}

#[tauri::command]
pub(crate) fn backup_resume(app: AppHandle) -> Result<RestorePreview, String> {
    let _lock = crate::guard::exclusive();
    resume_to(&Roots::from_app(&app)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{set_signing_to, test_archive_request};
    use crate::workspace::{import_to, read_from, save_to};

    fn sandbox() -> (PathBuf, Roots) {
        let root = std::env::temp_dir().join(format!("erechnung-backup-{}", Uuid::new_v4()));
        let roots = Roots {
            documents: root.join("documents"),
            local: root.join("local"),
            roaming: root.join("roaming"),
        };
        fs::create_dir_all(&roots.documents).unwrap();
        fs::create_dir_all(&roots.local).unwrap();
        fs::create_dir_all(&roots.roaming).unwrap();
        (root, roots)
    }

    fn snapshot() -> String {
        let extraction = serde_json::json!({"pages":[{}],"fields":{},"lines":[],"lineItems":[],"warnings":[]});
        serde_json::json!({"schemaVersion":1,"extractionVersion":"text-layout-v1","draft":{"invoiceNumber":"RE-1"},"initialDraft":{},"sourceSelections":{},
            "pendingSourceFields":[],"completed":false,"sourceExtraction":extraction,"extraction":extraction}).to_string()
    }

    fn password() -> &'static str {
        "Testkennwort-12"
    }

    #[test]
    fn roundtrip_restores_archive_draft_and_key() {
        let (root, roots) = sandbox();
        let paths = roots.archive();
        set_signing_to(&paths, true).unwrap();
        archive::save_and_archive_to(&paths, &roots.documents, test_archive_request("RE-1001", "xrechnung"))
            .unwrap();
        let doc = import_to(&roots.workspace(), "eingang.pdf", b"%PDF-a", None, None).unwrap();
        save_to(&roots.workspace(), &doc.id, 0, &snapshot()).unwrap();
        fs::write(roots.learning(), r#"{"schemaVersion":2,"activeProfileId":"p","profiles":[]}"#).unwrap();
        let dest = roots.documents.join("E-Rechnung Sicherungen").join("test.erechnung");
        let created = create_to(&roots, &dest, password()).unwrap();
        assert!(created.archive_entries >= 1);
        assert!(dest.exists());
        let empty = Roots {
            documents: root.join("empty/documents"),
            local: root.join("empty/local"),
            roaming: root.join("empty/roaming"),
        };
        fs::create_dir_all(&empty.documents).unwrap();
        fs::create_dir_all(&empty.local).unwrap();
        fs::create_dir_all(&empty.roaming).unwrap();
        preview_to(&empty, &dest, password()).unwrap();
        confirm_to(&empty).unwrap();
        let restored = empty.archive();
        let report = archive::verify_archive_to(&restored).unwrap();
        assert!(report.valid);
        assert_eq!(report.entry_count, created.archive_entries);
        let reopened = read_from(&empty.workspace(), &doc.id).unwrap();
        assert_eq!(reopened.document.revision, 1);
        assert!(protect::load_key(&restored.signing_key).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_wrong_password_and_tampered_package() {
        let (root, roots) = sandbox();
        archive::save_and_archive_to(
            &roots.archive(),
            &roots.documents,
            test_archive_request("RE-2", "xrechnung"),
        )
        .unwrap();
        let dest = root.join("ok.erechnung");
        create_to(&roots, &dest, password()).unwrap();
        let err = preview_to(&roots, &dest, "falsches-passwort-xx")
            .err()
            .unwrap();
        assert!(
            err.contains("Kennwort") || err.contains("beschädigt"),
            "{err}"
        );
        let mut bytes = fs::read(&dest).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x5a;
        let tampered = root.join("bad.erechnung");
        fs::write(&tampered, bytes).unwrap();
        assert!(preview_to(&roots, &tampered, password()).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_path_traversal_in_pack() {
        let (root, roots) = sandbox();
        let work = root.join("work");
        fs::create_dir_all(&work).unwrap();
        let dest = root.join("pack.bin");
        let mut file = File::create(&dest).unwrap();
        file.write_all(PACK_MAGIC).unwrap();
        write_entry(&mut file, "../escape.bin", b"nope").unwrap();
        drop(file);
        let staging = root.join("staging");
        fs::create_dir_all(&staging).unwrap();
        assert!(
            unpack(&dest, &staging)
                .err()
                .unwrap()
                .contains("unzulässig")
        );
        let _ = roots;
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keeps_previous_data_after_restore() {
        let (root, roots) = sandbox();
        archive::save_and_archive_to(
            &roots.archive(),
            &roots.documents,
            test_archive_request("OLD", "xrechnung"),
        )
        .unwrap();
        let dest = root.join("one.erechnung");
        create_to(&roots, &dest, password()).unwrap();
        archive::save_and_archive_to(
            &roots.archive(),
            &roots.documents,
            test_archive_request("NEW", "xrechnung"),
        )
        .unwrap();
        preview_to(&roots, &dest, password()).unwrap();
        confirm_to(&roots).unwrap();
        let replaced = fs::read_dir(roots.replaced()).unwrap().next().unwrap().unwrap().path();
        assert!(replaced.join("E-Rechnungsarchiv").exists());
        fs::remove_dir_all(root).unwrap();
    }

    fn invoice_numbers(database: &Path) -> Vec<String> {
        let db = Connection::open(database).unwrap();
        let mut statement = db
            .prepare("SELECT invoice_number FROM archive_entries ORDER BY invoice_number")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn resume_keeps_work_written_after_interrupted_restore() {
        let (root, roots) = sandbox();
        archive::save_and_archive_to(
            &roots.archive(),
            &roots.documents,
            test_archive_request("RE-OLD", "xrechnung"),
        )
        .unwrap();
        let dest = root.join("keep.erechnung");
        create_to(&roots, &dest, password()).unwrap();
        preview_to(&roots, &dest, password()).unwrap();
        let mut journal = read_journal(&roots).unwrap().unwrap();
        journal.state = "replacing".into();
        let replaced = roots.replaced().join(&journal.id);
        journal.replaced = Some(replaced.to_string_lossy().into_owned());
        write_journal(&roots, &journal).unwrap();
        fs::create_dir_all(&replaced).unwrap();
        move_dir(&roots.archive().root, &replaced.join("E-Rechnungsarchiv")).unwrap();
        fs::create_dir_all(roots.workspace()).unwrap();
        move_dir(&roots.workspace(), &replaced.join("workspace")).unwrap();
        archive::save_and_archive_to(
            &roots.archive(),
            &roots.documents,
            test_archive_request("RE-NEW", "xrechnung"),
        )
        .unwrap();
        let doc = import_to(&roots.workspace(), "neu.pdf", b"%PDF-n", None, None).unwrap();
        save_to(&roots.workspace(), &doc.id, 0, &snapshot()).unwrap();
        resume_to(&roots).unwrap();
        assert_eq!(
            invoice_numbers(&roots.archive().database),
            vec!["RE-OLD".to_string()]
        );
        assert_eq!(
            invoice_numbers(
                &replaced
                    .join("E-Rechnungsarchiv-nach-unterbrechung")
                    .join("archiv.sqlite3")
            ),
            vec!["RE-NEW".to_string()]
        );
        assert!(replaced.join("workspace-nach-unterbrechung").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resumes_interrupted_restore() {
        let (root, roots) = sandbox();
        archive::save_and_archive_to(
            &roots.archive(),
            &roots.documents,
            test_archive_request("RE-R", "xrechnung"),
        )
        .unwrap();
        let dest = root.join("resume.erechnung");
        create_to(&roots, &dest, password()).unwrap();
        let empty = Roots {
            documents: root.join("e/documents"),
            local: root.join("e/local"),
            roaming: root.join("e/roaming"),
        };
        fs::create_dir_all(&empty.documents).unwrap();
        fs::create_dir_all(&empty.local).unwrap();
        fs::create_dir_all(&empty.roaming).unwrap();
        preview_to(&empty, &dest, password()).unwrap();
        let mut journal = read_journal(&empty).unwrap().unwrap();
        journal.state = "replacing".into();
        journal.replaced = Some(empty.replaced().join(&journal.id).to_string_lossy().into_owned());
        write_journal(&empty, &journal).unwrap();
        resume_to(&empty).unwrap();
        assert!(archive::verify_archive_to(&empty.archive()).unwrap().valid);
        assert!(!PathBuf::from(&journal.staging).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preview_is_not_interrupted_and_confirm_wipes_plaintext_staging() {
        let (root, roots) = sandbox();
        set_signing_to(&roots.archive(), true).unwrap();
        archive::save_and_archive_to(
            &roots.archive(),
            &roots.documents,
            test_archive_request("RE-P", "xrechnung"),
        )
        .unwrap();
        let dest = root.join("preview.erechnung");
        create_to(&roots, &dest, password()).unwrap();
        let empty = Roots {
            documents: root.join("p/documents"),
            local: root.join("p/local"),
            roaming: root.join("p/roaming"),
        };
        fs::create_dir_all(&empty.documents).unwrap();
        fs::create_dir_all(&empty.local).unwrap();
        fs::create_dir_all(&empty.roaming).unwrap();
        preview_to(&empty, &dest, password()).unwrap();
        let status = status_to(&empty).unwrap();
        assert!(!status.pending_restore);
        assert!(status.pending_preview);
        assert!(status.checked_preview.is_some());
        let staging = PathBuf::from(&read_journal(&empty).unwrap().unwrap().staging);
        assert!(staging.join("tree/keys/archive-signing-key.bin").exists());
        confirm_to(&empty).unwrap();
        assert!(!staging.exists());
        let done = status_to(&empty).unwrap();
        assert!(!done.pending_restore);
        assert!(!done.pending_preview);
        fs::remove_dir_all(root).unwrap();
    }
}
