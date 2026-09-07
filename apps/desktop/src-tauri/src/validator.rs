use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

const TICKET_TTL_MS: i64 = 30 * 60 * 1000;
const VALIDATION_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_REPORT_BYTES: usize = 8 * 1024 * 1024;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static CANCEL_FLAG: Mutex<Option<std::sync::Arc<AtomicBool>>> = Mutex::new(None);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OfficialIssue {
    severity: String,
    code: String,
    path: String,
    message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OfficialReport {
    pub(crate) schema_version: u32,
    pub(crate) status: String,
    pub(crate) valid: bool,
    pub(crate) engine: String,
    pub(crate) engine_version: String,
    pub(crate) rule_version: String,
    pub(crate) xml_sha256: String,
    pub(crate) pdf_sha256: String,
    pub(crate) report_sha256: Option<String>,
    pub(crate) issues: Vec<OfficialIssue>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidationTicket {
    pub(crate) id: String,
    pub(crate) expires_at_ms: i64,
    pub(crate) document_id: String,
    pub(crate) source_revision: i64,
    pub(crate) format: String,
    pub(crate) snapshot_hash: String,
    pub(crate) original_hash: String,
    pub(crate) xml_sha256: String,
    pub(crate) pdf_sha256: String,
    pub(crate) report_xml: String,
    pub(crate) report: OfficialReport,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidateInvoiceRequest {
    format: String,
    xml_contents: String,
    pdf_contents_base64: String,
    document_id: String,
    source_revision: i64,
    snapshot: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidateInvoiceResult {
    pub(crate) status: String,
    pub(crate) valid: bool,
    pub(crate) ticket_id: Option<String>,
    pub(crate) rule_version: String,
    pub(crate) engine: String,
    pub(crate) issues: Vec<OfficialIssue>,
}

#[derive(Clone)]
pub(crate) struct ValidatorBundle {
    pub(crate) java: PathBuf,
    pub(crate) kosit_jar: PathBuf,
    pub(crate) scenarios: PathBuf,
    pub(crate) repository: PathBuf,
    pub(crate) mustang_jar: PathBuf,
    pub(crate) verapdf_jar: PathBuf,
    pub(crate) kosit_engine_version: String,
    pub(crate) kosit_rule_version: String,
    pub(crate) mustang_engine_version: String,
    pub(crate) mustang_rule_version: String,
    pub(crate) verapdf_engine_version: String,
    pub(crate) verapdf_rule_version: String,
}

#[derive(Default)]
pub(crate) struct ProcessOutcome {
    pub(crate) code: i32,
    pub(crate) output: String,
    pub(crate) report_xml: Option<String>,
    pub(crate) timed_out: bool,
    pub(crate) cancelled: bool,
}

fn now_ms() -> Result<i64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    i64::try_from(millis).map_err(|_| "Systemzeit ist außerhalb des gültigen Bereichs.".to_string())
}

pub(crate) fn sha256_hex(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}

fn ticket_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = super::paths::app_data(app, true)?.join("validation-tickets");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory)
}

fn write_ticket(directory: &Path, ticket: &ValidationTicket) -> Result<(), String> {
    let path = directory.join(format!("{}.json", ticket.id));
    let bytes = serde_json::to_vec(ticket).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())
}

pub(crate) fn load_ticket(app: &AppHandle, id: &str) -> Result<ValidationTicket, String> {
    Uuid::parse_str(id).map_err(|_| "Ungültige Prüfkennung.".to_string())?;
    let path = ticket_dir(app)?.join(format!("{id}.json"));
    let ticket: ValidationTicket = serde_json::from_slice(
        &fs::read(&path).map_err(|_| "Die Prüfung ist abgelaufen. Bitte die Ausgabe erneut starten.".to_string())?,
    )
    .map_err(|_| "Der Prüfbeleg ist beschädigt.".to_string())?;
    if ticket.expires_at_ms < now_ms()? {
        let _ = fs::remove_file(path);
        return Err("Die Prüfung ist abgelaufen. Bitte die Ausgabe erneut starten.".into());
    }
    Ok(ticket)
}

pub(crate) fn consume_ticket(app: &AppHandle, id: &str) {
    if let Ok(directory) = ticket_dir(app) {
        let _ = fs::remove_file(directory.join(format!("{id}.json")));
    }
}

#[derive(Deserialize)]
struct ManifestFile {
    #[serde(default)]
    java: ManifestJava,
    kosit: ManifestTool,
    mustang: ManifestTool,
    #[serde(default)]
    verapdf: ManifestTool,
}

