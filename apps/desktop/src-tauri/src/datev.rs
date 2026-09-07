//! Local handoff files, not a DATEV network integration or accounting engine.
use base64::Engine;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;
use uuid::Uuid;

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn root(_app: &AppHandle) -> Result<PathBuf, String> {
    Ok(super::paths::documents()?
        .join("E-Rechnungsarchiv")
        .join("Steuerkanzlei"))
}
fn database(root: &Path) -> Result<Connection, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let db = Connection::open(root.join("exporte.sqlite3")).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
        .map_err(|e| e.to_string())?;
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version > 1 {
        return Err("Die Kanzleiexporte benötigen eine neuere Programmversion.".into());
    }
    if version == 0 {
        db.execute_batch("BEGIN IMMEDIATE;
      CREATE TABLE exports(id TEXT PRIMARY KEY,request_hash TEXT NOT NULL,created_at_ms INTEGER NOT NULL,state TEXT NOT NULL CHECK(state IN ('pending','complete')),manifest TEXT NOT NULL,error TEXT);
      CREATE TABLE export_files(export_id TEXT NOT NULL REFERENCES exports(id),path TEXT NOT NULL,sha256 TEXT NOT NULL,payload BLOB,PRIMARY KEY(export_id,path));
      CREATE TABLE export_invoices(export_id TEXT NOT NULL REFERENCES exports(id),archive_id TEXT NOT NULL,document_id TEXT NOT NULL,content_hash TEXT NOT NULL,original_hash TEXT NOT NULL,business_key TEXT NOT NULL,PRIMARY KEY(export_id,archive_id));
      CREATE INDEX invoice_document ON export_invoices(document_id); CREATE INDEX invoice_original ON export_invoices(original_hash); CREATE INDEX invoice_business ON export_invoices(business_key);
      PRAGMA user_version=1; COMMIT;").map_err(|e|e.to_string())?;
    }
    Ok(db)
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportFileRequest {
    contents_base64: String,
    date_from: String,
    date_to: String,
    gross: String,
    booking_count: u32,
    archive_ids: Vec<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentFileRequest {
    archive_id: String,
    guid: String,
    pdf_name: String,
    xml_name: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentPackageRequest {
    xml: String,
    files: Vec<DocumentFileRequest>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportRequest {
    id: String,
    profile: String,
    files: Vec<ExportFileRequest>,
    repeat_reason: String,
    document_package: DocumentPackageRequest,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportSummary {
    id: String,
    created_at_ms: i64,
    state: String,
    manifest: String,
    error: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct ExportPage {
    entries: Vec<ExportSummary>,
    total: i64,
}
struct Artifact {
    path: String,
    bytes: Vec<u8>,
}
struct Reference {
    archive_id: String,
    document_id: String,
    content_hash: String,
    original_hash: String,
    business_key: String,
}
fn business_key(snapshot: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let invoice = &value["invoice"];
    let normalize = |v: &Value| {
        v.as_str()
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_uppercase()
    };
    Ok(hash(
        json!([
            normalize(&invoice["seller"]["name"]),
            normalize(&invoice["seller"]["vatId"]),
            normalize(&invoice["invoiceNumber"])
        ])
        .to_string()
        .as_bytes(),
    ))
}
fn reference(source: &super::archive::DatevSource) -> Result<Reference, String> {
    Ok(Reference {
        archive_id: source.archive_id.clone(),
        document_id: source.document_id.clone(),
        content_hash: source.content_hash.clone(),
        original_hash: source.original_hash.clone(),
        business_key: business_key(&source.snapshot)?,
    })
}
fn duplicate(db: &Connection, r: &Reference) -> Result<bool, String> {
    db.query_row("SELECT EXISTS(SELECT 1 FROM export_invoices WHERE document_id=?1 OR original_hash=?2 OR business_key=?3)",params![r.document_id,r.original_hash,r.business_key],|r|r.get(0)).map_err(|e|e.to_string())
}
fn load(db: &Connection, id: &str) -> Result<ExportSummary, String> {
    db.query_row(
        "SELECT id,created_at_ms,state,manifest,error FROM exports WHERE id=?1",
        [id],
        |r| {
            Ok(ExportSummary {
                id: r.get(0)?,
                created_at_ms: r.get(1)?,
                state: r.get(2)?,
                manifest: r.get(3)?,
                error: r.get(4)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

// Prepared operations are durable BEFORE handoff files exist. Retain byte
// payloads until completion, then keep only the immutable files and metadata.
fn prepare(
    root: &Path,
    id: &str,
    request_hash: &str,
    manifest: &str,
    artifacts: &[Artifact],
    references: &[Reference],
    repeat_reason: &str,
) -> Result<(), String> {
    Uuid::parse_str(id).map_err(|_| "Ungültige Exportkennung.".to_string())?;
    let mut db = database(root)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let previous: Option<String> = tx
        .query_row("SELECT request_hash FROM exports WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(previous) = previous {
        return if previous == request_hash {
            Ok(())
        } else {
            Err("Diese Exportkennung gehört zu anderen Daten.".into())
        };
    }
    let mut documents = std::collections::HashSet::new();
    let mut business_keys = std::collections::HashSet::new();
    for r in references {
        if !documents.insert(&r.document_id) || !business_keys.insert(&r.business_key) {
            return Err("Dieselbe Rechnung oder Rechnungsnummer wurde mehrfach ausgewählt.".into());
        }
        if duplicate(&tx, r)? && repeat_reason.trim().chars().count() < 10 {
            return Err("Diese Rechnung oder eine mögliche Kopie wurde bereits für die Kanzlei ausgegeben. Bitte den erneuten Export ausdrücklich begründen; ein doppelter Import kann doppelte Buchungen erzeugen.".into());
        }
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64;
    tx.execute("INSERT INTO exports(id,request_hash,created_at_ms,state,manifest) VALUES(?1,?2,?3,'pending',?4)",params![id,request_hash,now,manifest]).map_err(|e|e.to_string())?;
    for a in artifacts {
        tx.execute(
            "INSERT INTO export_files(export_id,path,sha256,payload) VALUES(?1,?2,?3,?4)",
            params![id, a.path, hash(&a.bytes), a.bytes],
        )
        .map_err(|e| e.to_string())?;
    }
    for r in references {
        tx.execute("INSERT INTO export_invoices(export_id,archive_id,document_id,content_hash,original_hash,business_key) VALUES(?1,?2,?3,?4,?5,?6)",params![id,r.archive_id,r.document_id,r.content_hash,r.original_hash,r.business_key]).map_err(|e|e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffffffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb88320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
fn push_u16(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_le_bytes());
}
fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_le_bytes());
}
fn zip_store(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    if files.is_empty() {
        return Err("Das Belegpaket enthält keine Dateien.".into());
    }
    let mut local = Vec::new();
    let mut central = Vec::new();
    for (name, bytes) in files {
        if name.len() > 80
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || name.contains("..")
        {
            return Err("Ungültiger Dateiname im Belegpaket.".into());
        }
        let offset = local.len() as u32;
        let crc = crc32(bytes);
        let size = bytes.len() as u32;
        local.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        push_u16(&mut local, 20);
        push_u16(&mut local, 0);
        push_u16(&mut local, 0);
        push_u16(&mut local, 0);
        push_u16(&mut local, 0);
        push_u32(&mut local, crc);
        push_u32(&mut local, size);
        push_u32(&mut local, size);
        push_u16(&mut local, name.len() as u16);
        push_u16(&mut local, 0);
        local.extend_from_slice(name.as_bytes());
        local.extend_from_slice(bytes);
        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        push_u16(&mut central, 20);
        push_u16(&mut central, 20);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, crc);
        push_u32(&mut central, size);
        push_u32(&mut central, size);
        push_u16(&mut central, name.len() as u16);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, 0);
        push_u32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let mut zip = local;
    let central_offset = zip.len() as u32;
    zip.extend_from_slice(&central);
    zip.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    push_u16(&mut zip, 0);
    push_u16(&mut zip, 0);
    push_u16(&mut zip, files.len() as u16);
    push_u16(&mut zip, files.len() as u16);
    push_u32(&mut zip, central.len() as u32);
    push_u32(&mut zip, central_offset);
    push_u16(&mut zip, 0);
    Ok(zip)
}
fn guid_file(guid: &str, extension: &str) -> Result<String, String> {
    if guid.len() != 36
        || guid.as_bytes()[14] != b'5'
        || !guid.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err("Ungültige Belegkennung.".into());
    }
    Ok(format!("{guid}.{extension}"))
}
fn readme() -> Vec<u8> {
    "Paket für die Steuerkanzlei\r\n\r\nDieses Paket wurde lokal erstellt. Es wurde nichts an DATEV übertragen.\r\n\r\nImportreihenfolge:\r\n1. Belege.zip mit DATEV Belegtransfer hochladen. Die ZIP-Datei nicht entpacken.\r\n2. Danach die EXTF-Dateien als Buchungsstapel in DATEV Rechnungswesen importieren.\r\n\r\nDie Spalte Beleglink verweist auf dieselbe GUID wie document.xml im ZIP. Nur so hängen Buchung und Belegbild zusammen.\r\nDer Importstatus in DATEV ist dieser Anwendung nicht bekannt.\r\n".as_bytes().to_vec()
}
fn artifact_path(root: &Path, id: &str, name: &str) -> Result<PathBuf, String> {
    Uuid::parse_str(id).map_err(|_| "Ungültige Exportkennung.".to_string())?;
    if name.is_empty()
        || name.len() > 150
        || name.split('/').any(|part| {
            part.is_empty()
                || part.starts_with('.')
                || part.ends_with('.')
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
        || Path::new(name)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Ungültiger Exportpfad.".into());
    }
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    // Check each existing ancestor before creating any child (including junctions).
    let child = |parent: &Path, name: &str| -> Result<PathBuf, String> {
        let next = parent.join(name);
        if !next.exists() {
            fs::create_dir(&next).map_err(|e| e.to_string())?;
        }
        let resolved = next.canonicalize().map_err(|e| e.to_string())?;
        if !resolved.starts_with(parent) {
            return Err("Exportordner verweist außerhalb der Ablage.".into());
        }
        Ok(resolved)
    };
    let directory = child(&child(&canonical_root, "Dateien")?, id)?;
    let parts: Vec<_> = name.split('/').collect();
    let mut parent = directory.clone();
    for part in &parts[..parts.len() - 1] {
        parent = child(&parent, part)?;
    }
    let path = directory.join(name);
    if path.exists()
        && !path
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&directory)
    {
        return Err("Exportdatei verweist außerhalb der Ablage.".into());
    }
    Ok(path)
}
fn finish(root: &Path, id: &str) -> Result<ExportSummary, String> {
    let mut db = database(root)?;
    let summary = load(&db, id)?;
    let operation = (|| -> Result<(), String> {
        let files = {
            let mut q = db
                .prepare(
                    "SELECT path,sha256,payload FROM export_files WHERE export_id=?1 ORDER BY path",
                )
                .map_err(|e| e.to_string())?;
            q.query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
        };
        if files.is_empty() {
            return Err("Die Exportdateien fehlen im Vorgangsprotokoll.".into());
        }
        if !files.iter().any(|(name, expected, _)| {
            name == "Begleitinformationen.json" && *expected == hash(summary.manifest.as_bytes())
        }) {
            return Err("Die Begleitinformationen passen nicht zum vorbereiteten Export.".into());
        }
        for (name, expected, payload) in files {
            let path = artifact_path(root, id, &name)?;
            if path.exists() {
                if hash(&fs::read(&path).map_err(|e| e.to_string())?) != expected {
                    return Err(format!(
                        "Die Exportdatei {name} wurde verändert. Sie wird nicht überschrieben."
                    ));
                }
            } else {
                let bytes=payload.ok_or_else(||format!("Die fertige Exportdatei {name} fehlt. Bitte aus einer Sicherung wiederherstellen."))?;
                if hash(&bytes) != expected {
                    return Err("Der vorbereitete Export wurde verändert.".into());
                }
                let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
                let mut f = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temporary)
                    .map_err(|e| e.to_string())?;
                f.write_all(&bytes)
                    .and_then(|_| f.sync_all())
                    .map_err(|e| e.to_string())?;
                drop(f);
                if path.exists() {
                    return Err(
                        "Die Exportdatei wurde zwischenzeitlich angelegt. Bitte erneut prüfen."
                            .into(),
                    );
                }
                fs::rename(&temporary, &path).map_err(|e| e.to_string())?;
            }
        }
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE exports SET state='complete',error=NULL WHERE id=?1",
            [id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE export_files SET payload=NULL WHERE export_id=?1",
            [id],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    })();
    if let Err(e) = operation {
        let _ = db.execute("UPDATE exports SET error=?2 WHERE id=?1", params![id, e]);
        return Err(e);
    }
    load(&db, id)
}
#[tauri::command]
pub(crate) fn datev_get_profile(app: AppHandle) -> Result<Option<String>, String> {
    super::workspace::datev_profile(&app, None)
}
#[tauri::command]
pub(crate) fn datev_save_profile(app: AppHandle, contents: String) -> Result<(), String> {
    super::workspace::datev_profile(&app, Some(&contents)).map(|_| ())
}
#[tauri::command]
pub(crate) fn datev_export_status(
    app: AppHandle,
    document_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    if document_ids.len() > 200 {
        return Err("Zu viele Rechnungen für eine Abfrage.".into());
    }
    let db = database(&root(&app)?)?;
    document_ids
        .into_iter()
        .filter_map(|id| {
            match db.query_row(
                "SELECT EXISTS(SELECT 1 FROM export_invoices WHERE document_id=?1)",
                [&id],
                |r| r.get::<_, bool>(0),
            ) {
                Ok(true) => Some(Ok(id)),
                Ok(false) => None,
                Err(e) => Some(Err(e.to_string())),
            }
        })
        .collect()
}
#[tauri::command]
pub(crate) fn datev_check_duplicates(
    app: AppHandle,
    archive_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    if archive_ids.len() > 100 {
        return Err("Bitte höchstens 100 Rechnungen auswählen.".into());
    }
    let db = database(&root(&app)?)?;
    let mut matches = Vec::new();
    for id in archive_ids {
        if duplicate(
            &db,
            &reference(&super::archive::datev_evidence(&app, &id)?)?,
        )? {
            matches.push(id);
        }
    }
    Ok(matches)
}
#[tauri::command]
pub(crate) fn datev_create_export(
    app: AppHandle,
    request: ExportRequest,
) -> Result<ExportSummary, String> {
    if request.files.is_empty() || request.files.len() > 2 || request.repeat_reason.len() > 2000 {
        return Err("Ungültiger Exportumfang.".into());
    }
    let root = root(&app)?;
    let request_hash = hash(&serde_json::to_vec(&request).map_err(|e| e.to_string())?);
    let db = database(&root)?;
    if let Some(previous) = db
        .query_row(
            "SELECT request_hash FROM exports WHERE id=?1",
            [&request.id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        if previous != request_hash {
            return Err("Die Exportkennung gehört zu anderen Daten.".into());
        }
        return finish(&root, &request.id);
    }
    if super::workspace::datev_profile(&app, None)?.as_deref() != Some(request.profile.as_str()) {
        return Err("Die Kanzleiangaben wurden geändert. Bitte die Vorschau erneuern.".into());
    }
    let profile: Value = serde_json::from_str(&request.profile).map_err(|e| e.to_string())?;
    if profile["schemaVersion"] != 1 || profile["confirmed"] != true {
        return Err("Die Kanzleiangaben müssen bestätigt sein.".into());
    }
    let mut references = Vec::new();
    let mut artifacts = Vec::new();
    let mut file_info = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for (index, file) in request.files.iter().enumerate() {
        if file.contents_base64.len() > 14 * 1024 * 1024
            || file.archive_ids.is_empty()
            || file.archive_ids.len() > 100
        {
            return Err("Der Export ist zu groß oder leer.".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&file.contents_base64)
            .map_err(|e| e.to_string())?;
        if !bytes.starts_with(b"\"EXTF\";700;21;\"Buchungsstapel\";13;")
            || !bytes.ends_with(b"\r\n")
            || !bytes.windows(5).any(|w| w == b"BEDI ")
        {
            return Err("Unbekanntes DATEV-Dateiformat.".into());
        }
        let name = format!("EXTF_{}_{}.csv", request.id, index + 1);
        file_info.push(json!({"name":name,"sha256":hash(&bytes),"dateFrom":file.date_from,"dateTo":file.date_to,"gross":file.gross,"bookingCount":file.booking_count,"archiveIds":file.archive_ids}));
        artifacts.push(Artifact { path: name, bytes });
        for id in &file.archive_ids {
            if !ids.insert(id.clone()) {
                return Err("Eine Rechnung wurde mehreren Stapeln zugeordnet.".into());
            }
            references.push(reference(&super::archive::datev_evidence(&app, id)?)?);
            if ids.len() > 100 {
                return Err("Der Export überschreitet 100 Rechnungen oder 128 MB. Bitte eine kleinere Auswahl verwenden.".into());
            }
        }
    }
    let package = &request.document_package;
    if package.xml.len() > 256 * 1024
        || package.files.len() != ids.len()
        || !package
            .xml
            .starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>")
        || !package
            .xml
            .contains("http://xml.datev.de/bedi/tps/document/v06.0")
    {
        return Err("Das Belegpaket ist unvollständig oder hat ein unbekanntes Format.".into());
    }
    let mut zip_files = vec![("document.xml".into(), package.xml.as_bytes().to_vec())];
    let mut package_ids = std::collections::HashSet::new();
    for file in &package.files {
        if !ids.contains(&file.archive_id) || !package_ids.insert(file.archive_id.clone()) {
            return Err("Das Belegpaket passt nicht zu den ausgewählten Rechnungen.".into());
        }
        let pdf_name = guid_file(&file.guid, "pdf")?;
        let xml_name = guid_file(&file.guid, "xml")?;
        if file.pdf_name != pdf_name
            || file.xml_name != xml_name
            || !package.xml.contains(&file.guid)
            || !package.xml.contains(&pdf_name)
            || !package.xml.contains(&xml_name)
        {
            return Err("Belegkennung und Dateinamen im Paket stimmen nicht überein.".into());
        }
        let (pdf, xml) = super::archive::datev_attachments(&app, &file.archive_id)?;
        zip_files.push((pdf_name, pdf));
        zip_files.push((xml_name, xml));
    }
    let zip = zip_store(&zip_files)?;
    let readme = readme();
    file_info.push(json!({"name":"Belege.zip","sha256":hash(&zip),"documentCount":package.files.len()}));
    file_info.push(json!({"name":"LiesMich.txt","sha256":hash(&readme)}));
    artifacts.push(Artifact {
        path: "Belege.zip".into(),
        bytes: zip,
    });
    artifacts.push(Artifact {
        path: "LiesMich.txt".into(),
        bytes: readme,
    });
    if artifacts.iter().map(|a| a.bytes.len()).sum::<usize>() > 128 * 1024 * 1024 {
        return Err("Der Export überschreitet 100 Rechnungen oder 128 MB. Bitte eine kleinere Auswahl verwenden.".into());
    }
    let manifest=json!({"schemaVersion":2,"id":request.id,"format":"EXTF-700-21-13","encoding":"windows-1252","timestampConvention":"UTC","documentPackage":{"format":"DATEV-XML-document-v06","file":"Belege.zip","importOrder":["Belege.zip","EXTF"]},"profile":profile,"profileSha256":hash(request.profile.as_bytes()),"repeatReason":request.repeat_reason,"externalImportStatus":"unknown","validation":"Lokale Prüfung; DATEV-Testimport durch die Steuerkanzlei ausstehend","files":file_info,
      "invoices":references.iter().map(|r|json!({"archiveId":r.archive_id,"documentId":r.document_id,"contentHash":r.content_hash,"originalHash":r.original_hash})).collect::<Vec<_>>()}).to_string();
    artifacts.push(Artifact {
        path: "Begleitinformationen.json".into(),
        bytes: manifest.as_bytes().to_vec(),
    });
    prepare(
        &root,
        &request.id,
        &request_hash,
        &manifest,
        &artifacts,
        &references,
        &request.repeat_reason,
    )?;
    finish(&root, &request.id)
}
#[tauri::command]
pub(crate) fn datev_list_exports(
    app: AppHandle,
    offset: Option<i64>,
) -> Result<ExportPage, String> {
    let db = database(&root(&app)?)?;
    let total = db
        .query_row("SELECT COUNT(*) FROM exports", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let mut q=db.prepare("SELECT id,created_at_ms,state,manifest,error FROM exports ORDER BY created_at_ms DESC,id LIMIT 100 OFFSET ?1").map_err(|e|e.to_string())?;
    let entries = q
        .query_map([offset.unwrap_or(0).max(0)], |r| {
            Ok(ExportSummary {
                id: r.get(0)?,
                created_at_ms: r.get(1)?,
                state: r.get(2)?,
                manifest: r.get(3)?,
                error: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ExportPage { entries, total })
}
#[tauri::command]
pub(crate) fn datev_resume_export(app: AppHandle, id: String) -> Result<ExportSummary, String> {
    finish(&root(&app)?, &id)
}
#[tauri::command]
pub(crate) fn datev_open_export(app: AppHandle, id: String) -> Result<(), String> {
    let root = root(&app)?;
    finish(&root, &id)?;
    super::archive::open_native(&root.join("Dateien").join(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("erechnung-datev-test-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn artifacts() -> Vec<Artifact> {
        vec![
            Artifact {
                path: "Begleitinformationen.json".into(),
                bytes: b"{}".to_vec(),
            },
            Artifact {
                path: "EXTF_test.csv".into(),
                bytes: b"immutable bytes\r\n".to_vec(),
            },
        ]
    }
    fn invoice() -> Reference {
        Reference {
            archive_id: "archive".into(),
            document_id: "document".into(),
            content_hash: "content".into(),
            original_hash: "original".into(),
            business_key: "business".into(),
        }
    }
    #[test]
    fn resumes_prepared_export_and_reuses_identical_completed_files() {
        let s = Sandbox::new();
        let id = Uuid::new_v4().to_string();
        prepare(&s.0, &id, "request", "{}", &artifacts(), &[invoice()], "").unwrap();
        assert_eq!(
            load(&database(&s.0).unwrap(), &id).unwrap().state,
            "pending"
        );
        // Simulate a crash after one file has already reached disk.
        fs::write(
            artifact_path(&s.0, &id, "Begleitinformationen.json").unwrap(),
            b"{}",
        )
        .unwrap();
        assert_eq!(finish(&s.0, &id).unwrap().state, "complete");
        let path = artifact_path(&s.0, &id, "EXTF_test.csv").unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        prepare(&s.0, &id, "request", "{}", &artifacts(), &[invoice()], "").unwrap();
        finish(&s.0, &id).unwrap();
        assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), modified);
        let count: i64 = database(&s.0)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM export_files WHERE payload IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        assert!(prepare(&s.0, &id, "changed", "{}", &artifacts(), &[], "").is_err());
    }
    #[test]
    fn reserves_pending_invoices_and_requires_explicit_repeat_reason() {
        let s = Sandbox::new();
        let id = Uuid::new_v4().to_string();
        prepare(&s.0, &id, "first", "{}", &artifacts(), &[invoice()], "").unwrap();
        let second = Uuid::new_v4().to_string();
        assert!(
            prepare(
                &s.0,
                &second,
                "second",
                "{}",
                &artifacts(),
                &[invoice()],
                ""
            )
            .is_err()
        );
        prepare(
            &s.0,
            &second,
            "second",
            "{}",
            &artifacts(),
            &[invoice()],
            "Bewusst erneut nach Rücksprache",
        )
        .unwrap();
        let mut copy = invoice();
        copy.archive_id = "copy".into();
        assert!(
            prepare(
                &s.0,
                &Uuid::new_v4().to_string(),
                "third",
                "{}",
                &artifacts(),
                &[invoice(), copy],
                "Wiederholung bestätigt"
            )
            .is_err()
        );
    }
    #[test]
    fn preserves_tampered_files_and_pending_payload_for_recovery() {
        let s = Sandbox::new();
        let id = Uuid::new_v4().to_string();
        prepare(&s.0, &id, "request", "{}", &artifacts(), &[], "").unwrap();
        let path = artifact_path(&s.0, &id, "EXTF_test.csv").unwrap();
        fs::write(&path, b"changed").unwrap();
        assert!(
            finish(&s.0, &id)
                .unwrap_err()
                .contains("nicht überschrieben")
        );
        assert_eq!(fs::read(&path).unwrap(), b"changed");
        let db = database(&s.0).unwrap();
        let summary = load(&db, &id).unwrap();
        assert_eq!(summary.state, "pending");
        assert!(summary.error.is_some());
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM export_files WHERE payload IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }
    #[test]
    fn detects_database_manifest_or_payload_changes() {
        let s = Sandbox::new();
        let id = Uuid::new_v4().to_string();
        prepare(&s.0, &id, "request", "{}", &artifacts(), &[], "").unwrap();
        let db = database(&s.0).unwrap();
        db.execute("UPDATE exports SET manifest='changed'", [])
            .unwrap();
        assert!(
            finish(&s.0, &id)
                .unwrap_err()
                .contains("Begleitinformationen")
        );
        db.execute("UPDATE exports SET manifest='{}'", []).unwrap();
        db.execute(
            "UPDATE export_files SET payload=X'01' WHERE path='EXTF_test.csv'",
            [],
        )
        .unwrap();
        assert!(
            finish(&s.0, &id)
                .unwrap_err()
                .contains("vorbereitete Export wurde verändert")
        );
    }
    #[test]
    fn does_not_reconstruct_missing_completed_files() {
        let s = Sandbox::new();
        let id = Uuid::new_v4().to_string();
        prepare(&s.0, &id, "request", "{}", &artifacts(), &[], "").unwrap();
        finish(&s.0, &id).unwrap();
        fs::remove_file(artifact_path(&s.0, &id, "EXTF_test.csv").unwrap()).unwrap();
        assert!(finish(&s.0, &id).unwrap_err().contains("Sicherung"));
    }
    #[test]
    fn rejects_traversal_absolute_paths_and_windows_alternate_streams() {
        let s = Sandbox::new();
        let id = Uuid::new_v4().to_string();
        for path in [
            "../escape",
            "/absolute",
            "C:/escape",
            "file:stream",
            "Belege/../escape",
            "file.",
            "Belege//file",
            "",
        ] {
            assert!(artifact_path(&s.0, &id, path).is_err(), "{path}");
        }
        assert!(artifact_path(&s.0, "../escape", "file").is_err());
        assert!(
            artifact_path(&s.0, &id, "Belege/test.pdf")
                .unwrap()
                .starts_with(s.0.canonicalize().unwrap())
        );
    }
    fn zip_names(bytes: &[u8]) -> Vec<String> {
        let mut names = Vec::new();
        let mut i = 0;
        while i + 30 <= bytes.len() && bytes[i..i + 4] == [0x50, 0x4b, 0x03, 0x04] {
            let name_len = u16::from_le_bytes(bytes[i + 26..i + 28].try_into().unwrap()) as usize;
            let extra = u16::from_le_bytes(bytes[i + 28..i + 30].try_into().unwrap()) as usize;
            let size = u32::from_le_bytes(bytes[i + 18..i + 22].try_into().unwrap()) as usize;
            names.push(String::from_utf8(bytes[i + 30..i + 30 + name_len].to_vec()).unwrap());
            i += 30 + name_len + extra + size;
        }
        names
    }
    #[test]
    fn packs_document_xml_and_named_files_into_an_uncompressed_zip() {
        let zip = zip_store(&[
            ("document.xml".into(), b"<archive/>".to_vec()),
            (
                "AAAAAAAA-BBBB-5CCC-ADDD-EEEEEEEEEEEE.pdf".into(),
                b"%PDF".to_vec(),
            ),
        ])
        .unwrap();
        assert_eq!(&zip[..2], &[0x50, 0x4b]);
        assert_eq!(
            zip_names(&zip),
            [
                "document.xml",
                "AAAAAAAA-BBBB-5CCC-ADDD-EEEEEEEEEEEE.pdf"
            ]
        );
        assert!(zip_store(&[("../x".into(), vec![])]).is_err());
    }
}
