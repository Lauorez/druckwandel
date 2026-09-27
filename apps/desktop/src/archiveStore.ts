import { invoke } from "@tauri-apps/api/core";
import { emitSettingsChanged } from "./settingsWindow.js";

export type ArchiveFormat = "xrechnung" | "zugferd";
export type ArchiveSignatureFilter = "" | "signed" | "unsigned";

export interface ArchiveMetadata {
  invoiceNumber: string;
  issueDate: string;
  sellerName: string;
  buyerName: string;
  grossAmount: string;
  currency: string;
  sourceFileName: string;
}

export interface SaveAndArchiveRequest {
  format: ArchiveFormat;
  outputFileName: string;
  pdfContentsBase64: string;
  xmlContents: string;
  metadata: ArchiveMetadata;
  evidence: { schemaVersion: 1; documentId: string; sourceRevision: number; snapshot: string; hybridConfirmed: boolean };
  ticketId: string;
}

export interface ArchiveEntrySummary {
  id: string;
  sequence: number;
  createdAtMs: number;
  invoiceNumber: string;
  issueDate: string;
  sellerName: string;
  buyerName: string;
  grossAmount: string;
  currency: string;
  format: ArchiveFormat;
  signed: boolean;
  documentId?: string | null;
  contentHash?: string | null;
  independentlyChecked?: boolean;
  ruleVersion?: string | null;
}

export interface ArchiveEntryDetail extends ArchiveEntrySummary {
  sourceFileName: string;
  pdfPath: string;
  xmlPath: string;
  pdfSha256: string;
  xmlSha256: string;
  previousChainHash: string;
  chainHash: string;
  signingKeyId?: string;
}

export interface ArchiveStatus {
  rootPath: string;
  entryCount: number;
  signedCount: number;
  signingEnabled: boolean;
  signingKeyId?: string;
}

export interface ArchiveQuery {
  search?: string;
  format?: "" | ArchiveFormat;
  dateFrom?: string;
  dateTo?: string;
  signature?: ArchiveSignatureFilter;
  limit?: number;
  offset?: number;
}

export interface ArchiveListResult {
  entries: ArchiveEntrySummary[];
  total: number;
}

export interface ArchiveVerificationIssue {
  sequence?: number;
  invoiceNumber?: string;
  message: string;
}

export interface ArchiveVerificationReport {
  valid: boolean;
  checkedAtMs: number;
  checkedAtDisplay: string;
  entryCount: number;
  fileCount: number;
  signedCount: number;
  unsignedCount: number;
  issues: ArchiveVerificationIssue[];
  reportPath: string;
}

export interface SaveAndArchiveResult {
  outputPath: string;
  archiveEntry: ArchiveEntryDetail;
}

export function saveAndArchiveInvoice(request: SaveAndArchiveRequest): Promise<SaveAndArchiveResult> {
  return invoke("save_and_archive_invoice", { request });
}

export interface OfficialCheckIssue {
  severity: "error" | "warning";
  code: string;
  path: string;
  message: string;
}

export interface OfficialCheckResult {
  status: string;
  valid: boolean;
  ticketId?: string | null;
  ruleVersion: string;
  engine: string;
  issues: OfficialCheckIssue[];
}

export function validatePreparedInvoice(request: {
  format: ArchiveFormat;
  xmlContents: string;
  pdfContentsBase64: string;
  documentId: string;
  sourceRevision: number;
  snapshot: string;
}): Promise<OfficialCheckResult> {
  return invoke("validate_prepared_invoice", { request });
}

export function cancelInvoiceValidation(): Promise<void> {
  return invoke("cancel_invoice_validation");
}

export function listArchiveEntries(query: ArchiveQuery): Promise<ArchiveListResult> {
  return invoke("list_archive_entries", { query });
}

export function getArchiveEntry(id: string): Promise<ArchiveEntryDetail> {
  return invoke("get_archive_entry", { id });
}

export function getArchiveStatus(): Promise<ArchiveStatus> {
  return invoke("get_archive_status");
}

export async function setArchiveSigning(enabled: boolean): Promise<ArchiveStatus> {
  const status = await invoke<ArchiveStatus>("set_archive_signing", { enabled });
  await emitSettingsChanged("archive");
  return status;
}

export function verifyArchive(): Promise<ArchiveVerificationReport> {
  return invoke("verify_archive");
}

export function openArchiveFolder(): Promise<void> {
  return invoke("open_archive_folder");
}

export function openArchiveEntryFile(id: string, fileKind: "pdf" | "xml" | "report"): Promise<void> {
  return invoke("open_archive_entry_file", { id, fileKind });
}

export function openArchiveReport(reportPath: string): Promise<void> {
  return invoke("open_archive_report", { reportPath });
}