#[derive(Default, Deserialize)]
struct ManifestJava {
    #[serde(default)]
    relative_path: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ManifestTool {
    engine_version: String,
    rule_version: String,
    #[serde(default)]
    jar: String,
    #[serde(default)]
    scenarios: String,
    #[serde(default)]
    repository: String,
}

fn validator_root(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(root) = std::env::var("ERECHNUNG_VALIDATOR_ROOT") {
        let path = PathBuf::from(root);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    let resource = app
        .path()
        .resource_dir()
        .map_err(|_| "Das Prüfprogramm-Paket wurde nicht gefunden.".to_string())?;
    Ok(resource.join("resources").join("validators"))
}

pub(crate) fn load_bundle(app: &AppHandle) -> Result<ValidatorBundle, String> {
    let root = validator_root(app)?;
    let manifest_path = root.join("manifest.json");
    let manifest: ManifestFile = serde_json::from_slice(&fs::read(&manifest_path).map_err(|_| {
        "Das Prüfprogramm ist in dieser Installation nicht enthalten.".to_string()
    })?)
    .map_err(|_| "Das Prüfprogramm-Verzeichnis ist beschädigt.".to_string())?;
    let java_relative = if manifest.java.relative_path.is_empty() {
        if cfg!(windows) {
            "jre/bin/java.exe".to_string()
        } else {
            "jre/bin/java".to_string()
        }
    } else {
        manifest.java.relative_path
    };
    let mut java = root.join(java_relative);
    if cfg!(windows) && java.extension().is_none() {
        java.set_extension("exe");
    }
    let bundle = ValidatorBundle {
        java,
        kosit_jar: root.join(&manifest.kosit.jar),
        scenarios: root.join(&manifest.kosit.scenarios),
        repository: root.join(if manifest.kosit.repository.is_empty() {
            "kosit/xrechnung"
        } else {
            &manifest.kosit.repository
        }),
        mustang_jar: root.join(&manifest.mustang.jar),
        verapdf_jar: root.join(&manifest.verapdf.jar),
        kosit_engine_version: manifest.kosit.engine_version,
        kosit_rule_version: manifest.kosit.rule_version,
        mustang_engine_version: manifest.mustang.engine_version,
        mustang_rule_version: manifest.mustang.rule_version,
        verapdf_engine_version: manifest.verapdf.engine_version,
        verapdf_rule_version: manifest.verapdf.rule_version,
    };
    for path in [
        &bundle.java,
        &bundle.kosit_jar,
        &bundle.scenarios,
        &bundle.repository,
        &bundle.mustang_jar,
    ] {
        if !path.exists() {
            return Err(
                "Das gebündelte Prüfprogramm fehlt. Eine fertige E-Rechnung kann nicht ohne unabhängige Prüfung erzeugt werden."
                    .into(),
            );
        }
    }
    Ok(bundle)
}

fn attribute<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = source.find(&key)? + key.len();
    let end = source[start..].find('"')? + start;
    Some(&source[start..end])
}

fn collect_inner<'a>(source: &'a str, tag: &str) -> Vec<&'a str> {
    let mut blocks = Vec::new();
    let mut rest = source;
    let unprefixed_open = format!("<{tag}");
    let prefixed_open = format!(":{tag}");
    let unprefixed_close = format!("</{tag}>");
    let prefixed_close = format!(":{tag}>");
    while !rest.is_empty() {
        let unprefixed = rest.find(&unprefixed_open);
        let prefixed = rest.find(&prefixed_open).and_then(|rel| rest[..rel].rfind('<'));
        let start = match (unprefixed, prefixed) {
            (Some(left), Some(right)) => left.min(right),
            (Some(left), None) => left,
            (None, Some(right)) => right,
            (None, None) => break,
        };
        let slice = &rest[start..];
        let close = match (slice.find(&unprefixed_close), slice.find(&prefixed_close)) {
            (Some(left), Some(right)) if left <= right => left + unprefixed_close.len(),
            (Some(left), None) => left + unprefixed_close.len(),
            (None, Some(right)) => right + prefixed_close.len(),
            (Some(_), Some(right)) => right + prefixed_close.len(),
            (None, None) => {
                if let Some(end) = slice.find("/>") {
                    blocks.push(&slice[..end + 2]);
                    rest = &slice[end + 2..];
                    continue;
                }
                break;
            }
        };
        blocks.push(&slice[..close]);
        rest = &slice[close..];
    }
    blocks
}

fn inner_text(block: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    if let Some(start_rel) = block.find(&open).or_else(|| block.find(&format!(":{tag}"))) {
        if let Some(gt) = block[start_rel..].find('>') {
            let content_start = start_rel + gt + 1;
            if let Some(end) = block[content_start..].find('<') {
                return block[content_start..content_start + end]
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&")
                    .trim()
                    .to_string();
            }
        }
    }
    String::new()
}

