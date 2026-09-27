import { afterEach, describe, expect, it, vi } from "vitest";
import { DraftWriter, parseLegacyDraft, parseWorkspaceSnapshot, type WorkDocument, type WorkspaceSnapshot } from "../apps/desktop/src/workspaceStore.js";
import { analyzeDocumentPages } from "../src/extraction/analyze.js";
import { reviewDraftFromExtraction } from "../src/review/draft.js";

function fixture(): WorkspaceSnapshot {
  const extraction = analyzeDocumentPages([{ page: 1,width: 600,height: 800,tokens: [] }]);
  const draft = reviewDraftFromExtraction(extraction);
  return { schemaVersion: 1,extractionVersion: "text-layout-v1",sourceExtraction: extraction,extraction,draft,initialDraft: draft,sourceSelections: {},pendingSourceFields: [],completed: false,hybridConfirmed: false };
}
const document: WorkDocument = { id: "test",name: "invoice.pdf",sourceKey: null,jobId: null,originalSha256: "test",revision: 1,status: "draft",updatedAtMs: 0,error: null };
const change = (s: WorkspaceSnapshot, name: string) => ({ ...s,draft: { ...s.draft,invoiceNumber: name } });
describe("persistent draft boundary", () => {
  afterEach(() => vi.useRealTimers());
  it("round trips snapshots, rejects corrupt versions and foreign token selections", () => {
    const source = fixture();
    expect(parseWorkspaceSnapshot(JSON.stringify(source))).toEqual(source);
    expect(() => parseWorkspaceSnapshot(JSON.stringify({ ...source,schemaVersion: 2 }))).toThrow();
    expect(() => parseWorkspaceSnapshot(JSON.stringify({ ...source,draft: { lines: [] } }))).toThrow();
    expect(() => parseWorkspaceSnapshot(JSON.stringify({ ...source,sourceSelections: { sellerName: ["foreign"] } }))).toThrow();
    expect(() => parseWorkspaceSnapshot(JSON.stringify({ ...source,extraction: { ...source.extraction,pages: [{}] } }))).toThrow();
  });
  it("imports old draft values without guessing the original or reusing old marks", () => {
    const s = fixture();
    expect(parseLegacyDraft(JSON.stringify({ schemaVersion: 1,source: "original.pdf",draft: s.draft,sourceSelections: { sellerName: ["old"] } })))
      .toEqual({ source: "original.pdf",draft: s.draft });
    expect(() => parseLegacyDraft('{"schemaVersion":1,"draft":{}}')).toThrow();
  });
  it("rejects corrupt seller contacts and transformation entries before rendering", () => {
    const s = fixture();
    expect(() => parseWorkspaceSnapshot(JSON.stringify({ ...s, draft: { ...s.draft, seller: { ...s.draft.seller, email: null } } }))).toThrow();
    s.extraction.fields.invoiceNumber = { name: "invoiceNumber", value: "RE1", confidence: 1, sourceTokenIds: [], sourceText: "RE1", transformations: [] };
    const damaged = JSON.parse(JSON.stringify(s));
    damaged.extraction.fields.invoiceNumber.transformations = [null];
    expect(() => parseWorkspaceSnapshot(JSON.stringify(damaged))).toThrow();
  });
  it("migrates old drafts without document type or allowances", () => {
    const s = fixture();
    const { invoiceType:_t, precedingInvoiceNumber:_n, precedingInvoiceDate:_d, precedingInvoices:_p, prepaidAmount:_pre, finalInvoice:_f, prepaymentInvoice:_prepay, allowances:_a, ...legacyDraft } = s.draft;
    const old = { ...legacyDraft, lines: s.draft.lines.map(({ allowances:_ignored, ...line }) => line) };
    const parsed = parseWorkspaceSnapshot(JSON.stringify({ ...s, draft: old, initialDraft: old }));
    expect(parsed.draft.invoiceType).toBe("380");
    expect(parsed.draft.precedingInvoiceNumber).toBe("");
    expect(parsed.draft.finalInvoice).toBe(false);
    expect(parsed.draft.prepaymentInvoice).toBe(false);
    expect(parsed.draft.prepaidAmount).toBe("");
    expect(parsed.draft.precedingInvoices).toEqual([]);
    expect(parsed.draft.allowances).toEqual([]);
    expect(parsed.draft.lines.every(line => Array.isArray(line.allowances))).toBe(true);
  });
  it("migrates 19 % to the standard tax case and does not guess 0 %", () => {
    const s = fixture();
    const base = { id: "1", description: "Leistung", quantity: "1", unitCode: "C62", netUnitPrice: "100", sourceTokenIds: [] as string[] };
    const zero = { ...s.draft, lines: [{ ...base, taxRate: "0" }] };
    const parsedZero = parseWorkspaceSnapshot(JSON.stringify({ ...s, draft: zero, initialDraft: zero }));
    expect(parsedZero.draft.lines[0]?.taxCase).toBe("");
    const nineteen = { ...s.draft, lines: [{ ...base, taxRate: "19" }] };
    const parsed19 = parseWorkspaceSnapshot(JSON.stringify({ ...s, draft: nineteen, initialDraft: nineteen }));
    expect(parsed19.draft.lines[0]).toMatchObject({ taxCase: "S19", taxRate: "19", exemptionReason: "" });
  });
  it("serializes saves and drains newer changes before a document switch", async () => {
    vi.useFakeTimers();
    const s = fixture();
    let release!: () => void;
    const wait = new Promise<void>(resolve => { release = resolve; });
    const save = vi.fn(async (_id: string,revision: number,_contents: string) => { if (revision === 1) await wait; return { ...document,revision: revision+1 }; });
    const notify = vi.fn();
    const writer = new DraftWriter(document,JSON.stringify(s),save,notify);
    writer.update(change(s,"A"));
    const pending = writer.flush();
    writer.update(change(s,"B"));
    expect(save).toHaveBeenCalledTimes(1);
    release(); await pending;
    expect(save).toHaveBeenCalledTimes(2);
    expect(save.mock.calls.map(call => call[1])).toEqual([1,2]);
    expect(JSON.parse(save.mock.calls[1]![2]).draft.invoiceNumber).toBe("B");
    expect(writer.dirty).toBe(false);
    expect(notify).toHaveBeenLastCalledWith("saved");
    writer.dispose();
  });
  it("keeps failed writes dirty and retries against the last committed revision", async () => {
    vi.useFakeTimers();
    const s = fixture();
    const save = vi.fn().mockRejectedValueOnce(new Error("disk full")).mockResolvedValueOnce({ ...document,revision: 2 });
    const notify = vi.fn();
    const writer = new DraftWriter(document,JSON.stringify(s),save,notify);
    writer.update(change(s,"unsaved"));
    await expect(writer.flush()).rejects.toThrow("disk full");
    expect(writer.dirty).toBe(true);
    expect(notify).toHaveBeenLastCalledWith("error","disk full");
    await writer.flush();
    expect(save.mock.calls.map(call => call[1])).toEqual([1,1]);
    expect(writer.dirty).toBe(false);
    writer.dispose();
  });
});
