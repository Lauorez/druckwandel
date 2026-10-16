use crate::invoice::InvoiceInput;
use crate::recognition::RecognitionResult;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_PDF_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub sha256: String,
}

pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("documents")).map_err(|e| e.to_string())?;
        let workspace = Self { root };
        workspace.database()?;
        Ok(workspace)
    }

    fn database(&self) -> Result<Connection, String> {
        let db =
            Connection::open(self.root.join("workspace.sqlite3")).map_err(|e| e.to_string())?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS documents (
               id TEXT PRIMARY KEY,
               name TEXT NOT NULL,
               sha256 TEXT NOT NULL,
               created_at_ms INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS invoice_drafts (
               document_id TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
               data TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS imported_print_jobs (
               job_id TEXT PRIMARY KEY,
               document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE
             );
             CREATE TABLE IF NOT EXISTS recognition_results (
               document_id TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
               data TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS handoff_queue (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               arguments TEXT NOT NULL
             );",
        )
        .map_err(|e| e.to_string())?;
        Ok(db)
    }

    pub fn list(&self) -> Result<Vec<Document>, String> {
        let db = self.database()?;
        let mut statement = db
            .prepare(
                "SELECT id, name, sha256 FROM documents ORDER BY created_at_ms DESC, rowid DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                Ok(Document {
                    path: self.root.join("documents").join(format!("{id}.pdf")),
                    id,
                    name: row.get(1)?,
                    sha256: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn load_invoice(&self, document_id: &str) -> Result<InvoiceInput, String> {
        let db = self.database()?;
        let result = db.query_row(
            "SELECT data FROM invoice_drafts WHERE document_id = ?1",
            [document_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(data) => {
                serde_json::from_str(&data).map_err(|e| format!("Entwurf ist beschädigt: {e}"))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(InvoiceInput::blank()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn has_invoice(&self, document_id: &str) -> Result<bool, String> {
        let db = self.database()?;
        db.query_row(
            "SELECT EXISTS(SELECT 1 FROM invoice_drafts WHERE document_id=?1)",
            [document_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
    }

    pub fn save_recognition(
        &self,
        document_id: &str,
        result: &RecognitionResult,
    ) -> Result<(), String> {
        let data = serde_json::to_string(result).map_err(|e| e.to_string())?;
        if data.len() > 5 * 1024 * 1024 {
            return Err("Erkennungsergebnis ist zu groß.".into());
        }
        self.database()?.execute("INSERT INTO recognition_results(document_id,data) VALUES (?1,?2) ON CONFLICT(document_id) DO UPDATE SET data=excluded.data",params![document_id,data]).map_err(|e|e.to_string())?;
        Ok(())
    }

    pub fn load_recognition(&self, document_id: &str) -> Result<Option<RecognitionResult>, String> {
        let result = self.database()?.query_row(
            "SELECT data FROM recognition_results WHERE document_id=?1",
            [document_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(data) => serde_json::from_str(&data)
                .map(Some)
                .map_err(|e| e.to_string()),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn enqueue_handoff(&self, arguments: &[String]) -> Result<(), String> {
        let data = serde_json::to_string(arguments).map_err(|e| e.to_string())?;
        if data.len() > 64 * 1024 {
            return Err("Startargumente sind zu groß.".into());
        }
        self.database()?
            .execute("INSERT INTO handoff_queue(arguments) VALUES (?1)", [data])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn take_handoffs(&self) -> Result<Vec<Vec<String>>, String> {
        let mut db = self.database()?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let rows: Vec<(i64, String)> = tx
            .prepare("SELECT id,arguments FROM handoff_queue ORDER BY id LIMIT 16")
            .map_err(|e| e.to_string())?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for (id, data) in rows {
            result.push(serde_json::from_str(&data).map_err(|e| e.to_string())?);
            tx.execute("DELETE FROM handoff_queue WHERE id=?1", [id])
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }

    pub fn save_invoice(&self, document_id: &str, invoice: &InvoiceInput) -> Result<(), String> {
        let data = serde_json::to_string(invoice).map_err(|e| e.to_string())?;
        if data.len() > 1024 * 1024 {
            return Err("Der Entwurf überschreitet 1 MB.".into());
        }
        let db = self.database()?;
        db.execute(
            "INSERT INTO invoice_drafts (document_id, data) VALUES (?1, ?2)
             ON CONFLICT(document_id) DO UPDATE SET data=excluded.data",
            params![document_id, data],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn import_pdf(&self, source: &Path) -> Result<Document, String> {
        self.import_pdf_named(source, None)
    }

    pub fn has_print_job(&self, job_id: &str) -> Result<bool, String> {
        let db = self.database()?;
        let count: i64 = db
            .query_row(
                "SELECT count(*) FROM imported_print_jobs WHERE job_id=?1",
                [job_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(count != 0)
    }

    pub fn document_for_print_job(&self, job_id: &str) -> Result<Option<Document>, String> {
        let db = self.database()?;
        let result = db.query_row(
            "SELECT d.id, d.name, d.sha256 FROM documents d JOIN imported_print_jobs p ON p.document_id=d.id WHERE p.job_id=?1",
            [job_id],
            |row| {
                let id: String = row.get(0)?;
                Ok(Document {
                    path: self.root.join("documents").join(format!("{id}.pdf")),
                    id,
                    name: row.get(1)?,
                    sha256: row.get(2)?,
                })
            },
        );
        match result {
            Ok(document) => Ok(Some(document)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn import_print_job(
        &self,
        source: &Path,
        name: &str,
        job_id: &str,
    ) -> Result<Document, String> {
        if self.has_print_job(job_id)? {
            return Err("Druckauftrag wurde bereits importiert.".into());
        }
        let document = self.import_pdf_named(source, Some(name))?;
        let db = self.database()?;
        if let Err(error) = db.execute(
            "INSERT INTO imported_print_jobs(job_id, document_id) VALUES (?1, ?2)",
            params![job_id, document.id],
        ) {
            let _ = db.execute("DELETE FROM documents WHERE id=?1", [&document.id]);
            let _ = fs::remove_file(&document.path);
            return Err(error.to_string());
        }
        Ok(document)
    }

    fn import_pdf_named(
        &self,
        source: &Path,
        display_name: Option<&str>,
    ) -> Result<Document, String> {
        let source_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or("Ungültiger PDF-Dateiname")?
            .to_owned();
        let name = display_name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&source_name)
            .chars()
            .take(255)
            .collect::<String>();
        let mut input = File::open(source).map_err(|e| e.to_string())?;
        let size = input.metadata().map_err(|e| e.to_string())?.len();
        if size > MAX_PDF_BYTES {
            return Err("Die PDF-Datei überschreitet 100 MB.".into());
        }
        let mut header = [0_u8; 5];
        input.read_exact(&mut header).map_err(|e| e.to_string())?;
        if &header != b"%PDF-" {
            return Err("Die Datei ist kein PDF.".into());
        }
        let id = Uuid::new_v4().to_string();
        let path = self.root.join("documents").join(format!("{id}.pdf"));
        let temporary = self.root.join("documents").join(format!(".{id}.tmp"));
        let result = (|| {
            let mut output = File::create(&temporary).map_err(|e| e.to_string())?;
            output.write_all(&header).map_err(|e| e.to_string())?;
            let mut hash = Sha256::new();
            hash.update(header);
            let mut copied = header.len() as u64;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                copied += n as u64;
                if copied > MAX_PDF_BYTES {
                    return Err("Die PDF-Datei überschreitet 100 MB.".into());
                }
                output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                hash.update(&buffer[..n]);
            }
            output.sync_all().map_err(|e| e.to_string())?;
            fs::rename(&temporary, &path).map_err(|e| e.to_string())?;
            let sha256 = format!("{:x}", hash.finalize());
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_millis() as i64;
            let db = self.database()?;
            if let Err(error) = db.execute(
                "INSERT INTO documents (id, name, sha256, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
                params![id, name, sha256, now],
            ) {
                let _ = fs::remove_file(&path);
                return Err(error.to_string());
            }
            Ok(Document {
                id,
                name,
                path,
                sha256,
            })
        })();
        let _ = fs::remove_file(&temporary);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_pdf_and_lists_it_after_reopen() {
        let root = std::env::temp_dir().join(format!("druckwandel-native-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("rechnung.pdf");
        fs::write(&source, b"%PDF-1.7\nexample").unwrap();

        let workspace = Workspace::new(root.join("data")).unwrap();
        let document = workspace.import_pdf(&source).unwrap();
        assert_eq!(fs::read(&document.path).unwrap(), b"%PDF-1.7\nexample");
        assert_eq!(document.sha256.len(), 64);
        assert_eq!(
            Workspace::new(root.join("data")).unwrap().list().unwrap()[0].id,
            document.id
        );
        let mut invoice = InvoiceInput::blank();
        invoice.invoice_number = "R-1".into();
        workspace.save_invoice(&document.id, &invoice).unwrap();
        assert_eq!(
            Workspace::new(root.join("data"))
                .unwrap()
                .load_invoice(&document.id)
                .unwrap()
                .invoice_number,
            "R-1"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_non_pdf_without_creating_a_draft() {
        let root = std::env::temp_dir().join(format!("druckwandel-native-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("rechnung.pdf");
        fs::write(&source, b"not a pdf").unwrap();

        let workspace = Workspace::new(root.join("data")).unwrap();
        assert!(workspace.import_pdf(&source).is_err());
        assert!(workspace.list().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