fn map_location(location: &str, code: &str) -> String {
    let haystack = format!("{location} {code}");
    if haystack.contains("BuyerReference") || haystack.contains("BR-DE-15") || haystack.contains("BT-10") {
        return "buyerReference".into();
    }
    if haystack.contains("AccountingSupplier") || haystack.contains("SellerTradeParty") || haystack.contains("BR-06") {
        return "seller.name".into();
    }
    if haystack.contains("AccountingCustomer") || haystack.contains("BuyerTradeParty") || haystack.contains("BR-07") {
        return "buyer.name".into();
    }
    if haystack.contains("GrandTotal") || haystack.contains("BR-CO-15") {
        return "totals.taxInclusive".into();
    }
    if haystack.contains("TaxTotal") || haystack.contains("BR-CO-14") {
        return "totals.taxTotal".into();
    }
    if haystack.contains("DuePayable") || haystack.contains("BR-CO-16") {
        return "totals.payable".into();
    }
    "document".into()
}

fn issue(code: &str, location: &str, message: &str) -> OfficialIssue {
    OfficialIssue {
        severity: "error".into(),
        code: if code.is_empty() { "XML".into() } else { code.into() },
        path: map_location(location, code),
        message: if message.trim().is_empty() {
            "Die unabhängige Prüfung hat einen Fehler gemeldet.".into()
        } else {
            message.trim().into()
        },
    }
}

pub(crate) fn evaluate_kosit(outcome: &ProcessOutcome, rule_version: &str, engine_version: &str) -> OfficialReport {
    if outcome.cancelled {
        return failed("cancelled", "kosit", engine_version, rule_version, "CANCELLED", "Die Prüfung wurde abgebrochen.");
    }
    if outcome.timed_out {
        return failed("timeout", "kosit", engine_version, rule_version, "TIMEOUT", "Die unabhängige Prüfung hat zu lange gedauert.");
    }
    let Some(report) = outcome.report_xml.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
        return failed("missing-report", "kosit", engine_version, rule_version, "KOSIT-REPORT", "Der maschinenlesbare Prüfbericht fehlt. Die Datei gilt nicht als geprüft.");
    };
    let report_tag = report.find("<rep:report").or_else(|| report.find("<report")).and_then(|index| {
        report[index..].find('>').map(|end| &report[index..=index + end])
    });
    let valid_attr = report_tag.and_then(|tag| attribute(tag, "valid")).map(str::to_ascii_lowercase);
    if !matches!(valid_attr.as_deref(), Some("true" | "false")) {
        return failed("unreadable-report", "kosit", engine_version, rule_version, "KOSIT-REPORT", "Der Prüfbericht konnte nicht gelesen werden.");
    }
    let rejected = report.contains(":reject") || report.contains("<reject")
        || outcome.output.to_ascii_uppercase().contains("| REJECT");
    let accepted = report.contains(":accept") || report.contains("<accept");
    let mut issues = Vec::new();
    for block in collect_inner(report, "failed-assert") {
        issues.push(issue(
            attribute(block, "id").unwrap_or(""),
            attribute(block, "location").unwrap_or(""),
            &inner_text(block, "text"),
        ));
    }
    if valid_attr.as_deref() == Some("true") && outcome.code == 0 && rejected {
        if issues.is_empty() {
            issues.push(issue("KOSIT", "", "KoSIT empfiehlt die Ablehnung des Dokuments."));
        }
        return OfficialReport {
            schema_version: 1,
            status: "failed".into(),
            valid: false,
            engine: "kosit".into(),
            engine_version: engine_version.into(),
            rule_version: rule_version.into(),
            xml_sha256: String::new(),
            pdf_sha256: String::new(),
            report_sha256: Some(sha256_hex(report.as_bytes())),
            issues,
        };
    }
    let passed = valid_attr.as_deref() == Some("true") && !rejected && (accepted || !report.contains("assessment")) && outcome.code == 0;
    if !passed {
        if issues.is_empty() {
            issues.push(issue("KOSIT", "", "KoSIT meldet ein nicht akzeptables Dokument."));
        }
        return OfficialReport {
            schema_version: 1,
            status: "failed".into(),
            valid: false,
            engine: "kosit".into(),
            engine_version: engine_version.into(),
            rule_version: rule_version.into(),
            xml_sha256: String::new(),
            pdf_sha256: String::new(),
            report_sha256: Some(sha256_hex(report.as_bytes())),
            issues,
        };
    }
    OfficialReport {
        schema_version: 1,
        status: "passed".into(),
        valid: true,
        engine: "kosit".into(),
        engine_version: engine_version.into(),
        rule_version: rule_version.into(),
        xml_sha256: String::new(),
        pdf_sha256: String::new(),
        report_sha256: Some(sha256_hex(report.as_bytes())),
        issues: Vec::new(),
    }
}

