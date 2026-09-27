import { describe, expect, it } from "vitest";
import { calculateInvoice } from "../src/domain/calculate.js";
import { assertExportableConsistency, compareInvoiceToSource } from "../src/engine/consistency.js";
import { analyzeDocumentPages } from "../src/extraction/analyze.js";
import { calculateReviewDraft, reviewDraftFromExtraction } from "../src/review/draft.js";
import type { ExtractionResult } from "../src/extraction/types.js";
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

  it("allows export when the PDF net is the position sum and a document Rabatt is applied", () => {
    const box = { x: 10, y: 10, width: 80, height: 10 };
    const extraction: ExtractionResult = {
      pages: [{ page: 1, width: 500, height: 700, tokens: [] }],
      lines: [
        { id: "l1", page: 1, text: "Leistung 100,00", box, tokenIds: ["t1"] },
        { id: "l2", page: 1, text: "Rabatt 10 %: 10,00 EUR", box, tokenIds: ["t2"] },
      ],
      fields: {
        invoiceNumber: { name: "invoiceNumber", value: "RE-1", confidence: 1, sourceTokenIds: [], sourceText: "RE-1", transformations: [] },
        lineNet: { name: "lineNet", value: "100.00", confidence: 1, sourceTokenIds: [], sourceText: "100,00", transformations: [] },
        taxTotal: { name: "taxTotal", value: "17.10", confidence: 1, sourceTokenIds: [], sourceText: "17,10", transformations: [] },
        payable: { name: "payable", value: "107.10", confidence: 1, sourceTokenIds: [], sourceText: "107,10", transformations: [] },
      },
      lineItems: [{
        description: "Leistung",
        quantity: "1",
        netUnitPrice: "100.00",
        netAmount: "100.00",
        taxRate: "19",
        confidence: 1,
        sourceTokenIds: ["t1"],
        sourceText: "Leistung 100,00",
      }],
      warnings: [],
      usedOcr: false,
    };
    const draft = reviewDraftFromExtraction(extraction);
    const calculated = calculateReviewDraft(draft);
    expect(calculated?.totals).toMatchObject({
      lineNet: "100.00",
      taxExclusive: "90.00",
      taxTotal: "17.10",
      payable: "107.10",
    });
    const result = compareInvoiceToSource(extraction, draft, calculated);
    expect(result.mismatches).toEqual([]);
    expect(result.blocked).toBe(false);
    expect(() => assertExportableConsistency(result, true)).not.toThrow();
  });

  it("still blocks a net that matches neither the position sum nor the amount after allowances", () => {
    const extraction = analyzeDocumentPages([{ page: 1, width: 600, height: 800, tokens: [] }]);
    const draft = reviewDraftFromExtraction(extraction);
    const calculated = calculateInvoice({
      ...standardInvoice,
      allowances: [{ charge: false, amount: "10.00", reason: "Rabatt", tax: { categoryCode: "S", rate: "19" } }],
    });
    extraction.fields.lineNet = { name: "lineNet", value: "50.00", confidence: 1, sourceTokenIds: [], sourceText: "50,00", transformations: [] };
    draft.invoiceNumber = calculated.invoiceNumber;
    const result = compareInvoiceToSource(extraction, draft, calculated);
    expect(result.blocked).toBe(true);
    expect(result.mismatches.some((item) => item.path === "totals.lineNet")).toBe(true);
  });
});
