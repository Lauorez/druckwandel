use crate::workspace::{Document, Workspace};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Handoff {
    schema_version: u32,
    job_id: String,
    handed_off_at: String,
    pdf_file_name: String,
    document_name: String,
    printer_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Acknowledgement<'a> {
    schema_version: u32,
    job_id: &'a str,
    opened_at_ms: u128,
    status: &'static str,
}

#[derive(Debug)]
pub struct PrintJob {
    pub id: String,
    pub name: String,
    pub pdf: PathBuf,
    root: PathBuf,
}

pub fn inbox_path() -> Result<PathBuf, String> {
    Ok(dirs::document_dir()
        .ok_or("Dokumentenordner nicht gefunden")?
        .join("E-Rechnung Druckeingang"))
}

fn read_job(root: &Path, id: &str) -> Result<PrintJob, String> {
    let uuid = Uuid::parse_str(id).map_err(|_| "Ungültige Druckauftrag-ID")?;
    let id = uuid.to_string();
    let metadata_path = root.join(format!("{id}.printjob.json"));
    let metadata = fs::metadata(&metadata_path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 {
        return Err("Druckauftrag-Metadaten sind ungültig.".into());
    }
    let handoff: Handoff =
        serde_json::from_slice(&fs::read(&metadata_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let filename = format!("{id}.pdf");
    if handoff.schema_version != 1
        || !handoff.job_id.eq_ignore_ascii_case(&id)
        || !handoff.pdf_file_name.eq_ignore_ascii_case(&filename)
        || handoff.document_name.trim().is_empty()
        || handoff.handed_off_at.trim().is_empty()
        || handoff.printer_name.trim().is_empty()
    {
        return Err("Druckauftrag-Metadaten passen nicht zum PDF.".into());
    }
    let pdf = root.join(filename);
    let file = fs::metadata(&pdf).map_err(|e| e.to_string())?;
    if !file.is_file() || file.len() > 100 * 1024 * 1024 {
        return Err("Druckauftrag-PDF ist ungültig oder zu groß.".into());
    }
    Ok(PrintJob {
        id,
        name: handoff.document_name,
        pdf,
        root: root.to_owned(),
    })
}

pub fn pending_jobs(workspace: &Workspace) -> Result<Vec<PrintJob>, String> {
    let root = inbox_path()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut jobs = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(id) = name.strip_suffix(".printjob.json") else {
            continue;
        };
        if Uuid::parse_str(id).is_err() {
            continue;
        }
        if workspace.has_print_job(id)? && root.join(format!("{id}.review.json")).exists() {
            continue;
        }
        // A malformed or incomplete handoff must not prevent other jobs from opening.
        if let Ok(job) = read_job(&root, id) {
            jobs.push(job);
        }
    }
    jobs.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(jobs)
}

pub fn job_from_uri(uri: &str) -> Result<PrintJob, String> {
    let prefix = "erechnung-review://print-job/";
    let id = uri
        .strip_prefix(prefix)
        .and_then(|rest| rest.split(['?', '#']).next())
        .ok_or("Ungültiger Druckauftrag-Link")?;
    read_job(&inbox_path()?, id)
}

pub fn import_job(workspace: &Workspace, job: &PrintJob) -> Result<Document, String> {
    let document = match workspace.document_for_print_job(&job.id)? {
        Some(document) => document,
        None => workspace.import_print_job(&job.pdf, &job.name, &job.id)?,
    };
    let acknowledgement = Acknowledgement {
        schema_version: 1,
        job_id: &job.id,
        opened_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis(),
        status: "opened",
    };
    let target = job.root.join(format!("{}.review.json", job.id));
    let temporary = job.root.join(format!(".{}.{}.tmp", job.id, Uuid::new_v4()));
    let result = (|| {
        let mut file = File::create(&temporary).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(&acknowledgement).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, &target).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_file(&temporary);
    result?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_companion_handoff_and_records_import_once() {
        let temp = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4().to_string();
        let pdf = temp.path().join(format!("{id}.pdf"));
        fs::write(&pdf, b"%PDF-1.7\nexample").unwrap();
        fs::write(
            temp.path().join(format!("{id}.printjob.json")),
            format!("{{\"schemaVersion\":1,\"jobId\":\"{id}\",\"handedOffAt\":\"2026-10-16T09:00:00Z\",\"pdfFileName\":\"{id}.pdf\",\"documentName\":\"Rechnung aus Drucker\",\"printerName\":\"E-Rechnung\"}}"),
        ).unwrap();
        let job = read_job(temp.path(), &id).unwrap();
        let workspace = Workspace::new(temp.path().join("workspace")).unwrap();
        let document = import_job(&workspace, &job).unwrap();
        assert_eq!(document.name, "Rechnung aus Drucker");
        assert!(workspace.has_print_job(&id).unwrap());
        fs::remove_file(temp.path().join(format!("{id}.review.json"))).unwrap();
        assert_eq!(import_job(&workspace, &job).unwrap().id, document.id);
        assert_eq!(workspace.list().unwrap().len(), 1);
        let ack: serde_json::Value = serde_json::from_slice(
            &fs::read(temp.path().join(format!("{id}.review.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(ack["status"], "opened");
    }
}