pub(crate) fn evaluate_mustang(outcome: &ProcessOutcome, rule_version: &str, engine_version: &str) -> OfficialReport {
    if outcome.cancelled {
        return failed("cancelled", "mustang", engine_version, rule_version, "CANCELLED", "Die Prüfung wurde abgebrochen.");
    }
    if outcome.timed_out {
        return failed("timeout", "mustang", engine_version, rule_version, "TIMEOUT", "Die unabhängige Prüfung hat zu lange gedauert.");
    }
    let source = format!("{}\n{}", outcome.report_xml.clone().unwrap_or_default(), outcome.output);
    if !source.contains("<summary") && !source.contains("<validation") {
        let status = if outcome.output.trim().is_empty() { "missing-report" } else { "unreadable-report" };
        return failed(status, "mustang", engine_version, rule_version, "MUSTANG-REPORT", "Der maschinenlesbare Prüfbericht fehlt oder ist unlesbar. Die Datei gilt nicht als geprüft.");
    }
    let mut last_status = None;
    let mut rest = source.as_str();
    while let Some(index) = rest.find("<summary") {
        if let Some(status) = attribute(&rest[index..], "status") {
            last_status = Some(status.to_ascii_lowercase());
        }
        rest = &rest[index + 8..];
    }
    let mut issues = Vec::new();
    for block in collect_inner(&source, "error") {
        issues.push(issue(
            attribute(block, "criterion").unwrap_or("MUSTANG"),
            attribute(block, "location").unwrap_or(""),
            &{
                let text = inner_text(block, "error");
                if text.is_empty() {
                    attribute(block, "message").unwrap_or(block).to_string()
                } else {
                    text
                }
            },
        ));
    }
    let passed = outcome.code == 0 && last_status.as_deref() == Some("valid");
    if !passed {
        if issues.is_empty() {
            issues.push(issue("MUSTANG", "", "Die Factur-X-/ZUGFeRD-Prüfung ist fehlgeschlagen."));
        }
        return OfficialReport {
            schema_version: 1,
            status: if last_status.is_some() { "failed" } else { "unknown" }.into(),
            valid: false,
            engine: "mustang".into(),
            engine_version: engine_version.into(),
            rule_version: rule_version.into(),
            xml_sha256: String::new(),
            pdf_sha256: String::new(),
            report_sha256: outcome.report_xml.as_ref().map(|report| sha256_hex(report.as_bytes())),
            issues,
        };
    }
    OfficialReport {
        schema_version: 1,
        status: "passed".into(),
        valid: true,
        engine: "mustang".into(),
        engine_version: engine_version.into(),
        rule_version: rule_version.into(),
        xml_sha256: String::new(),
        pdf_sha256: String::new(),
        report_sha256: outcome.report_xml.as_ref().map(|report| sha256_hex(report.as_bytes())),
        issues: Vec::new(),
    }
}

pub(crate) fn evaluate_verapdf(outcome: &ProcessOutcome, rule_version: &str, engine_version: &str) -> OfficialReport {
    if outcome.cancelled {
        return failed("cancelled", "verapdf", engine_version, rule_version, "CANCELLED", "Die PDF/A-Prüfung wurde abgebrochen.");
    }
    if outcome.timed_out {
        return failed("timeout", "verapdf", engine_version, rule_version, "TIMEOUT", "Die PDF/A-Prüfung hat zu lange gedauert.");
    }
    let report = outcome.report_xml.clone().unwrap_or_else(|| outcome.output.clone());
    if !report.contains("<report") && !report.contains("<validationReport") {
        let status = if outcome.output.trim().is_empty() { "missing-report" } else { "unreadable-report" };
        return failed(status, "verapdf", engine_version, rule_version, "VERAPDF-REPORT", "Der maschinenlesbare PDF/A-Bericht fehlt. Die Datei gilt nicht als PDF/A-geprüft.");
    }
    let validation_tag = report.find("<validationReport").and_then(|index| {
        report[index..].find('>').map(|end| report[index..=index + end].to_string())
    }).unwrap_or_default();
    let compliant = attribute(&validation_tag, "isCompliant").map(str::to_ascii_lowercase);
    let flavour = attribute(&validation_tag, "flavour").unwrap_or_default().to_ascii_uppercase();
    let failed_parse = report.contains("failedToParse=\"1\"") || report.contains("encrypted=\"1\"");
    let details_tag = report.find("<details").and_then(|index| {
        report[index..].find('>').map(|end| report[index..=index + end].to_string())
    }).unwrap_or_default();
    let failed_checks = attribute(&details_tag, "failedChecks").unwrap_or("0").parse::<i32>().unwrap_or(1);
    let passed = outcome.code == 0 && compliant.as_deref() == Some("true") && flavour.contains("3B") && !failed_parse && failed_checks == 0;
    if !passed {
        return OfficialReport {
            schema_version: 1,
            status: if compliant.is_some() { "failed" } else { "unreadable-report" }.into(),
            valid: false,
            engine: "verapdf".into(),
            engine_version: engine_version.into(),
            rule_version: rule_version.into(),
            xml_sha256: String::new(),
            pdf_sha256: String::new(),
            report_sha256: Some(sha256_hex(report.as_bytes())),
            issues: vec![issue("PDFA", "", "Die PDF/A-3-Prüfung ist fehlgeschlagen. Nicht jedes PDF kann umgewandelt werden.")],
        };
    }
    OfficialReport {
        schema_version: 1,
        status: "passed".into(),
        valid: true,
        engine: "verapdf".into(),
        engine_version: engine_version.into(),
        rule_version: rule_version.into(),
        xml_sha256: String::new(),
        pdf_sha256: String::new(),
        report_sha256: Some(sha256_hex(report.as_bytes())),
        issues: Vec::new(),
    }
}

