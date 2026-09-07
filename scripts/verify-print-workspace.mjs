// Called by smoke-print-workspace.ps1 after an actual print-triggered cold start.
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";

const [root,jobId,pdfPath,reportPath] = process.argv.slice(2);
assert(root && jobId && pdfPath && reportPath,"Missing print test paths.");
const db = new DatabaseSync(join(root,"workspace.sqlite3"),{readOnly:true});
try {
  const rows = db.prepare("SELECT id,name,revision,original_sha256,snapshot FROM documents WHERE job_id=?").all(jobId);
  assert.equal(rows.length,1);
  const row = rows[0];
  assert.equal(db.prepare("SELECT value FROM settings WHERE key='last_opened'").get().value,row.id);
  const hash = bytes => createHash("sha256").update(bytes).digest("hex");
  assert.equal(hash(readFileSync(pdfPath)),row.original_sha256);
  assert.equal(hash(readFileSync(join(root,"originals",`${row.id}.pdf`))),row.original_sha256);
  const snapshot = JSON.parse(row.snapshot);
  assert.equal(snapshot.schemaVersion,1);
  assert(snapshot.extraction.pages.length>0);
  assert(row.revision>=1);
  const result = {timestamp:new Date().toISOString(),result:"passed",jobId,documentId:row.id,name:row.name,revision:row.revision,originalSha256:row.original_sha256};
  writeFileSync(reportPath,JSON.stringify(result,null,2)+"\n");
  console.log("PASS: real virtual print, cold start, opened receipt, single persistent draft and matching original hash.");
} finally { db.close(); }
