import { invoke } from "@tauri-apps/api/core";

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
  evidence: { schemaVersion: 1; documentId: string; sourceRevision: number; snapshot: string };
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

export function listArchiveEntries(query: ArchiveQuery): Promise<ArchiveListResult> {
  return invoke("list_archive_entries", { query });
}

export function getArchiveEntry(id: string): Promise<ArchiveEntryDetail> {
  return invoke("get_archive_entry", { id });
}

export function getArchiveStatus(): Promise<ArchiveStatus> {
  return invoke("get_archive_status");
}

export function setArchiveSigning(enabled: boolean): Promise<ArchiveStatus> {
  return invoke("set_archive_signing", { enabled });
}

export function verifyArchive(): Promise<ArchiveVerificationReport> {
  return invoke("verify_archive");
}

export function openArchiveFolder(): Promise<void> {
  return invoke("open_archive_folder");
}

export function openArchiveEntryFile(id: string, fileKind: "pdf" | "xml"): Promise<void> {
  return invoke("open_archive_entry_file", { id, fileKind });
}

export function openArchiveReport(reportPath: string): Promise<void> {
  return invoke("open_archive_report", { reportPath });
}