fn failed(status: &str, engine: &str, engine_version: &str, rule_version: &str, code: &str, message: &str) -> OfficialReport {
    OfficialReport {
        schema_version: 1,
        status: status.into(),
        valid: false,
        engine: engine.into(),
        engine_version: engine_version.into(),
        rule_version: rule_version.into(),
        xml_sha256: String::new(),
        pdf_sha256: String::new(),
        report_sha256: None,
        issues: vec![issue(code, "", message)],
    }
}

fn java_offline_args() -> [&'static str; 9] {
    [
        "-Djava.awt.headless=true",
        "-Djava.net.useSystemProxies=false",
        "-Dhttp.proxyHost=127.0.0.1",
        "-Dhttp.proxyPort=9",
        "-Dhttps.proxyHost=127.0.0.1",
        "-Dhttps.proxyPort=9",
        "-Djavax.xml.accessExternalDTD=",
        "-Djavax.xml.accessExternalSchema=",
        "-Djavax.xml.accessExternalStylesheet=",
    ]
}

fn spawn_limited(mut command: Command, cancel: &AtomicBool) -> Result<ProcessOutcome, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Das Prüfprogramm konnte nicht gestartet werden.".to_string())?;
    let started = Instant::now();
    let (sender, receiver) = mpsc::channel::<(Vec<u8>, Vec<u8>)>();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    thread::spawn(move || {
        let mut out = Vec::new();
        let mut err = Vec::new();
        if let Some(stream) = stdout.take() {
            let _ = stream.take(MAX_OUTPUT_BYTES as u64 + 1).read_to_end(&mut out);
        }
        if let Some(stream) = stderr.take() {
            let _ = stream.take(MAX_OUTPUT_BYTES as u64 + 1).read_to_end(&mut err);
        }
        let _ = sender.send((out, err));
    });
    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(ProcessOutcome {
                code: -1,
                cancelled: true,
                ..ProcessOutcome::default()
            });
        }
        if started.elapsed() > VALIDATION_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(ProcessOutcome {
                code: -1,
                timed_out: true,
                ..ProcessOutcome::default()
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let (out, err) = receiver.recv().unwrap_or_default();
                let mut output = String::from_utf8_lossy(&out).into_owned();
                output.push_str(&String::from_utf8_lossy(&err));
                if output.len() > MAX_OUTPUT_BYTES {
                    output.truncate(MAX_OUTPUT_BYTES);
                    output.push_str("\n[truncated]");
                }
                return Ok(ProcessOutcome {
                    code: status.code().unwrap_or(-1),
                    output,
                    ..ProcessOutcome::default()
                });
            }
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn run_kosit(bundle: &ValidatorBundle, xml: &[u8], cancel: &AtomicBool) -> Result<ProcessOutcome, String> {
    let work = std::env::temp_dir().join(format!("erechnung-kosit-{}", Uuid::new_v4()));
    fs::create_dir_all(&work).map_err(|error| error.to_string())?;
    let invoice = work.join("invoice.xml");
    let reports = work.join("reports");
    fs::create_dir_all(&reports).map_err(|error| error.to_string())?;
    File::create(&invoice)
        .and_then(|mut file| file.write_all(xml))
        .map_err(|error| error.to_string())?;
    let mut command = Command::new(&bundle.java);
    command.args(java_offline_args()).arg("-jar").arg(&bundle.kosit_jar).arg("-s").arg(&bundle.scenarios).arg("-r").arg(&bundle.repository).arg("-o").arg(&reports).arg(&invoice);
    let mut outcome = spawn_limited(command, cancel)?;
    if let Ok(entries) = fs::read_dir(&reports) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.ends_with("-report.xml") {
                let bytes = fs::read(entry.path()).unwrap_or_default();
                if bytes.len() <= MAX_REPORT_BYTES {
                    outcome.report_xml = Some(String::from_utf8_lossy(&bytes).into_owned());
                }
                break;
            }
        }
    }
    let _ = fs::remove_dir_all(&work);
    Ok(outcome)
}

