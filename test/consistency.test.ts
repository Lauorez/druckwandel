import { describe, expect, it } from "vitest";
import { calculateInvoice } from "../src/domain/calculate.js";
import { assertExportableConsistency, compareInvoiceToSource } from "../src/engine/consistency.js";
import { analyzeDocumentPages } from "../src/extraction/analyze.js";
import { reviewDraftFromExtraction } from "../src/review/draft.js";
import { standardInvoice } from "./fixtures/invoice.js";

describe("invoice content consistency", () => {
  it("treats matching extracted totals as confirmed matches", () => {
    const extraction = analyzeDocumentPages([{ page: 1, width: 600, height: 800, tokens: [] }]);
    const draft = reviewDraftFromExtraction(extraction);
    draft.invoiceNumber = "R-1";
    extraction.fields.invoiceNumber = { name: "invoiceNumber", value: "R-1", confidence: 1, sourceTokenIds: [], sourceText: "R-1", transformations: [] };
    extraction.fields.payable = { name: "payable", value: "282.08", confidence: 1, sourceTokenIds: [], sourceText: "282.08", transformations: [] };
    const calculated = calculateInvoice(standardInvoice);
    draft.invoiceNumber = calculated.invoiceNumber;
    extraction.fields.invoiceNumber.value = calculated.invoiceNumber;
    const result = compareInvoiceToSource(extraction, draft, calculated);
    expect(result.mismatches).toEqual([]);
    expect(result.items.some((item) => item.path === "totals.payable" && item.kind === "match")).toBe(true);
  });

  it("blocks unresolved amount mismatches and requires confirmation for supplements", () => {
    const extraction = analyzeDocumentPages([{ page: 1, width: 600, height: 800, tokens: [] }]);
    const draft = reviewDraftFromExtraction(extraction);
    const calculated = calculateInvoice(standardInvoice);
    extraction.fields.payable = { name: "payable", value: "10.00", confidence: 1, sourceTokenIds: [], sourceText: "10,00", transformations: [] };
    draft.invoiceNumber = calculated.invoiceNumber;
    const result = compareInvoiceToSource(extraction, draft, calculated);
    expect(result.blocked).toBe(true);
    expect(() => assertExportableConsistency(result, true)).toThrow(/weichen/);
    delete extraction.fields.payable;
    const supplemented = compareInvoiceToSource(extraction, { ...draft, invoiceNumber: "NEU-1" }, calculated);
    expect(supplemented.blocked).toBe(false);
    expect(supplemented.supplemented.some((item) => item.path === "invoiceNumber")).toBe(true);
    expect(() => assertExportableConsistency(supplemented, false)).toThrow(/bestätigen/);
    expect(() => assertExportableConsistency(supplemented, true)).not.toThrow();
  });
});
