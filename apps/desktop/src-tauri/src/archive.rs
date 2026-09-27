use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use rusqlite::{
    Connection, OptionalExtension, TransactionBehavior, params, params_from_iter, types::Value,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 3;
const MAX_PDF_BYTES: usize = 120 * 1024 * 1024;
const MAX_XML_BYTES: usize = 20 * 1024 * 1024;
const EMPTY_HEAD: &str = "";

#[derive(Clone)]
pub(crate) struct ArchivePaths {
    pub(crate) root: PathBuf,
    pub(crate) database: PathBuf,
    reports: PathBuf,
    pub(crate) signing_key: PathBuf,
}

impl ArchivePaths {
    pub(crate) fn new(documents: &Path, app_data: &Path) -> Self {
        let root = documents.join("E-Rechnungsarchiv");
        Self {
            database: root.join("archiv.sqlite3"),
            reports: root.join("Prüfberichte"),
            root,
            signing_key: app_data.join("archive-signing-key-v1.bin"),
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveMetadata {
    invoice_number: String,
    issue_date: String,
    seller_name: String,
    buyer_name: String,
    gross_amount: String,
    currency: String,
    source_file_name: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveAndArchiveRequest {
    format: String,
    output_file_name: String,
    pdf_contents_base64: String,
    xml_contents: String,
    metadata: ArchiveMetadata,
    #[serde(default)]
    evidence: Option<InvoiceEvidence>,
    #[serde(default)]
    ticket_id: Option<String>,
    #[serde(default, skip)]
    validation_json: Option<String>,
    #[serde(default, skip)]
    report_xml: Option<String>,
}

#[cfg(test)]
pub(crate) fn test_archive_request(invoice_number: &str, format: &str) -> SaveAndArchiveRequest {
    let xml = if format == "xrechnung" {
        format!("<?xml version=\"1.0\"?><Invoice id=\"{invoice_number}\"></Invoice>")
    } else {
        format!(
            "<?xml version=\"1.0\"?><rsm:CrossIndustryInvoice id=\"{invoice_number}\"></rsm:CrossIndustryInvoice>"
        )
    };
    let pdf = if format == "zugferd" {
        b"%PDF-1.7\nfactur-x.xml\n/Alternative\n%%EOF".as_slice()
    } else {
        b"%PDF-1.7\n%%EOF".as_slice()
    };
    SaveAndArchiveRequest {
        evidence: None,
        ticket_id: None,
        validation_json: None,
        report_xml: None,
        format: format.to_string(),
        output_file_name: format!(
            "{invoice_number}.{}",
            if format == "xrechnung" { "xml" } else { "pdf" }
        ),
        pdf_contents_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, pdf),
        xml_contents: xml,
        metadata: ArchiveMetadata {
            invoice_number: invoice_number.to_string(),
            issue_date: "2026-09-02".to_string(),
            seller_name: "Musterbetrieb GmbH".to_string(),
            buyer_name: "Beispielkunde AG".to_string(),
            gross_amount: "119.00".to_string(),
            currency: "EUR".to_string(),
            source_file_name: "quelle.pdf".to_string(),
        },
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InvoiceEvidence {
    schema_version: u32,
    document_id: String,
    source_revision: i64,
    snapshot: String,
    #[serde(default)]
    original_hash: String,
    #[serde(default)]
    content_hash: String,
    #[serde(default)]
    hybrid_confirmed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatevSource {
    pub(crate) archive_id: String,
    pub(crate) document_id: String,
    pub(crate) content_hash: String,
    pub(crate) original_hash: String,
    pub(crate) snapshot: String,
    pub(crate) xml: String,
    pub(crate) format: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveEntrySummary {
    id: String,
    sequence: i64,
    created_at_ms: i64,
    invoice_number: String,
    issue_date: String,
    seller_name: String,
    buyer_name: String,
    gross_amount: String,
    currency: String,
    format: String,
    signed: bool,
    document_id: Option<String>,
    content_hash: Option<String>,
    independently_checked: bool,
    rule_version: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveEntryDetail {
    id: String,
    sequence: i64,
    created_at_ms: i64,
    invoice_number: String,
    issue_date: String,
    seller_name: String,
    buyer_name: String,
    gross_amount: String,
    currency: String,
    format: String,
    source_file_name: String,
    pdf_path: String,
    xml_path: String,
    pdf_sha256: String,
    xml_sha256: String,
    previous_chain_hash: String,
    chain_hash: String,
    signed: bool,
    signing_key_id: Option<String>,
    independently_checked: bool,
    rule_version: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveAndArchiveResult {
    output_path: String,
    archive_entry: ArchiveEntryDetail,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveQuery {
    search: Option<String>,
    format: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    signature: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveListResult {
    entries: Vec<ArchiveEntrySummary>,
    total: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveStatus {
    root_path: String,
    entry_count: i64,
    signed_count: i64,
    signing_enabled: bool,
    signing_key_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveVerificationIssue {
    sequence: Option<i64>,
    invoice_number: Option<String>,
    pub(crate) message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveVerificationReport {
    pub(crate) valid: bool,
    checked_at_ms: i64,
    checked_at_display: String,
    pub(crate) entry_count: i64,
    file_count: i64,
    pub(crate) signed_count: i64,
    unsigned_count: i64,
    pub(crate) issues: Vec<ArchiveVerificationIssue>,
    report_path: String,
}

#[derive(Clone)]
struct ArchiveRow {
    id: String,
    sequence: i64,
    created_at_ms: i64,
    invoice_number: String,
    issue_date: String,
    seller_name: String,
    buyer_name: String,
    gross_amount: String,
    currency: String,
    format: String,
    source_file_name: String,
    pdf_path: String,
    xml_path: String,
    pdf_sha256: String,
    xml_sha256: String,
    previous_chain_hash: String,
    chain_hash: String,
    signature: Option<Vec<u8>>,
    signing_key_id: Option<String>,
    evidence_json: Option<String>,
    validation_json: Option<String>,
}

fn archive_paths(app: &AppHandle) -> Result<ArchivePaths, String> {
    let documents = super::paths::documents()?;
    let app_data = super::paths::app_data(app, false)?;
    Ok(ArchivePaths::new(&documents, &app_data))
}

fn now_ms() -> Result<i64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    i64::try_from(millis).map_err(|_| "Systemzeit ist außerhalb des gültigen Bereichs.".to_string())
}

fn open_database(paths: &ArchivePaths) -> Result<Connection, String> {
    fs::create_dir_all(&paths.root)
        .map_err(|error| format!("Archivordner konnte nicht erstellt werden: {error}"))?;
    let connection = Connection::open(&paths.database)
        .map_err(|error| format!("Archivdatenbank konnte nicht geöffnet werden: {error}"))?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;",
        )
        .map_err(|error| format!("Archivdatenbank konnte nicht vorbereitet werden: {error}"))?;
    migrate_database(&connection)?;
    Ok(connection)
}

fn migrate_database(connection: &Connection) -> Result<(), String> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if version > SCHEMA_VERSION {
        return Err("Das Archiv wurde mit einer neueren Programmversion erstellt.".to_string());
    }
    if version == 0 {
        connection
            .execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE TABLE archive_entries (
                   sequence INTEGER PRIMARY KEY,
                   id TEXT NOT NULL UNIQUE,
                   created_at_ms INTEGER NOT NULL,
                   invoice_number TEXT NOT NULL,
                   issue_date TEXT NOT NULL,
                   seller_name TEXT NOT NULL,
                   buyer_name TEXT NOT NULL,
                   gross_amount TEXT NOT NULL,
                   currency TEXT NOT NULL,
                   format TEXT NOT NULL CHECK(format IN ('xrechnung', 'zugferd')),
                   source_file_name TEXT NOT NULL,
                   pdf_path TEXT NOT NULL,
                   xml_path TEXT NOT NULL,
                   pdf_sha256 TEXT NOT NULL,
                   xml_sha256 TEXT NOT NULL,
                   previous_chain_hash TEXT NOT NULL,
                   chain_hash TEXT NOT NULL UNIQUE,
                   signature BLOB,
                   signing_key_id TEXT
                 );
                 CREATE TABLE archive_keys (
                   id TEXT PRIMARY KEY,
                   public_key BLOB NOT NULL,
                   created_at_ms INTEGER NOT NULL
                 );
                 CREATE TABLE archive_settings (
                   key TEXT PRIMARY KEY,
                   value TEXT NOT NULL
                 );
                 CREATE TABLE archive_state (
                   singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                   entry_count INTEGER NOT NULL,
                   head_hash TEXT NOT NULL
                 );
                 INSERT INTO archive_settings(key, value) VALUES('signing_enabled', '0');
                 INSERT INTO archive_state(singleton, entry_count, head_hash) VALUES(1, 0, '');
                 CREATE INDEX archive_entries_issue_date ON archive_entries(issue_date DESC);
                 CREATE INDEX archive_entries_invoice_number ON archive_entries(invoice_number);
                 CREATE INDEX archive_entries_format ON archive_entries(format);
                 PRAGMA user_version = 1;
                 COMMIT;",
            )
            .map_err(|error| {
                format!("Archivdatenbank konnte nicht eingerichtet werden: {error}")
            })?;
    }
    if version < 2 {
        if !table_has_column(connection, "archive_entries", "evidence_json")? {
            connection
                .execute_batch("BEGIN IMMEDIATE; ALTER TABLE archive_entries ADD COLUMN evidence_json TEXT; COMMIT;")
                .map_err(|error| format!("Das Archiv konnte nicht erweitert werden: {error}"))?;
        }
        connection
            .execute_batch("PRAGMA user_version=2;")
            .map_err(|error| error.to_string())?;
    }
    if version < 3 {
        if !table_has_column(connection, "archive_entries", "validation_json")? {
            connection
                .execute_batch("BEGIN IMMEDIATE; ALTER TABLE archive_entries ADD COLUMN validation_json TEXT; COMMIT;")
                .map_err(|error| format!("Das Archiv konnte nicht um Prüfnachweise erweitert werden: {error}"))?;
        }
        connection
            .execute_batch("PRAGMA user_version=3;")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn table_has_column(connection: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let mut rows = statement.query([]).map_err(|error| error.to_string())?;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let name: String = row.get(1).map_err(|error| error.to_string())?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_plain_field(
    name: &str,
    value: &str,
    maximum_chars: usize,
    required: bool,
) -> Result<(), String> {
    let length = value.chars().count();
    if (required && value.trim().is_empty())
        || length > maximum_chars
        || value.contains(['\0', '\r', '\n'])
    {
        return Err(format!("Die Archivangabe „{name}“ ist ungültig."));
    }
    Ok(())
}

fn validate_request(request: &SaveAndArchiveRequest, pdf: &[u8]) -> Result<(), String> {
    if !matches!(request.format.as_str(), "xrechnung" | "zugferd") {
        return Err("Unbekanntes Rechnungsformat für das Archiv.".to_string());
    }
    if pdf.len() > MAX_PDF_BYTES || !pdf.starts_with(b"%PDF-") {
        return Err("Die PDF-Rechnung ist beschädigt oder zu groß.".to_string());
    }
    if request.xml_contents.len() > MAX_XML_BYTES
        || !request.xml_contents.trim_start().starts_with("<?xml")
    {
        return Err("Die Rechnungsdaten sind beschädigt oder zu groß.".to_string());
    }
    if request.format == "xrechnung" && !request.xml_contents.contains("<Invoice ") {
        return Err("Die Datei für Behörden ist unvollständig.".to_string());
    }
    if request.format == "zugferd"
        && (!request.xml_contents.contains("CrossIndustryInvoice")
            || !contains_bytes(pdf, b"factur-x.xml")
            || !contains_bytes(pdf, b"/Alternative"))
    {
        return Err("Die PDF-Rechnung enthält nicht alle benötigten Rechnungsdaten.".to_string());
    }
    let required_extension = if request.format == "xrechnung" {
        "xml"
    } else {
        "pdf"
    };
    validate_file_name(&request.output_file_name, required_extension)?;
    validate_plain_field(
        "Rechnungsnummer",
        &request.metadata.invoice_number,
        200,
        true,
    )?;
    validate_plain_field("Rechnungsdatum", &request.metadata.issue_date, 10, true)?;
    validate_plain_field("Absender", &request.metadata.seller_name, 300, true)?;
    validate_plain_field("Empfänger", &request.metadata.buyer_name, 300, true)?;
    validate_plain_field("Rechnungsbetrag", &request.metadata.gross_amount, 80, true)?;
    validate_plain_field("Währung", &request.metadata.currency, 3, true)?;
    validate_plain_field("Quelldatei", &request.metadata.source_file_name, 255, false)?;
    if !is_iso_date(&request.metadata.issue_date) {
        return Err("Das Rechnungsdatum ist für das Archiv ungültig.".to_string());
    }
    if let Some(evidence) = &request.evidence {
        Uuid::parse_str(&evidence.document_id)
            .map_err(|_| "Ungültiger Rechnungsbezug.".to_string())?;
        if evidence.schema_version != 1
            || evidence.source_revision < 1
            || evidence.snapshot.len() > MAX_XML_BYTES
            || evidence.original_hash.len() != 64
            || evidence.content_hash != sha256_hex(evidence.snapshot.as_bytes())
        {
            return Err("Der bestätigte Rechnungsstand ist unvollständig.".into());
        }
        let value: serde_json::Value = serde_json::from_str(&evidence.snapshot)
            .map_err(|_| "Ungültiger Rechnungsstand.".to_string())?;
        let invoice = &value["invoice"];
        if value["schemaVersion"] != 1
            || value["serializerVersion"] != "ubl-cii-v1"
            || invoice["invoiceNumber"] != request.metadata.invoice_number
            || invoice["issueDate"] != request.metadata.issue_date
            || invoice["seller"]["name"] != request.metadata.seller_name
            || invoice["buyer"]["name"] != request.metadata.buyer_name
            || invoice["totals"]["payable"] != request.metadata.gross_amount
            || invoice["currency"] != request.metadata.currency
        {
            return Err(
                "Archivangaben und bestätigter Rechnungsstand stimmen nicht überein.".into(),
            );
        }
        if !evidence.hybrid_confirmed {
            return Err("Bitte bestätigen Sie, dass die Angaben die Originalrechnung korrekt wiedergeben.".into());
        }
    }
    Ok(())
}

fn validate_file_name(file_name: &str, required_extension: &str) -> Result<(), String> {
    let candidate = Path::new(file_name);
    if candidate.file_name().and_then(|value| value.to_str()) != Some(file_name)
        || matches!(file_name, "" | "." | "..")
        || candidate
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case(required_extension))
    {
        return Err("Ungültiger Dateiname oder Dateityp.".to_string());
    }
    Ok(())
}

fn is_iso_date(value: &str) -> bool {
    if value.len() != 10
        || !value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 4 | 7) {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
    {
        return false;
    }
    let Ok(year) = value[0..4].parse::<u32>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = value[8..10].parse::<u32>() else {
        return false;
    };
    if month == 0 || month > 12 || day == 0 {
        return false;
    }
    let leap_year =
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days_in_month = [
        31,
        if leap_year { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    day <= days_in_month[(month - 1) as usize]
}

fn contains_bytes(contents: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && contents
            .windows(needle.len())
            .any(|window| window == needle)
}

fn sha256_hex(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}

fn push_hash_component(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn calculate_chain_hash(row: &ArchiveRow) -> String {
    let mut hasher = Sha256::new();
    let domain = if row.validation_json.is_some() {
        "e-rechnungsarchiv-chain-v3"
    } else if row.evidence_json.is_some() {
        "e-rechnungsarchiv-chain-v2"
    } else {
        "e-rechnungsarchiv-chain-v1"
    };
    for value in [
        domain.to_string(),
        row.sequence.to_string(),
        row.id.clone(),
        row.created_at_ms.to_string(),
        row.invoice_number.clone(),
        row.issue_date.clone(),
        row.seller_name.clone(),
        row.buyer_name.clone(),
        row.gross_amount.clone(),
        row.currency.clone(),
        row.format.clone(),
        row.source_file_name.clone(),
        row.pdf_path.clone(),
        row.xml_path.clone(),
        row.pdf_sha256.clone(),
        row.xml_sha256.clone(),
        row.previous_chain_hash.clone(),
        row.signing_key_id.clone().unwrap_or_default(),
    ] {
        push_hash_component(&mut hasher, value.as_bytes());
    }
    if let Some(evidence) = &row.evidence_json {
        push_hash_component(&mut hasher, sha256_hex(evidence.as_bytes()).as_bytes());
    }
    if let Some(validation) = &row.validation_json {
        push_hash_component(&mut hasher, sha256_hex(validation.as_bytes()).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn relative_archive_path(issue_date: &str, id: &str, file_name: &str) -> String {
    format!(
        "Belege/{}/{}/{id}/{file_name}",
        &issue_date[0..4],
        &issue_date[5..7]
    )
}

fn checked_relative_path(root: &Path, value: &str) -> Result<PathBuf, String> {
    let relative = Path::new(value);
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("Ungültiger Dateipfad im Archiv.".to_string());
    }
    Ok(root.join(relative))
}

fn write_new_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| "Zielordner konnte nicht ermittelt werden.".to_string())?;
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let temporary = directory.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("archive"),
        Uuid::new_v4()
    ));
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(contents)
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        if path.exists() {
            return Err(
                "Eine Archivdatei mit derselben Kennung ist bereits vorhanden.".to_string(),
            );
        }
        fs::rename(&temporary, path).map_err(|error| error.to_string())?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}

fn available_output_path(documents: &Path, file_name: &str) -> Result<PathBuf, String> {
    let directory = documents.join("E-Rechnung Ausgaben");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let candidate = Path::new(file_name);
    let stem = candidate
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Ungültiger Dateiname.".to_string())?;
    let extension = candidate
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Ungültiger Dateityp.".to_string())?;
    for suffix in 1..=10_000 {
        let name = if suffix == 1 {
            file_name.to_string()
        } else {
            format!("{stem} ({suffix}).{extension}")
        };
        let path = directory.join(name);
        if !path.exists() {
            return Ok(path);
        }
    }
    Err("Für die Ausgabe konnte kein freier Dateiname gefunden werden.".to_string())
}

fn write_private_key(path: &Path, bytes: &[u8; 32]) -> Result<(), String> {
    crate::protect::store_key(path, bytes)
}

fn load_or_create_signing_key(path: &Path) -> Result<SigningKey, String> {
    if path.exists() {
        let secret = crate::protect::load_key(path)?;
        return Ok(SigningKey::from_bytes(&secret));
    }
    let key = SigningKey::generate(&mut OsRng);
    write_private_key(path, &key.to_bytes())?;
    Ok(key)
}

fn key_id(key: &SigningKey) -> String {
    sha256_hex(&key.verifying_key().to_bytes())
}

fn signing_enabled(connection: &Connection) -> Result<bool, String> {
    let value: String = connection
        .query_row(
            "SELECT value FROM archive_settings WHERE key = 'signing_enabled'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    Ok(value == "1")
}

pub(crate) fn save_and_archive_to(
    paths: &ArchivePaths,
    documents: &Path,
    request: SaveAndArchiveRequest,
) -> Result<SaveAndArchiveResult, String> {
    if request.pdf_contents_base64.len() > 164 * 1024 * 1024 {
        return Err("Die kodierte PDF-Rechnung ist zu groß.".to_string());
    }
    let pdf = base64::engine::general_purpose::STANDARD
        .decode(&request.pdf_contents_base64)
        .map_err(|error| format!("PDF-Rechnung ist nicht gültig kodiert: {error}"))?;
    validate_request(&request, &pdf)?;
    let mut connection = open_database(paths)?;
    let use_signature = signing_enabled(&connection)?;
    let signing_key = use_signature
        .then(|| load_or_create_signing_key(&paths.signing_key))
        .transpose()?;
    let output_path = available_output_path(documents, &request.output_file_name)?;
    let id = Uuid::new_v4().to_string();
    let pdf_relative = relative_archive_path(&request.metadata.issue_date, &id, "rechnung.pdf");
    let xml_relative =
        relative_archive_path(&request.metadata.issue_date, &id, "rechnungsdaten.xml");
    let pdf_path = checked_relative_path(&paths.root, &pdf_relative)?;
    let xml_path = checked_relative_path(&paths.root, &xml_relative)?;
    let archive_directory = pdf_path
        .parent()
        .ok_or_else(|| "Archivordner konnte nicht ermittelt werden.".to_string())?
        .to_path_buf();
    let mut output_created = false;

    let result = (|| -> Result<SaveAndArchiveResult, String> {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Archiv konnte nicht gesperrt werden: {error}"))?;
        let (sequence, previous_chain_hash): (i64, String) = transaction
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1,
                        COALESCE((SELECT chain_hash FROM archive_entries ORDER BY sequence DESC LIMIT 1), '')
                 FROM archive_entries",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| error.to_string())?;
        let created_at_ms = now_ms()?;
        let signing_key_id = signing_key.as_ref().map(key_id);
        let mut row = ArchiveRow {
            id: id.clone(),
            sequence,
            created_at_ms,
            invoice_number: request.metadata.invoice_number.trim().to_string(),
            issue_date: request.metadata.issue_date.clone(),
            seller_name: request.metadata.seller_name.trim().to_string(),
            buyer_name: request.metadata.buyer_name.trim().to_string(),
            gross_amount: request.metadata.gross_amount.clone(),
            currency: request.metadata.currency.to_uppercase(),
            format: request.format.clone(),
            source_file_name: request.metadata.source_file_name.clone(),
            pdf_path: pdf_relative.clone(),
            xml_path: xml_relative.clone(),
            pdf_sha256: sha256_hex(&pdf),
            xml_sha256: sha256_hex(request.xml_contents.as_bytes()),
            previous_chain_hash,
            chain_hash: String::new(),
            signature: None,
            signing_key_id,
            evidence_json: request
                .evidence
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|e| e.to_string())?,
            validation_json: None,
        };
        let report_relative = relative_archive_path(&request.metadata.issue_date, &id, "pruefbericht.xml");
        if let Some(report_xml) = &request.report_xml {
            if report_xml.len() > MAX_XML_BYTES || request.validation_json.is_none() {
                return Err("Der Prüfnachweis ist unvollständig.".into());
            }
            let mut proof: serde_json::Value = serde_json::from_str(request.validation_json.as_ref().unwrap())
                .map_err(|_| "Der Prüfnachweis ist beschädigt.".to_string())?;
            if proof["schemaVersion"] != 1 || proof["status"] != "passed" {
                return Err("Ohne erfolgreiche unabhängige Prüfung kann keine fertige E-Rechnung erzeugt werden.".into());
            }
            if proof["xmlSha256"] != row.xml_sha256 || proof["pdfSha256"] != row.pdf_sha256 {
                return Err("Prüfergebnis und Rechnungsdatei passen nicht zusammen.".into());
            }
            if proof["reportSha256"] != sha256_hex(report_xml.as_bytes()) {
                return Err("Der Prüfbericht passt nicht zum gespeicherten Nachweis.".into());
            }
            proof["reportPath"] = report_relative.clone().into();
            row.validation_json = Some(proof.to_string());
        }
        row.chain_hash = calculate_chain_hash(&row);
        row.signature = signing_key
            .as_ref()
            .map(|key| key.sign(row.chain_hash.as_bytes()).to_bytes().to_vec());

        write_new_file(&pdf_path, &pdf)?;
        write_new_file(&xml_path, request.xml_contents.as_bytes())?;
        if let Some(report_xml) = &request.report_xml {
            write_new_file(&checked_relative_path(&paths.root, &report_relative)?, report_xml.as_bytes())?;
        }
        let output_contents = if request.format == "xrechnung" {
            request.xml_contents.as_bytes()
        } else {
            &pdf
        };
        write_new_file(&output_path, output_contents)?;
        output_created = true;

        if let (Some(key), Some(identifier)) = (&signing_key, &row.signing_key_id) {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO archive_keys(id, public_key, created_at_ms) VALUES(?1, ?2, ?3)",
                    params![identifier, key.verifying_key().to_bytes().as_slice(), created_at_ms],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction
            .execute(
                "INSERT INTO archive_entries(
                   sequence, id, created_at_ms, invoice_number, issue_date, seller_name, buyer_name,
                   gross_amount, currency, format, source_file_name, pdf_path, xml_path, pdf_sha256,
                   xml_sha256, previous_chain_hash, chain_hash, signature, signing_key_id, evidence_json, validation_json
                 ) VALUES(
                   ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
                 )",
                params![
                    row.sequence,
                    row.id,
                    row.created_at_ms,
                    row.invoice_number,
                    row.issue_date,
                    row.seller_name,
                    row.buyer_name,
                    row.gross_amount,
                    row.currency,
                    row.format,
                    row.source_file_name,
                    row.pdf_path,
                    row.xml_path,
                    row.pdf_sha256,
                    row.xml_sha256,
                    row.previous_chain_hash,
                    row.chain_hash,
                    row.signature,
                    row.signing_key_id,
                    row.evidence_json,
                    row.validation_json,
                ],
            )
            .map_err(|error| format!("Archiveintrag konnte nicht gespeichert werden: {error}"))?;
        transaction
            .execute(
                "UPDATE archive_state SET entry_count = ?1, head_hash = ?2 WHERE singleton = 1",
                params![row.sequence, row.chain_hash],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .commit()
            .map_err(|error| format!("Archiv konnte nicht abgeschlossen werden: {error}"))?;

        Ok(SaveAndArchiveResult {
            output_path: output_path.to_string_lossy().into_owned(),
            archive_entry: detail_from_row(&row),
        })
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&archive_directory);
        if output_created {
            let _ = fs::remove_file(&output_path);
        }
    }
    result
}

fn row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArchiveRow> {
    Ok(ArchiveRow {
        sequence: row.get(0)?,
        id: row.get(1)?,
        created_at_ms: row.get(2)?,
        invoice_number: row.get(3)?,
        issue_date: row.get(4)?,
        seller_name: row.get(5)?,
        buyer_name: row.get(6)?,
        gross_amount: row.get(7)?,
        currency: row.get(8)?,
        format: row.get(9)?,
        source_file_name: row.get(10)?,
        pdf_path: row.get(11)?,
        xml_path: row.get(12)?,
        pdf_sha256: row.get(13)?,
        xml_sha256: row.get(14)?,
        previous_chain_hash: row.get(15)?,
        chain_hash: row.get(16)?,
        signature: row.get(17)?,
        signing_key_id: row.get(18)?,
        evidence_json: row.get(19)?,
        validation_json: row.get(20)?,
    })
}

const ROW_COLUMNS: &str = "sequence, id, created_at_ms, invoice_number, issue_date, seller_name,
  buyer_name, gross_amount, currency, format, source_file_name, pdf_path, xml_path, pdf_sha256,
  xml_sha256, previous_chain_hash, chain_hash, signature, signing_key_id, evidence_json, validation_json";

fn independently_checked(row: &ArchiveRow) -> bool {
    row.validation_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .is_some_and(|value| value["status"] == "passed")
}

fn rule_version_of(row: &ArchiveRow) -> Option<String> {
    row.validation_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .and_then(|value| value["ruleVersion"].as_str().map(str::to_string))
}

fn report_path_of(row: &ArchiveRow) -> Option<String> {
    row.validation_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .and_then(|value| value["reportPath"].as_str().map(str::to_string))
}

fn detail_from_row(row: &ArchiveRow) -> ArchiveEntryDetail {
    ArchiveEntryDetail {
        id: row.id.clone(),
        sequence: row.sequence,
        created_at_ms: row.created_at_ms,
        invoice_number: row.invoice_number.clone(),
        issue_date: row.issue_date.clone(),
        seller_name: row.seller_name.clone(),
        buyer_name: row.buyer_name.clone(),
        gross_amount: row.gross_amount.clone(),
        currency: row.currency.clone(),
        format: row.format.clone(),
        source_file_name: row.source_file_name.clone(),
        pdf_path: row.pdf_path.clone(),
        xml_path: row.xml_path.clone(),
        pdf_sha256: row.pdf_sha256.clone(),
        xml_sha256: row.xml_sha256.clone(),
        previous_chain_hash: row.previous_chain_hash.clone(),
        chain_hash: row.chain_hash.clone(),
        signed: row.signature.is_some(),
        signing_key_id: row.signing_key_id.clone(),
        independently_checked: independently_checked(row),
        rule_version: rule_version_of(row),
    }
}

fn load_row(connection: &Connection, id: &str) -> Result<ArchiveRow, String> {
    Uuid::parse_str(id).map_err(|_| "Ungültige Archivkennung.".to_string())?;
    connection
        .query_row(
            &format!("SELECT {ROW_COLUMNS} FROM archive_entries WHERE id = ?1"),
            [id],
            row_from_sql,
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Der Archiveintrag wurde nicht gefunden.".to_string())
}

fn verified_datev_source(paths: &ArchivePaths, id: &str) -> Result<DatevSource, String> {
    let db = open_database(paths)?;
    let row = load_row(&db, id)?;
    if calculate_chain_hash(&row) != row.chain_hash {
        return Err("Die Archivangaben wurden verändert. Bitte das Archiv prüfen.".into());
    }
    let evidence: InvoiceEvidence = serde_json::from_str(row.evidence_json.as_deref().ok_or_else(|| "Für diese ältere Ausgabe fehlt der vollständige Rechnungsstand. Bitte die Originalrechnung prüfen und erneut als E-Rechnung speichern.".to_string())?).map_err(|_| "Der Rechnungsstand ist beschädigt.".to_string())?;
    if evidence.schema_version != 1
        || sha256_hex(evidence.snapshot.as_bytes()) != evidence.content_hash
    {
        return Err("Der Rechnungsstand wurde verändert.".into());
    }
    Uuid::parse_str(&evidence.document_id).map_err(|_| "Ungültiger Rechnungsbezug.".to_string())?;
    let pdf = read_checked_file(&paths.root, &row.pdf_path)?;
    let xml = read_checked_file(&paths.root, &row.xml_path)?;
    if sha256_hex(&pdf) != row.pdf_sha256 || sha256_hex(&xml) != row.xml_sha256 {
        return Err("Eine archivierte Rechnungsdatei wurde verändert.".into());
    }
    match (&row.signature, &row.signing_key_id) {
        (Some(signature), Some(identifier)) => {
            let key: Vec<u8> = db
                .query_row(
                    "SELECT public_key FROM archive_keys WHERE id=?1",
                    [identifier],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            if sha256_hex(&key) != *identifier {
                return Err("Der Archivschlüssel wurde verändert.".into());
            }
            let key = VerifyingKey::from_bytes(
                &<[u8; 32]>::try_from(key)
                    .map_err(|_| "Ungültiger Archivschlüssel.".to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let signature = Signature::from_slice(signature).map_err(|e| e.to_string())?;
            key.verify(row.chain_hash.as_bytes(), &signature)
                .map_err(|_| "Die Archivbestätigung ist ungültig.".to_string())?;
        }
        (None, None) => {}
        _ => return Err("Die Archivbestätigung ist unvollständig.".into()),
    }
    Ok(DatevSource {
        archive_id: row.id,
        document_id: evidence.document_id,
        content_hash: evidence.content_hash,
        original_hash: evidence.original_hash,
        snapshot: evidence.snapshot,
        xml: String::from_utf8(xml).map_err(|e| e.to_string())?,
        format: row.format,
    })
}

#[tauri::command]
pub(crate) fn archive_datev_source(app: AppHandle, id: String) -> Result<DatevSource, String> {
    verified_datev_source(&archive_paths(&app)?, &id)
}

pub(crate) fn datev_evidence(app: &AppHandle, id: &str) -> Result<DatevSource, String> {
    verified_datev_source(&archive_paths(app)?, id)
}

pub(crate) fn datev_attachments(app: &AppHandle, id: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    let paths = archive_paths(app)?;
    verified_datev_source(&paths, id)?;
    let row = load_row(&open_database(&paths)?, id)?;
    Ok((
        read_checked_file(&paths.root, &row.pdf_path)?,
        read_checked_file(&paths.root, &row.xml_path)?,
    ))
}

fn list_entries_to(paths: &ArchivePaths, query: ArchiveQuery) -> Result<ArchiveListResult, String> {
    let connection = open_database(paths)?;
    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    if !(1..=200).contains(&limit) || offset < 0 {
        return Err("Ungültige Seitengröße für das Archiv.".to_string());
    }
    let format = query.format.unwrap_or_default();
    if !matches!(format.as_str(), "" | "xrechnung" | "zugferd") {
        return Err("Ungültiger Archivfilter.".to_string());
    }
    let signature = query.signature.unwrap_or_default();
    if !matches!(signature.as_str(), "" | "signed" | "unsigned") {
        return Err("Ungültiger Schutzfilter.".to_string());
    }
    let date_from = query.date_from.unwrap_or_default();
    let date_to = query.date_to.unwrap_or_default();
    if (!date_from.is_empty() && !is_iso_date(&date_from))
        || (!date_to.is_empty() && !is_iso_date(&date_to))
    {
        return Err("Ungültiger Datumsfilter.".to_string());
    }
    let search = query
        .search
        .unwrap_or_default()
        .trim()
        .chars()
        .take(200)
        .collect::<String>();
    let escaped = search
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{escaped}%");
    let filter_values = [
        Value::Text(search),
        Value::Text(pattern),
        Value::Text(format),
        Value::Text(date_from),
        Value::Text(date_to),
        Value::Text(signature),
    ];
    let filters = "FROM archive_entries
               WHERE (?1 = '' OR invoice_number LIKE ?2 ESCAPE '\\' OR seller_name LIKE ?2 ESCAPE '\\'
                      OR buyer_name LIKE ?2 ESCAPE '\\' OR source_file_name LIKE ?2 ESCAPE '\\')
                 AND (?3 = '' OR format = ?3)
                 AND (?4 = '' OR issue_date >= ?4)
                 AND (?5 = '' OR issue_date <= ?5)
                 AND (?6 = '' OR (?6 = 'signed' AND signing_key_id IS NOT NULL)
                      OR (?6 = 'unsigned' AND signing_key_id IS NULL))";
    let total = connection
        .query_row(
            &format!("SELECT COUNT(*) {filters}"),
            params_from_iter(filter_values.iter()),
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let sql = format!(
        "SELECT id, sequence, created_at_ms, invoice_number, issue_date, seller_name,
                      buyer_name, gross_amount, currency, format, signing_key_id IS NOT NULL,
                      CASE WHEN json_valid(evidence_json) THEN json_extract(evidence_json,'$.documentId') END,
                      CASE WHEN json_valid(evidence_json) THEN json_extract(evidence_json,'$.contentHash') END,
                      CASE WHEN json_valid(validation_json) AND json_extract(validation_json,'$.status')='passed' THEN 1 ELSE 0 END,
                      CASE WHEN json_valid(validation_json) THEN json_extract(validation_json,'$.ruleVersion') END
               {filters}
               ORDER BY sequence DESC LIMIT ?7 OFFSET ?8"
    );
    let values = filter_values
        .into_iter()
        .chain([Value::Integer(limit), Value::Integer(offset)])
        .collect::<Vec<_>>();
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok(ArchiveEntrySummary {
                id: row.get(0)?,
                sequence: row.get(1)?,
                created_at_ms: row.get(2)?,
                invoice_number: row.get(3)?,
                issue_date: row.get(4)?,
                seller_name: row.get(5)?,
                buyer_name: row.get(6)?,
                gross_amount: row.get(7)?,
                currency: row.get(8)?,
                format: row.get(9)?,
                signed: row.get(10)?,
                document_id: row.get(11)?,
                content_hash: row.get(12)?,
                independently_checked: row.get::<_, i64>(13)? == 1,
                rule_version: row.get(14)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let entries = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(ArchiveListResult { entries, total })
}

fn archive_status_to(paths: &ArchivePaths) -> Result<ArchiveStatus, String> {
    let connection = open_database(paths)?;
    let (entry_count, signed_count): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(signing_key_id IS NOT NULL), 0) FROM archive_entries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let enabled = signing_enabled(&connection)?;
    let signing_key_id = if paths.signing_key.exists() {
        Some(key_id(&load_or_create_signing_key(&paths.signing_key)?))
    } else {
        None
    };
    Ok(ArchiveStatus {
        root_path: paths.root.to_string_lossy().into_owned(),
        entry_count,
        signed_count,
        signing_enabled: enabled,
        signing_key_id,
    })
}

pub(crate) fn set_signing_to(paths: &ArchivePaths, enabled: bool) -> Result<ArchiveStatus, String> {
    let connection = open_database(paths)?;
    if enabled {
        let key = load_or_create_signing_key(&paths.signing_key)?;
        connection
            .execute(
                "INSERT OR IGNORE INTO archive_keys(id, public_key, created_at_ms) VALUES(?1, ?2, ?3)",
                params![key_id(&key), key.verifying_key().to_bytes().as_slice(), now_ms()?],
            )
            .map_err(|error| error.to_string())?;
    }
    connection
        .execute(
            "UPDATE archive_settings SET value = ?1 WHERE key = 'signing_enabled'",
            [if enabled { "1" } else { "0" }],
        )
        .map_err(|error| error.to_string())?;
    archive_status_to(paths)
}

fn verification_issue(
    row: Option<&ArchiveRow>,
    message: impl Into<String>,
) -> ArchiveVerificationIssue {
    ArchiveVerificationIssue {
        sequence: row.map(|value| value.sequence),
        invoice_number: row.map(|value| value.invoice_number.clone()),
        message: message.into(),
    }
}

fn read_checked_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = checked_relative_path(root, relative)?;
    let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
    let canonical_file = path
        .canonicalize()
        .map_err(|_| format!("Datei fehlt: {}", path.to_string_lossy()))?;
    if !canonical_file.starts_with(canonical_root) {
        return Err("Archivdatei verweist außerhalb des Archivs.".to_string());
    }
    fs::read(canonical_file).map_err(|error| error.to_string())
}

fn collect_archive_files(root: &Path) -> Result<HashSet<String>, String> {
    fn visit(root: &Path, directory: &Path, files: &mut HashSet<String>) -> Result<(), String> {
        if !directory.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            let path = entry.path();
            if file_type.is_dir() && !file_type.is_symlink() {
                visit(root, &path, files)?;
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "Eine Datei liegt außerhalb des Belegarchivs.".to_string())?;
            let normalized = relative
                .components()
                .filter_map(|component| match component {
                    Component::Normal(value) => Some(value.to_string_lossy()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            files.insert(normalized);
            if files.len() > 100_000 {
                return Err("Das Archiv enthält unerwartet viele Dateien.".to_string());
            }
        }
        Ok(())
    }

    let mut files = HashSet::new();
    visit(root, &root.join("Belege"), &mut files)?;
    Ok(files)
}

fn render_report(report: &ArchiveVerificationReport) -> String {
    let mut text = format!(
        "Prüfbericht E-Rechnungsarchiv\n\nErgebnis: {}\nGeprüfte Einträge: {}\nGeprüfte Dateien: {}\nZusätzlich geschützte Einträge: {}\nNicht zusätzlich geschützte Einträge: {}\nPrüfzeitpunkt: {}\n",
        if report.valid {
            "Alles in Ordnung"
        } else {
            "Abweichungen gefunden"
        },
        report.entry_count,
        report.file_count,
        report.signed_count,
        report.unsigned_count,
        report.checked_at_display
    );
    if report.issues.is_empty() {
        text.push_str(
            "\nEs wurden keine fehlenden oder veränderten Archivbestandteile gefunden.\n",
        );
    } else {
        text.push_str("\nFestgestellte Abweichungen:\n");
        for issue in &report.issues {
            let reference = match (issue.sequence, issue.invoice_number.as_deref()) {
                (Some(sequence), Some(number)) => format!("Eintrag {sequence} ({number})"),
                (Some(sequence), None) => format!("Eintrag {sequence}"),
                _ => "Archiv".to_string(),
            };
            text.push_str(&format!("- {reference}: {}\n", issue.message));
        }
    }
    text.push_str("\nHinweis: Die lokale Prüfung erkennt Veränderungen an den archivierten Daten. Eine externe Zeitstempelung oder behördliche Zertifizierung ist damit nicht verbunden.\n");
    text
}

pub(crate) fn verify_archive_to(paths: &ArchivePaths) -> Result<ArchiveVerificationReport, String> {
    let connection = open_database(paths)?;
    let checked_at_ms = now_ms()?;
    let checked_at_display: String = connection
        .query_row(
            "SELECT strftime('%d.%m.%Y, %H:%M:%S', 'now', 'localtime')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let mut issues = Vec::new();
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if integrity != "ok" {
        issues.push(verification_issue(
            None,
            format!("Die Archivdatenbank meldet: {integrity}"),
        ));
    }
    let mut statement = connection
        .prepare(&format!(
            "SELECT {ROW_COLUMNS} FROM archive_entries ORDER BY sequence"
        ))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], row_from_sql)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut expected_sequence = 1_i64;
    let mut expected_previous = EMPTY_HEAD.to_string();
    let mut file_count = 0_i64;
    let mut signed_count = 0_i64;
    let expected_files = rows
        .iter()
        .flat_map(|row| {
            let mut files = vec![row.pdf_path.clone(), row.xml_path.clone()];
            if let Some(report) = report_path_of(row) {
                files.push(report);
            }
            files
        })
        .collect::<HashSet<_>>();

    for row in &rows {
        if row.sequence != expected_sequence {
            issues.push(verification_issue(
                Some(row),
                format!("Die laufende Nummer {expected_sequence} fehlt."),
            ));
            expected_sequence = row.sequence;
        }
        if row.previous_chain_hash != expected_previous {
            issues.push(verification_issue(
                Some(row),
                "Die Verbindung zum vorherigen Eintrag stimmt nicht.",
            ));
        }
        match read_checked_file(&paths.root, &row.pdf_path) {
            Ok(contents) => {
                file_count += 1;
                if sha256_hex(&contents) != row.pdf_sha256 {
                    issues.push(verification_issue(
                        Some(row),
                        "Die PDF-Datei wurde verändert.",
                    ));
                }
            }
            Err(message) => issues.push(verification_issue(Some(row), message)),
        }
        match read_checked_file(&paths.root, &row.xml_path) {
            Ok(contents) => {
                file_count += 1;
                if sha256_hex(&contents) != row.xml_sha256 {
                    issues.push(verification_issue(
                        Some(row),
                        "Die Rechnungsdaten wurden verändert.",
                    ));
                }
            }
            Err(message) => issues.push(verification_issue(Some(row), message)),
        }
        if let Some(report_path) = report_path_of(row) {
            match read_checked_file(&paths.root, &report_path) {
                Ok(contents) => {
                    file_count += 1;
                    let expected = row
                        .validation_json
                        .as_deref()
                        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                        .and_then(|value| value["reportSha256"].as_str().map(str::to_string));
                    let actual = sha256_hex(&contents);
                    if expected.as_deref() != Some(actual.as_str()) {
                        issues.push(verification_issue(
                            Some(row),
                            "Der unabhängige Prüfbericht wurde verändert.",
                        ));
                    }
                }
                Err(message) => issues.push(verification_issue(Some(row), message)),
            }
        } else if row.validation_json.is_some() {
            issues.push(verification_issue(
                Some(row),
                "Der unabhängige Prüfnachweis ist unvollständig.",
            ));
        }
        let calculated_chain_hash = calculate_chain_hash(row);
        if calculated_chain_hash != row.chain_hash {
            issues.push(verification_issue(
                Some(row),
                "Die gespeicherten Angaben wurden verändert.",
            ));
        }
        match (&row.signature, &row.signing_key_id) {
            (Some(signature_bytes), Some(identifier)) => {
                signed_count += 1;
                let public_key: Option<Vec<u8>> = connection
                    .query_row(
                        "SELECT public_key FROM archive_keys WHERE id = ?1",
                        [identifier],
                        |sql_row| sql_row.get(0),
                    )
                    .optional()
                    .map_err(|error| error.to_string())?;
                let verified = public_key
                    .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
                    .and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
                    .zip(
                        <[u8; 64]>::try_from(signature_bytes.clone())
                            .ok()
                            .map(|bytes| Signature::from_bytes(&bytes)),
                    )
                    .is_some_and(|(key, signature)| {
                        key.verify(row.chain_hash.as_bytes(), &signature).is_ok()
                            && sha256_hex(&key.to_bytes()) == *identifier
                    });
                if !verified {
                    issues.push(verification_issue(
                        Some(row),
                        "Der zusätzliche Schutz ist ungültig.",
                    ));
                }
            }
            (None, None) => {}
            _ => issues.push(verification_issue(
                Some(row),
                "Der zusätzliche Schutz ist unvollständig.",
            )),
        }
        expected_previous = row.chain_hash.clone();
        expected_sequence += 1;
    }

    for unregistered in collect_archive_files(&paths.root)?.difference(&expected_files) {
        issues.push(verification_issue(
            None,
            format!("Nicht zugeordnete Datei im Belegarchiv: {unregistered}"),
        ));
    }

    let (stored_count, stored_head): (i64, String) = connection
        .query_row(
            "SELECT entry_count, head_hash FROM archive_state WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    if stored_count != rows.len() as i64 {
        issues.push(verification_issue(
            None,
            "Die gespeicherte Anzahl der Archiveinträge stimmt nicht.",
        ));
    }
    if stored_head != expected_previous {
        issues.push(verification_issue(
            None,
            "Der Abschluss der Archivkette stimmt nicht.",
        ));
    }

    fs::create_dir_all(&paths.reports).map_err(|error| error.to_string())?;
    let report_path = paths
        .reports
        .join(format!("Archivprüfung-{checked_at_ms}.txt"));
    let mut report = ArchiveVerificationReport {
        valid: issues.is_empty(),
        checked_at_ms,
        checked_at_display,
        entry_count: rows.len() as i64,
        file_count,
        signed_count,
        unsigned_count: rows.len() as i64 - signed_count,
        issues,
        report_path: report_path.to_string_lossy().into_owned(),
    };
    write_new_file(&report_path, render_report(&report).as_bytes())?;
    report.report_path = report_path.to_string_lossy().into_owned();
    Ok(report)
}

pub(crate) fn open_native(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    Command::new("explorer.exe")
        .arg(path)
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(path)
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn find_existing_export(
    connection: &Connection,
    evidence: &InvoiceEvidence,
    format: &str,
    xml_sha256: &str,
    pdf_sha256: &str,
) -> Result<Option<ArchiveRow>, String> {
    connection
        .query_row(
            &format!(
                "SELECT {ROW_COLUMNS} FROM archive_entries
                 WHERE format = ?1 AND xml_sha256 = ?2 AND pdf_sha256 = ?3
                   AND json_valid(evidence_json)
                   AND json_extract(evidence_json,'$.documentId') = ?4
                   AND json_extract(evidence_json,'$.sourceRevision') = ?5
                 ORDER BY sequence DESC LIMIT 1"
            ),
            params![
                format,
                xml_sha256,
                pdf_sha256,
                evidence.document_id,
                evidence.source_revision
            ],
            row_from_sql,
        )
        .optional()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn save_and_archive_invoice(
    app: AppHandle,
    mut request: SaveAndArchiveRequest,
) -> Result<SaveAndArchiveResult, String> {
    let _lock = crate::guard::exclusive();
    let evidence = request
        .evidence
        .as_mut()
        .ok_or_else(|| "Bitte die Rechnung vor der Ausgabe als Entwurf speichern.".to_string())?;
    evidence.original_hash =
        super::workspace::export_source(&app, &evidence.document_id, evidence.source_revision)?;
    evidence.content_hash = sha256_hex(evidence.snapshot.as_bytes());
    let pdf = base64::engine::general_purpose::STANDARD
        .decode(&request.pdf_contents_base64)
        .map_err(|error| format!("PDF-Rechnung ist nicht gültig kodiert: {error}"))?;
    let xml_hash = sha256_hex(request.xml_contents.as_bytes());
    let pdf_hash = sha256_hex(&pdf);
    let paths = archive_paths(&app)?;
    let documents = super::paths::documents()?;
    let connection = open_database(&paths)?;
    if let Some(existing) = find_existing_export(
        &connection,
        evidence,
        &request.format,
        &xml_hash,
        &pdf_hash,
    )? {
        if let Some(ticket_id) = &request.ticket_id {
            super::validator::consume_ticket(&app, ticket_id);
        }
        return Ok(SaveAndArchiveResult {
            output_path: checked_relative_path(
                &paths.root,
                if request.format == "xrechnung" {
                    &existing.xml_path
                } else {
                    &existing.pdf_path
                },
            )?
            .to_string_lossy()
            .into_owned(),
            archive_entry: detail_from_row(&existing),
        });
    }
    drop(connection);
    let ticket_id = request
        .ticket_id
        .clone()
        .ok_or_else(|| "Bitte die Rechnung zuerst unabhängig prüfen lassen.".to_string())?;
    let ticket = super::validator::load_ticket(&app, &ticket_id)?;
    super::validator::ticket_matches(
        &ticket,
        &super::validator::ExportCandidate {
            document_id: &evidence.document_id,
            source_revision: evidence.source_revision,
            format: &request.format,
            xml: &request.xml_contents,
            pdf: &pdf,
            snapshot: &evidence.snapshot,
            original_hash: &evidence.original_hash,
        },
    )?;
    request.report_xml = Some(ticket.report_xml.clone());
    request.validation_json = Some(
        serde_json::json!({
            "schemaVersion": 1,
            "engine": ticket.report.engine,
            "engineVersion": ticket.report.engine_version,
            "ruleVersion": ticket.report.rule_version,
            "status": ticket.report.status,
            "xmlSha256": ticket.xml_sha256,
            "pdfSha256": ticket.pdf_sha256,
            "reportSha256": ticket.report.report_sha256.clone().unwrap_or_else(|| super::validator::sha256_hex(ticket.report_xml.as_bytes())),
            "snapshotHash": ticket.snapshot_hash,
            "documentId": ticket.document_id,
            "sourceRevision": ticket.source_revision
        })
        .to_string(),
    );
    let result = save_and_archive_to(&paths, &documents, request)?;
    super::validator::consume_ticket(&app, &ticket_id);
    Ok(result)
}

#[tauri::command]
pub(crate) fn list_archive_entries(
    app: AppHandle,
    query: ArchiveQuery,
) -> Result<ArchiveListResult, String> {
    list_entries_to(&archive_paths(&app)?, query)
}

#[tauri::command]
pub(crate) fn get_archive_entry(app: AppHandle, id: String) -> Result<ArchiveEntryDetail, String> {
    let paths = archive_paths(&app)?;
    let connection = open_database(&paths)?;
    Ok(detail_from_row(&load_row(&connection, &id)?))
}

#[tauri::command]
pub(crate) fn get_archive_status(app: AppHandle) -> Result<ArchiveStatus, String> {
    archive_status_to(&archive_paths(&app)?)
}

#[tauri::command]
pub(crate) fn set_archive_signing(app: AppHandle, enabled: bool) -> Result<ArchiveStatus, String> {
    let _lock = crate::guard::exclusive();
    set_signing_to(&archive_paths(&app)?, enabled)
}

#[tauri::command]
pub(crate) fn verify_archive(app: AppHandle) -> Result<ArchiveVerificationReport, String> {
    verify_archive_to(&archive_paths(&app)?)
}

#[tauri::command]
pub(crate) fn open_archive_folder(app: AppHandle) -> Result<(), String> {
    let paths = archive_paths(&app)?;
    open_database(&paths)?;
    open_native(&paths.root)
}

#[tauri::command]
pub(crate) fn open_archive_entry_file(
    app: AppHandle,
    id: String,
    file_kind: String,
) -> Result<(), String> {
    let paths = archive_paths(&app)?;
    let connection = open_database(&paths)?;
    let row = load_row(&connection, &id)?;
    let relative = match file_kind.as_str() {
        "pdf" => row.pdf_path,
        "xml" => row.xml_path,
        "report" => report_path_of(&row).ok_or_else(|| "Für diese ältere Ausgabe liegt kein unabhängiger Prüfbericht vor.".to_string())?,
        _ => return Err("Unbekannte Archivdatei.".to_string()),
    };
    let path = checked_relative_path(&paths.root, &relative)?;
    let root = paths
        .root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let path = path
        .canonicalize()
        .map_err(|_| "Die Archivdatei wurde nicht gefunden.".to_string())?;
    if !path.starts_with(root) {
        return Err("Zugriff außerhalb des Archivs abgelehnt.".to_string());
    }
    open_native(&path)
}

#[tauri::command]
pub(crate) fn open_archive_report(app: AppHandle, report_path: String) -> Result<(), String> {
    let paths = archive_paths(&app)?;
    let reports = paths
        .reports
        .canonicalize()
        .map_err(|_| "Der Prüfbericht wurde nicht gefunden.".to_string())?;
    let path = Path::new(&report_path)
        .canonicalize()
        .map_err(|_| "Der Prüfbericht wurde nicht gefunden.".to_string())?;
    if !path.starts_with(reports)
        || path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("txt"))
    {
        return Err("Zugriff außerhalb der Prüfberichte abgelehnt.".to_string());
    }
    open_native(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(name: &str) -> (PathBuf, ArchivePaths) {
        let root =
            std::env::temp_dir().join(format!("erechnung-archive-{name}-{}", Uuid::new_v4()));
        let documents = root.join("documents");
        let app_data = root.join("app-data");
        fs::create_dir_all(&documents).expect("test documents");
        (root, ArchivePaths::new(&documents, &app_data))
    }

    fn request(invoice_number: &str, format: &str) -> SaveAndArchiveRequest {
        let xml = if format == "xrechnung" {
            format!("<?xml version=\"1.0\"?><Invoice id=\"{invoice_number}\"></Invoice>")
        } else {
            format!(
                "<?xml version=\"1.0\"?><rsm:CrossIndustryInvoice id=\"{invoice_number}\"></rsm:CrossIndustryInvoice>"
            )
        };
        let pdf = if format == "zugferd" {
            b"%PDF-1.7\nfactur-x.xml\n/Alternative\n%%EOF".as_slice()
        } else {
            b"%PDF-1.7\n%%EOF".as_slice()
        };
        SaveAndArchiveRequest {
            evidence: None,
            ticket_id: None,
            validation_json: None,
            report_xml: None,
            format: format.to_string(),
            output_file_name: format!(
                "{invoice_number}.{}",
                if format == "xrechnung" { "xml" } else { "pdf" }
            ),
            pdf_contents_base64: base64::engine::general_purpose::STANDARD.encode(pdf),
            xml_contents: xml,
            metadata: ArchiveMetadata {
                invoice_number: invoice_number.to_string(),
                issue_date: "2026-09-02".to_string(),
                seller_name: "Musterbetrieb GmbH".to_string(),
                buyer_name: "Beispielkunde AG".to_string(),
                gross_amount: "119.00".to_string(),
                currency: "EUR".to_string(),
                source_file_name: "quelle.pdf".to_string(),
            },
        }
    }

    #[test]
    fn migrates_legacy_chain_and_binds_signed_invoice_evidence() {
        let (root, paths) = test_paths("evidence-migration");
        let documents = root.join("documents");
        let legacy =
            save_and_archive_to(&paths, &documents, request("OLD-1", "xrechnung")).unwrap();
        // Recreate exactly the pre-evidence schema without changing legacy hashes.
        open_database(&paths)
            .unwrap()
            .execute_batch(
                "ALTER TABLE archive_entries DROP COLUMN evidence_json; PRAGMA user_version=1;",
            )
            .unwrap();
        assert!(verify_archive_to(&paths).unwrap().valid);
        assert!(verified_datev_source(&paths, &legacy.archive_entry.id).is_err());
        set_signing_to(&paths, true).unwrap();
        let mut req = request("NEW-1", "xrechnung");
        let snapshot =
            serde_json::json!({"schemaVersion":1,"serializerVersion":"ubl-cii-v1","invoice":{
                "invoiceNumber":req.metadata.invoice_number,"issueDate":req.metadata.issue_date,
                "seller":{"name":req.metadata.seller_name},"buyer":{"name":req.metadata.buyer_name},
                "totals":{"payable":req.metadata.gross_amount},"currency":req.metadata.currency
            }})
            .to_string();
        req.evidence = Some(InvoiceEvidence {
            schema_version: 1,
            document_id: Uuid::new_v4().to_string(),
            source_revision: 3,
            original_hash: sha256_hex(b"original"),
            content_hash: sha256_hex(snapshot.as_bytes()),
            snapshot,
            hybrid_confirmed: true,
        });
        let new = save_and_archive_to(&paths, &documents, req).unwrap();
        assert_eq!(
            new.archive_entry.previous_chain_hash,
            legacy.archive_entry.chain_hash
        );
        assert!(verify_archive_to(&paths).unwrap().valid);
        assert!(verified_datev_source(&paths, &new.archive_entry.id).is_ok());
        let db = open_database(&paths).unwrap();
        db.execute("UPDATE archive_entries SET evidence_json=replace(evidence_json,'NEW-1','BAD-1') WHERE id=?1",[&new.archive_entry.id]).unwrap();
        assert!(!verify_archive_to(&paths).unwrap().valid);
        assert!(verified_datev_source(&paths, &new.archive_entry.id).is_err());
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn archives_pdf_and_xml_with_hash_chain() {
        let (root, paths) = test_paths("chain");
        let documents = root.join("documents");
        let first = save_and_archive_to(&paths, &documents, request("R-1001", "xrechnung"))
            .expect("first archive entry");
        let second = save_and_archive_to(&paths, &documents, request("R-1002", "zugferd"))
            .expect("second archive entry");

        assert_eq!(first.archive_entry.sequence, 1);
        assert_eq!(second.archive_entry.sequence, 2);
        assert_eq!(
            second.archive_entry.previous_chain_hash,
            first.archive_entry.chain_hash
        );
        assert_ne!(
            second.archive_entry.pdf_sha256,
            second.archive_entry.xml_sha256
        );
        assert!(Path::new(&first.output_path).is_file());
        let report = verify_archive_to(&paths).expect("verify archive");
        assert!(
            report.valid,
            "{:?}",
            report
                .issues
                .iter()
                .map(|issue| &issue.message)
                .collect::<Vec<_>>()
        );
        assert_eq!(report.entry_count, 2);
        assert_eq!(report.file_count, 4);
        fs::remove_dir_all(root).expect("remove isolated archive");
    }

    #[test]
    fn detects_changed_archived_file() {
        let (root, paths) = test_paths("tamper");
        let documents = root.join("documents");
        let result = save_and_archive_to(&paths, &documents, request("R-2001", "xrechnung"))
            .expect("archive entry");
        let pdf_path =
            checked_relative_path(&paths.root, &result.archive_entry.pdf_path).expect("pdf path");
        fs::write(pdf_path, b"changed").expect("tamper pdf");

        let report = verify_archive_to(&paths).expect("verify archive");
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("PDF-Datei wurde verändert"))
        );
        fs::remove_dir_all(root).expect("remove isolated archive");
    }

    #[test]
    fn detects_file_without_database_entry() {
        let (root, paths) = test_paths("orphan");
        open_database(&paths).expect("initialize archive");
        let unregistered = paths.root.join("Belege/2026/09/orphan/rechnung.pdf");
        write_new_file(&unregistered, b"%PDF-1.7\n%%EOF").expect("write unregistered file");

        let report = verify_archive_to(&paths).expect("verify archive");
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("Nicht zugeordnete Datei"))
        );
        fs::remove_dir_all(root).expect("remove isolated archive");
    }

    #[test]
    fn signs_and_verifies_new_entries_with_local_key() {
        let (root, paths) = test_paths("signature");
        let documents = root.join("documents");
        let status = set_signing_to(&paths, true).expect("enable signing");
        assert!(status.signing_enabled);
        assert!(status.signing_key_id.is_some());
        let result = save_and_archive_to(&paths, &documents, request("R-3001", "xrechnung"))
            .expect("signed archive entry");
        assert!(result.archive_entry.signed);

        let report = verify_archive_to(&paths).expect("verify signed archive");
        assert!(report.valid);
        assert_eq!(report.signed_count, 1);
        fs::remove_dir_all(root).expect("remove isolated archive");
    }

    #[test]
    fn searches_and_filters_archive_metadata() {
        let (root, paths) = test_paths("search");
        let documents = root.join("documents");
        save_and_archive_to(&paths, &documents, request("HAND-4711", "xrechnung"))
            .expect("archive entry");
        save_and_archive_to(&paths, &documents, request("PDF-2026", "zugferd"))
            .expect("archive entry");

        let by_number = list_entries_to(
            &paths,
            ArchiveQuery {
                search: Some("4711".to_string()),
                ..ArchiveQuery::default()
            },
        )
        .expect("search entries");
        assert_eq!(by_number.total, 1);
        assert_eq!(by_number.entries[0].invoice_number, "HAND-4711");
        let by_format = list_entries_to(
            &paths,
            ArchiveQuery {
                format: Some("zugferd".to_string()),
                ..ArchiveQuery::default()
            },
        )
        .expect("filter entries");
        assert_eq!(by_format.total, 1);
        assert_eq!(by_format.entries[0].invoice_number, "PDF-2026");

        let first_page = list_entries_to(
            &paths,
            ArchiveQuery {
                limit: Some(1),
                offset: Some(0),
                ..ArchiveQuery::default()
            },
        )
        .expect("first archive page");
        assert_eq!(first_page.total, 2);
        assert_eq!(first_page.entries.len(), 1);
        assert_eq!(first_page.entries[0].invoice_number, "PDF-2026");

        let second_page = list_entries_to(
            &paths,
            ArchiveQuery {
                limit: Some(1),
                offset: Some(1),
                ..ArchiveQuery::default()
            },
        )
        .expect("second archive page");
        assert_eq!(second_page.total, 2);
        assert_eq!(second_page.entries[0].invoice_number, "HAND-4711");
        fs::remove_dir_all(root).expect("remove isolated archive");
    }

    #[test]
    fn validates_real_calendar_dates() {
        assert!(super::is_iso_date("2024-02-29"));
        assert!(!super::is_iso_date("2026-02-29"));
        assert!(!super::is_iso_date("2026-13-01"));
        assert!(!super::is_iso_date("not-a-date"));
    }

    #[test]
    fn never_overwrites_an_existing_output() {
        let (root, paths) = test_paths("output");
        let documents = root.join("documents");
        let first = save_and_archive_to(&paths, &documents, request("R-4001", "xrechnung"))
            .expect("first output");
        let second = save_and_archive_to(&paths, &documents, request("R-4001", "xrechnung"))
            .expect("second output");
        assert_ne!(first.output_path, second.output_path);
        assert!(second.output_path.contains("(2)"));
        fs::remove_dir_all(root).expect("remove isolated archive");
    }

    fn checked_request(invoice_number: &str) -> SaveAndArchiveRequest {
        let mut req = request(invoice_number, "xrechnung");
        let report = include_str!("../../../../test/fixtures/validation/kosit-valid.xml");
        let xml_sha = sha256_hex(req.xml_contents.as_bytes());
        let pdf = base64::engine::general_purpose::STANDARD
            .decode(&req.pdf_contents_base64)
            .unwrap();
        req.report_xml = Some(report.to_string());
        req.validation_json = Some(
            serde_json::json!({
                "schemaVersion": 1,
                "engine": "kosit",
                "engineVersion": "1.5.0",
                "ruleVersion": "xrechnung-3.0.2-2026-01-31",
                "status": "passed",
                "xmlSha256": xml_sha,
                "pdfSha256": sha256_hex(&pdf),
                "reportSha256": sha256_hex(report.as_bytes()),
                "snapshotHash": "ab",
                "documentId": "00000000-0000-0000-0000-000000000001",
                "sourceRevision": 1
            })
            .to_string(),
        );
        req
    }

    #[test]
    fn new_checked_entries_use_v3_chain_and_keep_legacy_hashes() {
        let (root, paths) = test_paths("validation-v3");
        let documents = root.join("documents");
        let legacy = save_and_archive_to(&paths, &documents, request("OLD-2", "xrechnung")).unwrap();
        assert!(!legacy.archive_entry.independently_checked);
        let checked = save_and_archive_to(&paths, &documents, checked_request("NEW-2")).unwrap();
        assert!(checked.archive_entry.independently_checked);
        assert_eq!(
            checked.archive_entry.previous_chain_hash,
            legacy.archive_entry.chain_hash
        );
        assert_eq!(
            checked.archive_entry.rule_version.as_deref(),
            Some("xrechnung-3.0.2-2026-01-31")
        );
        assert!(verify_archive_to(&paths).unwrap().valid);
        let report_path = paths.root.join(
            checked
                .archive_entry
                .xml_path
                .replace("rechnungsdaten.xml", "pruefbericht.xml"),
        );
        fs::write(&report_path, b"tampered").unwrap();
        assert!(!verify_archive_to(&paths).unwrap().valid);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_revision_export_is_found_instead_of_duplicated() {
        let (root, paths) = test_paths("idempotent-export");
        let documents = root.join("documents");
        let mut req = checked_request("IDEM-1");
        let snapshot =
            serde_json::json!({"schemaVersion":1,"serializerVersion":"ubl-cii-v1","invoice":{
                "invoiceNumber":req.metadata.invoice_number,"issueDate":req.metadata.issue_date,
                "seller":{"name":req.metadata.seller_name},"buyer":{"name":req.metadata.buyer_name},
                "totals":{"payable":req.metadata.gross_amount},"currency":req.metadata.currency
            }})
            .to_string();
        req.evidence = Some(InvoiceEvidence {
            schema_version: 1,
            document_id: "11111111-1111-1111-1111-111111111111".into(),
            source_revision: 4,
            original_hash: sha256_hex(b"original"),
            content_hash: sha256_hex(snapshot.as_bytes()),
            snapshot,
            hybrid_confirmed: true,
        });
        let first = save_and_archive_to(&paths, &documents, req.clone()).unwrap();
        let db = open_database(&paths).unwrap();
        let found = find_existing_export(
            &db,
            req.evidence.as_ref().unwrap(),
            "xrechnung",
            &first.archive_entry.xml_sha256,
            &first.archive_entry.pdf_sha256,
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.id, first.archive_entry.id);
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
}