fn run_mustang(bundle: &ValidatorBundle, xml: &[u8], cancel: &AtomicBool) -> Result<ProcessOutcome, String> {
    let work = std::env::temp_dir().join(format!("erechnung-mustang-{}", Uuid::new_v4()));
    fs::create_dir_all(&work).map_err(|error| error.to_string())?;
    let invoice = work.join("invoice.xml");
    fs::write(&invoice, xml).map_err(|error| error.to_string())?;
    let mut command = Command::new(&bundle.java);
    command.args(java_offline_args()).arg("-jar").arg(&bundle.mustang_jar).args(["--action", "validate", "--source"]).arg(&invoice).arg("--disable-file-logging");
    let mut outcome = spawn_limited(command, cancel)?;
    if let Some(start) = outcome.output.find("<?xml").or_else(|| outcome.output.find("<validation")) {
        if let Some(end) = outcome.output[start..].find("</validation>") {
            outcome.report_xml = Some(outcome.output[start..start + end + "</validation>".len()].to_string());
        }
    }
    let _ = fs::remove_dir_all(&work);
    Ok(outcome)
}

fn run_mustang_extract(bundle: &ValidatorBundle, pdf: &[u8], expected_xml: &[u8], cancel: &AtomicBool) -> Result<(), String> {
    let work = std::env::temp_dir().join(format!("erechnung-extract-{}", Uuid::new_v4()));
    fs::create_dir_all(&work).map_err(|error| error.to_string())?;
    let invoice = work.join("invoice.pdf");
    let extracted = work.join("extracted.xml");
    fs::write(&invoice, pdf).map_err(|error| error.to_string())?;
    let mut command = Command::new(&bundle.java);
    command.args(java_offline_args()).arg("-jar").arg(&bundle.mustang_jar).args([
        "--action",
        "extract",
        "--source",
    ]).arg(&invoice).arg("--out").arg(&extracted).arg("--disable-file-logging");
    let outcome = spawn_limited(command, cancel)?;
    let bytes = fs::read(&extracted).unwrap_or_default();
    let _ = fs::remove_dir_all(&work);
    if outcome.cancelled {
        return Err("Die Prüfung wurde abgebrochen.".into());
    }
    if outcome.timed_out {
        return Err("Das Auslesen der eingebetteten Rechnungsdaten hat zu lange gedauert.".into());
    }
    if bytes != expected_xml {
        return Err("Die aus der PDF gelesenen Rechnungsdaten sind nicht mit der geprüften XML-Datei identisch.".into());
    }
    Ok(())
}

fn run_verapdf(bundle: &ValidatorBundle, pdf: &[u8], cancel: &AtomicBool) -> Result<ProcessOutcome, String> {
    let work = std::env::temp_dir().join(format!("erechnung-verapdf-{}", Uuid::new_v4()));
    fs::create_dir_all(&work).map_err(|error| error.to_string())?;
    let invoice = work.join("invoice.pdf");
    fs::write(&invoice, pdf).map_err(|error| error.to_string())?;
    let mut command = Command::new(&bundle.java);
    command.args(java_offline_args()).arg("-jar").arg(&bundle.verapdf_jar).args([
        "--flavour",
        "3b",
        "--format",
        "xml",
        "--maxfailures",
        "20",
        "--loglevel",
        "0",
    ]).arg(&invoice);
    let mut outcome = spawn_limited(command, cancel)?;
    if let Some(start) = outcome.output.find("<?xml").or_else(|| outcome.output.find("<report")) {
        if let Some(end) = outcome.output[start..].find("</report>") {
            outcome.report_xml = Some(outcome.output[start..start + end + "</report>".len()].to_string());
        }
    }
    let _ = fs::remove_dir_all(&work);
    Ok(outcome)
}

fn emit_progress(app: &AppHandle, phase: &str) {
    let _ = app.emit("invoice-validation-progress", serde_json::json!({ "phase": phase }));
}

