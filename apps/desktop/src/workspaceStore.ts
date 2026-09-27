import { invoke } from "@tauri-apps/api/core";
import { isInvoiceTypeCode } from "../../../src/domain/types.js";
import type { ExtractionResult } from "../../../src/extraction/types.js";
import { LEARNABLE_FIELD_NAMES, type FieldSourceSelections, type LearnableFieldName } from "../../../src/learning/correction-memory.js";
import type { ReviewDraft } from "../../../src/review/draft.js";
import { normalizeReviewDraft } from "../../../src/review/draft.js";
import type { PrintJob } from "./printInbox.js";

export interface WorkspaceSnapshot {
  schemaVersion: 1;
  extractionVersion: "text-layout-v1";
  sourceExtraction: ExtractionResult;
  extraction: ExtractionResult;
  draft: ReviewDraft;
  initialDraft: ReviewDraft;
  sourceSelections: FieldSourceSelections;
  pendingSourceFields: LearnableFieldName[];
  completed: boolean;
  hybridConfirmed: boolean;
}
export interface WorkDocument {
  id: string; name: string; sourceKey: string | null; jobId: string | null;
  originalSha256: string; revision: number; status: "new" | "draft" | "done" | "error";
  updatedAtMs: number; error: string | null;
}
export interface WorkPage { entries: WorkDocument[]; total: number; lastOpenedId: string | null }
export interface WorkDetail { document: WorkDocument; pdfBase64: string; snapshot: string | null }
export interface InboxCandidate { key: string; job: PrintJob; legacy: boolean }
export interface LegacyDraft { source: string; draft: ReviewDraft }

function object(value: unknown): value is Record<string, unknown> { return typeof value === "object" && value !== null && !Array.isArray(value); }
function strings(value: unknown): value is string[] { return Array.isArray(value) && value.every(v => typeof v === "string"); }
export function isReviewDraft(value: unknown): value is ReviewDraft {
  if (!object(value)) return false;
  if (!["invoiceNumber","issueDate","dueDate","serviceDate","currency","buyerReference"].every(k => typeof value[k] === "string")) return false;
  if (value.invoiceType !== undefined && (typeof value.invoiceType !== "string" || (value.invoiceType !== "386" && !isInvoiceTypeCode(value.invoiceType)))) return false;
  if (value.finalInvoice !== undefined && typeof value.finalInvoice !== "boolean") return false;
  if (value.prepaymentInvoice !== undefined && typeof value.prepaymentInvoice !== "boolean") return false;
  if (value.precedingInvoiceNumber !== undefined && typeof value.precedingInvoiceNumber !== "string") return false;
  if (value.precedingInvoiceDate !== undefined && typeof value.precedingInvoiceDate !== "string") return false;
  if (value.prepaidAmount !== undefined && typeof value.prepaidAmount !== "string") return false;
  if (value.precedingInvoices !== undefined) {
    if (!Array.isArray(value.precedingInvoices) || value.precedingInvoices.length > 50) return false;
    if (!value.precedingInvoices.every((item) => object(item) && typeof item.invoiceNumber === "string" && typeof item.issueDate === "string" && typeof item.paidAmount === "string")) return false;
  }
  const delivery = value.deliveryAddress;
  if (delivery !== undefined && (!object(delivery) || !["line1", "city", "postalCode", "countryCode"].every(k => typeof delivery[k] === "string"))) return false;
  for (const key of ["seller","buyer"]) {
    const party = value[key];
    if (!object(party) || !["name","addressLine1","postalCode","city","countryCode","vatId"].every(k => typeof party[k] === "string")) return false;
  }
  const seller = value.seller;
  if (!object(seller) || !["contactName", "phone", "email"].every(k => typeof seller[k] === "string")) return false;
  const allowance = (item: unknown) => object(item)
    && typeof item.id === "string"
    && typeof item.charge === "boolean"
    && typeof item.reason === "string"
    && typeof item.amount === "string"
    && typeof item.taxRate === "string"
    && (item.taxCase === undefined || typeof item.taxCase === "string")
    && (item.exemptionReason === undefined || typeof item.exemptionReason === "string")
    && (item.percent === undefined || typeof item.percent === "string");
  if (value.allowances !== undefined && (!Array.isArray(value.allowances) || value.allowances.length > 100 || !value.allowances.every(allowance))) return false;
  const payment = value.payment;
  return object(payment) && ["iban","bic","terms"].every(k => typeof payment[k] === "string")
    && Array.isArray(value.lines) && value.lines.length <= 10000 && value.lines.every(line => object(line)
      && ["id","description","quantity","unitCode","netUnitPrice","taxRate"].every(k => typeof line[k] === "string")
      && (line.taxCase === undefined || typeof line.taxCase === "string")
      && (line.exemptionReason === undefined || typeof line.exemptionReason === "string")
      && strings(line.sourceTokenIds)
      && (line.allowances === undefined || (Array.isArray(line.allowances) && line.allowances.length <= 20 && line.allowances.every(allowance))));
}
function extraction(value: unknown): value is ExtractionResult {
  if (!object(value) || !Array.isArray(value.pages) || !value.pages.length || value.pages.length > 1000
    || !Array.isArray(value.lines) || !object(value.fields) || !Array.isArray(value.lineItems) || !Array.isArray(value.warnings)
    || typeof value.usedOcr !== "boolean") return false;
  const box = (v: unknown) => object(v) && ["x","y","width","height"].every(k => typeof v[k] === "number" && Number.isFinite(v[k]));
  return value.pages.every(p => object(p) && typeof p.page === "number" && typeof p.width === "number" && p.width > 0
    && typeof p.height === "number" && p.height > 0 && Array.isArray(p.tokens) && p.tokens.every(t => object(t)
      && typeof t.id === "string" && typeof t.text === "string" && typeof t.page === "number" && box(t.box)))
    && value.lines.every(l => object(l) && typeof l.id === "string" && typeof l.text === "string" && typeof l.page === "number" && strings(l.tokenIds) && box(l.box))
    && Object.values(value.fields).every(f => object(f) && typeof f.value === "string" && typeof f.name === "string"
      && typeof f.sourceText === "string" && typeof f.confidence === "number" && strings(f.sourceTokenIds) && Array.isArray(f.transformations)
      && f.transformations.every(step => object(step) && ["operation", "input", "output"].every(k => typeof step[k] === "string")))
    && value.lineItems.every(l => object(l) && ["description","quantity","netUnitPrice","netAmount","sourceText"].every(k => typeof l[k] === "string") && strings(l.sourceTokenIds))
    && value.warnings.every(w => object(w) && typeof w.code === "string" && typeof w.message === "string");
}
export function parseWorkspaceSnapshot(contents: string): WorkspaceSnapshot {
  const v: unknown = JSON.parse(contents);
  if (!object(v) || v.schemaVersion !== 1 || v.extractionVersion !== "text-layout-v1"
    || !isReviewDraft(v.draft) || !isReviewDraft(v.initialDraft) || !extraction(v.extraction) || !extraction(v.sourceExtraction)
    || !object(v.sourceSelections) || !Object.entries(v.sourceSelections).every(([k, ids]) => LEARNABLE_FIELD_NAMES.includes(k as LearnableFieldName) && strings(ids))
    || !strings(v.pendingSourceFields) || !v.pendingSourceFields.every(k => LEARNABLE_FIELD_NAMES.includes(k as LearnableFieldName))
    || typeof v.completed !== "boolean") throw new Error("Der gespeicherte Entwurf ist beschädigt oder benötigt eine neuere Programmversion.");
  const snapshot = v as unknown as WorkspaceSnapshot;
  snapshot.draft = normalizeReviewDraft(snapshot.draft);
  snapshot.initialDraft = normalizeReviewDraft(snapshot.initialDraft);
  snapshot.hybridConfirmed = v.hybridConfirmed === true;
  const ids = new Set(v.extraction.pages.flatMap(p => p.tokens.map(t => t.id)));
  if (!Object.values(v.sourceSelections).every(tokens => (tokens as string[]).every(id => ids.has(id)))) throw new Error("Die gespeicherten Markierungen passen nicht zu dieser Rechnung.");
  return snapshot;
}
export function parseLegacyDraft(contents: string): LegacyDraft {
  if (contents.length > 5 * 1024 * 1024) throw new Error("Der Entwurf ist zu groß.");
  const v: unknown = JSON.parse(contents);
  if (!object(v) || v.schemaVersion !== 1 || typeof v.source !== "string" || !isReviewDraft(v.draft)) throw new Error("Diese Datei ist kein unterstützter Rechnungsentwurf.");
  return { source: v.source, draft: normalizeReviewDraft(v.draft) };
}
export function toBase64(contents: Uint8Array): string {
  let result = "";
  for (let i = 0; i < contents.length; i += 32768) result += String.fromCharCode(...contents.subarray(i,i+32768));
  return btoa(result);
}
export function fromBase64(value: string): Uint8Array { return Uint8Array.from(atob(value), c => c.charCodeAt(0)); }
export const workStore = {
  list: (offset = 0) => invoke<WorkPage>("workspace_list", { offset }),
  read: (id: string) => invoke<WorkDetail>("workspace_read", { id }),
  import: (name: string, pdf: Uint8Array) => invoke<WorkDocument>("workspace_import", { name, pdfBase64: toBase64(pdf) }),
  importPrint: (path: string) => invoke<WorkDocument>("workspace_import_print", { path }),
  activate: (id: string) => invoke<void>("workspace_activate", { id }),
  error: (id: string, message: string) => invoke<void>("workspace_error", { id, message }),
  scan: () => invoke<InboxCandidate[]>("workspace_scan_inbox"),
  save: (id: string, expectedRevision: number, contents: string) => invoke<WorkDocument>("workspace_save", { id, expectedRevision, contents }),
  remove: (id: string) => invoke<void>("workspace_delete", { id }),
  dismiss: (key: string) => invoke<void>("workspace_dismiss_inbox", { key }),
};

/** A single writer per active document. No older response can overwrite newer input. */
export class DraftWriter {
  private desired: string;
  private persisted: string;
  private running: Promise<void> | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;
  constructor(public document: WorkDocument, initial: string,
    private readonly save: typeof workStore.save,
    private readonly notify: (status: "saving" | "saved" | "error", message?: string) => void) {
    this.desired = initial; this.persisted = initial;
  }
  get dirty(): boolean { return this.desired !== this.persisted; }
  update(snapshot: WorkspaceSnapshot) {
    const next = JSON.stringify(snapshot);
    if (next === this.desired) return;
    this.desired = next;
    clearTimeout(this.timer);
    this.notify("saving");
    this.timer = setTimeout(() => { void this.flush().catch(() => undefined); }, 450);
  }
  flush(): Promise<void> {
    clearTimeout(this.timer);
    if (this.running) return this.running;
    this.running = this.drain().finally(() => { this.running = undefined; });
    return this.running;
  }
  private async drain() {
    try {
      while (this.dirty) {
        this.notify("saving");
        const contents = this.desired;
        this.document = await this.save(this.document.id,this.document.revision,contents);
        this.persisted = contents;
      }
      this.notify("saved");
    } catch (reason) {
      this.notify("error", reason instanceof Error ? reason.message : String(reason));
      throw reason;
    }
  }
  dispose() { clearTimeout(this.timer); }
}