#[tauri::command]
pub(crate) fn cancel_invoice_validation() -> Result<(), String> {
    if let Ok(guard) = CANCEL_FLAG.lock() {
        if let Some(flag) = guard.as_ref() {
            flag.store(true, Ordering::SeqCst);
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn validate_prepared_invoice(
    app: AppHandle,
    request: ValidateInvoiceRequest,
) -> Result<ValidateInvoiceResult, String> {
    if !matches!(request.format.as_str(), "xrechnung" | "zugferd") {
        return Err("Unbekanntes Rechnungsformat für die Prüfung.".into());
    }
    Uuid::parse_str(&request.document_id).map_err(|_| "Ungültiger Rechnungsbezug.".to_string())?;
    emit_progress(&app, "Angaben werden vorbereitet");
    let original_hash = super::workspace::export_source(&app, &request.document_id, request.source_revision)?;
    let bundle = load_bundle(&app)?;
    let (engine, engine_version, rule_version) = if request.format == "xrechnung" {
        ("kosit", bundle.kosit_engine_version.clone(), bundle.kosit_rule_version.clone())
    } else {
        ("mustang", bundle.mustang_engine_version.clone(), bundle.mustang_rule_version.clone())
    };
    if request.format == "zugferd" && !bundle.verapdf_jar.exists() {
        return Ok(ValidateInvoiceResult {
            status: "unavailable".into(),
            valid: false,
            ticket_id: None,
            rule_version: bundle.verapdf_rule_version.clone(),
            engine: "verapdf".into(),
            issues: vec![issue("VERAPDF", "", "Das PDF/A-Prüfprogramm ist in dieser Installation nicht enthalten. Eine fertige PDF-Rechnung kann nicht erzeugt werden.")],
        });
    }
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    *CANCEL_FLAG.lock().map_err(|_| "Eine Prüfung läuft bereits.".to_string())? = Some(cancel.clone());
    let xml = request.xml_contents.as_bytes().to_vec();
    let outcome = if request.format == "xrechnung" {
        run_kosit(&bundle, &xml, &cancel)
    } else {
        run_mustang(&bundle, &xml, &cancel)
    };
    *CANCEL_FLAG.lock().unwrap_or_else(|error| error.into_inner()) = None;
    emit_progress(&app, "Prüfbericht wird gelesen");
    let pdf = base64::engine::general_purpose::STANDARD
        .decode(&request.pdf_contents_base64)
        .map_err(|_| "PDF-Rechnung ist nicht gültig kodiert.".to_string())?;
    let outcome = outcome?;
    let mut report = if engine == "kosit" {
        evaluate_kosit(&outcome, &rule_version, &engine_version)
    } else {
        evaluate_mustang(&outcome, &rule_version, &engine_version)
    };
    report.xml_sha256 = sha256_hex(request.xml_contents.as_bytes());
    report.pdf_sha256 = sha256_hex(&pdf);
    let mut combined_report = outcome.report_xml.clone().filter(|value| !value.trim().is_empty());
    if request.format == "zugferd" && report.valid {
        emit_progress(&app, "Eingebettete Rechnungsdaten werden geprüft");
        if let Err(message) = run_mustang_extract(&bundle, &pdf, request.xml_contents.as_bytes(), &cancel) {
            report.valid = false;
            report.status = if message.contains("abgebrochen") { "cancelled" } else { "failed" }.into();
            report.issues = vec![issue("XML_MISMATCH", "", &message)];
        }
    }
    if request.format == "zugferd" && report.valid {
        emit_progress(&app, "PDF/A-Prüfung läuft");
        let pdfa_outcome = run_verapdf(&bundle, &pdf, &cancel)?;
        let pdfa = evaluate_verapdf(&pdfa_outcome, &bundle.verapdf_rule_version, &bundle.verapdf_engine_version);
        let pdfa_xml = pdfa_outcome.report_xml.clone().filter(|value| !value.trim().is_empty());
        if !pdfa.valid {
            report = pdfa;
            report.xml_sha256 = sha256_hex(request.xml_contents.as_bytes());
            report.pdf_sha256 = sha256_hex(&pdf);
            combined_report = pdfa_xml;
        } else if let (Some(xml_report), Some(pdfa_report)) = (combined_report.as_ref(), pdfa_xml.as_ref()) {
            combined_report = Some(format!("{xml_report}\n{pdfa_report}"));
        } else {
            report.valid = false;
            report.status = "missing-report".into();
            report.issues = vec![issue("VERAPDF-REPORT", "", "Der maschinenlesbare PDF/A-Bericht fehlt. Die Datei gilt nicht als PDF/A-geprüft.")];
        }
    }
    let mut ticket_id = None;
    if report.valid {
        let report_xml = combined_report.filter(|value| !value.trim().is_empty()).ok_or_else(|| {
                "Der maschinenlesbare Prüfbericht fehlt. Die Datei gilt nicht als geprüft.".to_string()
            })?;
        report.report_sha256 = Some(sha256_hex(report_xml.as_bytes()));
        let ticket = ValidationTicket {
            id: Uuid::new_v4().to_string(),
            expires_at_ms: now_ms()? + TICKET_TTL_MS,
            document_id: request.document_id.clone(),
            source_revision: request.source_revision,
            format: request.format.clone(),
            snapshot_hash: sha256_hex(request.snapshot.as_bytes()),
            original_hash,
            xml_sha256: report.xml_sha256.clone(),
            pdf_sha256: report.pdf_sha256.clone(),
            report_xml,
            report: report.clone(),
        };
        write_ticket(&ticket_dir(&app)?, &ticket)?;
        ticket_id = Some(ticket.id);
    }
    Ok(ValidateInvoiceResult {
        status: report.status,
        valid: report.valid,
        ticket_id,
        rule_version,
        engine: engine.into(),
        issues: report.issues,
    })
}

pub(crate) fn ticket_matches(
    ticket: &ValidationTicket,
    document_id: &str,
    source_revision: i64,
    format: &str,
    xml: &str,
    pdf: &[u8],
    snapshot: &str,
    original_hash: &str,
) -> Result<(), String> {
    if !ticket.report.valid || ticket.report.status != "passed" {
        return Err("Die unabhängige Prüfung ist nicht erfolgreich abgeschlossen.".into());
    }
    if ticket.document_id != document_id
        || ticket.source_revision != source_revision
        || ticket.format != format
        || ticket.xml_sha256 != sha256_hex(xml.as_bytes())
        || ticket.pdf_sha256 != sha256_hex(pdf)
        || ticket.snapshot_hash != sha256_hex(snapshot.as_bytes())
        || ticket.original_hash != original_hash
    {
        return Err("Die Rechnung wurde nach der Prüfung geändert. Bitte die Ausgabe erneut starten.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(report: &str, code: i32) -> ProcessOutcome {
        ProcessOutcome {
            code,
            output: String::new(),
            report_xml: Some(report.to_string()),
            ..ProcessOutcome::default()
        }
    }

    #[test]
    fn kosit_requires_readable_accepted_report() {
        let valid = include_str!("../../../../test/fixtures/validation/kosit-valid.xml");
        let passed = evaluate_kosit(&outcome(valid, 0), "xrechnung-3.0.2", "1.5.0");
        assert!(passed.valid);
        assert_eq!(passed.status, "passed");
        let missing = evaluate_kosit(&ProcessOutcome { code: 0, output: "ACCEPT".into(), ..ProcessOutcome::default() }, "xrechnung-3.0.2", "1.5.0");
        assert!(!missing.valid);
        assert_eq!(missing.status, "missing-report");
        let rejected = evaluate_kosit(&outcome(include_str!("../../../../test/fixtures/validation/kosit-exit0-rejected.xml"), 0), "xrechnung-3.0.2", "1.5.0");
        assert!(!rejected.valid);
    }

    #[test]
    fn kosit_maps_failed_asserts() {
        let failed = evaluate_kosit(&outcome(include_str!("../../../../test/fixtures/validation/kosit-invalid.xml"), 1), "xrechnung-3.0.2", "1.5.0");
        assert!(failed.issues.iter().any(|issue| issue.path == "buyerReference" && issue.code == "BR-DE-15"));
    }

    #[test]
    fn mustang_requires_summary() {
        let passed = evaluate_mustang(&outcome(include_str!("../../../../test/fixtures/validation/mustang-valid.xml"), 0), "factur-x", "2.16.2");
        assert!(passed.valid);
        let failed = evaluate_mustang(&outcome(include_str!("../../../../test/fixtures/validation/mustang-invalid.xml"), 1), "factur-x", "2.16.2");
        assert!(!failed.valid);
        assert!(failed.issues.iter().any(|issue| issue.path == "totals.taxInclusive"));
    }

    #[test]
    fn verapdf_requires_compliant_3b_report() {
        let passed = evaluate_verapdf(&outcome(include_str!("../../../../test/fixtures/validation/verapdf-valid.xml"), 0), "pdfa-3b", "1.28.2");
        assert!(passed.valid);
        let failed = evaluate_verapdf(&outcome(include_str!("../../../../test/fixtures/validation/verapdf-invalid.xml"), 1), "pdfa-3b", "1.28.2");
        assert!(!failed.valid);
    }

    #[test]
    fn timeout_and_cancel_are_failures() {
        assert_eq!(evaluate_kosit(&ProcessOutcome { timed_out: true, code: -1, ..ProcessOutcome::default() }, "r", "e").status, "timeout");
        assert_eq!(evaluate_kosit(&ProcessOutcome { cancelled: true, code: -1, ..ProcessOutcome::default() }, "r", "e").status, "cancelled");
    }

    #[test]
    fn ticket_rejects_changed_bytes() {
        let mut ticket = ValidationTicket {
            id: Uuid::new_v4().to_string(),
            expires_at_ms: now_ms().unwrap() + 1000,
            document_id: Uuid::new_v4().to_string(),
            source_revision: 1,
            format: "xrechnung".into(),
            snapshot_hash: sha256_hex(b"snap"),
            original_hash: "orig".into(),
            xml_sha256: sha256_hex(b"xml"),
            pdf_sha256: sha256_hex(b"pdf"),
            report_xml: "<rep:report valid=\"true\"/>".into(),
            report: OfficialReport {
                schema_version: 1,
                status: "passed".into(),
                valid: true,
                engine: "kosit".into(),
                engine_version: "1".into(),
                rule_version: "x".into(),
                xml_sha256: sha256_hex(b"xml"),
                pdf_sha256: sha256_hex(b"pdf"),
                report_sha256: None,
                issues: vec![],
            },
        };
        assert!(ticket_matches(&ticket, &ticket.document_id, 1, "xrechnung", "xml", b"pdf", "snap", "orig").is_ok());
        assert!(ticket_matches(&ticket, &ticket.document_id, 1, "xrechnung", "xml-changed", b"pdf", "snap", "orig").is_err());
        ticket.report.valid = false;
        assert!(ticket_matches(&ticket, &ticket.document_id, 1, "xrechnung", "xml", b"pdf", "snap", "orig").is_err());
    }
}
