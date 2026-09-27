import { describe, expect, it } from "vitest";
import { detectUnsupportedCases } from "../src/policy/unsupported-cases.js";
import { detectTaxCase } from "../src/policy/tax-cases.js";
import { detectDocumentAllowances, detectDocumentKind } from "../src/policy/document-kind.js";
import { createCorpusReport, evaluateExtraction, type CorpusCase } from "../src/evaluation/corpus.js";
import { createQualityReport } from "../src/evaluation/quality-report.js";
import type { ExtractionResult } from "../src/extraction/types.js";

const extraction: ExtractionResult = {
  pages: [{ page: 1, width: 500, height: 700, tokens: [
    { id: "credit", page: 1, text: "GUTSCHRIFT", box: { x: 10, y: 10, width: 80, height: 10 }, origin: "text-layer" },
  ] }],
  lines: [{ id: "line-1", page: 1, text: "GUTSCHRIFT", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["credit"] }],
  fields: {
    invoiceNumber: { name: "invoiceNumber", value: "GS-1", confidence: 0.9, sourceTokenIds: ["credit"], sourceText: "GUTSCHRIFT GS-1", transformations: [] },
  },
  lineItems: [],
  warnings: [],
  usedOcr: false,
};

describe("unsupported case policy", () => {
  it("does not block a credit note or the word discount", () => {
    expect(detectUnsupportedCases(extraction)).toEqual([]);
    expect(detectDocumentKind(extraction)?.invoiceType).toBe("381");
    const discount: ExtractionResult = {
      ...extraction,
      lines: [{ id: "line-1", page: 1, text: "Rabatt 10 %: 10,00 EUR", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["credit"] }],
    };
    expect(detectUnsupportedCases(discount)).toEqual([]);
    expect(detectDocumentAllowances(discount)).toEqual([expect.objectContaining({ reason: "Rabatt", amount: "10.00", percent: "10" })]);
  });

  it("does not block an advance or final invoice and reads its preceding amounts", () => {
    const finalInvoice: ExtractionResult = {
      ...extraction,
      lines: [
        { id: "line-1", page: 1, text: "Schlussrechnung", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["credit"] },
        { id: "line-2", page: 1, text: "Abschlag 1: RE-A-1 vom 01.07.2026 100,00", box: { x: 10, y: 30, width: 180, height: 10 }, tokenIds: ["prev"] },
      ],
    };
    expect(detectUnsupportedCases(finalInvoice)).toEqual([]);
    expect(detectDocumentKind(finalInvoice)).toMatchObject({ invoiceType: "380", finalInvoice: true, prepaidAmount: "100.00" });
    expect(detectDocumentKind(finalInvoice)?.precedingInvoices[0]).toMatchObject({ invoiceNumber: "RE-A-1", paidAmount: "100.00" });
    const partial: ExtractionResult = {
      ...extraction,
      lines: [{ id: "line-1", page: 1, text: "Abschlagsrechnung", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["credit"] }],
    };
    expect(detectDocumentKind(partial)?.invoiceType).toBe("326");
    const prepayment: ExtractionResult = {
      ...extraction,
      lines: [{ id: "line-1", page: 1, text: "Anzahlungsrechnung", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["credit"] }],
    };
    expect(detectDocumentKind(prepayment)).toMatchObject({ invoiceType: "380", prepaymentInvoice: true, finalInvoice: false });
  });

  it("does not block reverse charge or tax-exemption wording", () => {
    const reverse: ExtractionResult = {
      ...extraction,
      lines: [{ id: "line-1", page: 1, text: "Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["credit"] }],
    };
    expect(detectUnsupportedCases(reverse)).toEqual([]);
    expect(detectTaxCase(reverse)?.id).toBe("AE");
    const exempt: ExtractionResult = {
      ...extraction,
      lines: [{ id: "line-1", page: 1, text: "steuerfrei gemäß § 4 UStG", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["credit"] }],
    };
    expect(detectUnsupportedCases(exempt)).toEqual([]);
    expect(detectTaxCase(exempt)?.id).toBe("E");
  });

  it("blocks a document that requires OCR", () => {
    const result = detectUnsupportedCases({
      ...extraction,
      lines: [],
      warnings: [{ code: "OCR_REQUIRED", message: "OCR erforderlich." }],
    });
    expect(result.map((unsupportedCase) => unsupportedCase.code)).toEqual(["OCR_REQUIRED"]);
  });
});

describe("ground-truth evaluation", () => {
  it("reports exact field and policy mismatches with machine-readable paths", () => {
    const corpusCase: CorpusCase = {
      id: "credit",
      description: "Credit note",
      pdf: "credit.pdf",
      expected: {
        fields: { invoiceNumber: "GS-2" },
        lineItems: [],
        warningCodes: [],
        unsupportedCaseCodes: [],
      },
    };
    const evaluation = evaluateExtraction(corpusCase, extraction);
    expect(evaluation.passed).toBe(false);
    expect(evaluation.metrics).toEqual({ expectedFields: 1, matchedFields: 0, expectedLineItems: 0, matchedLineItems: 0 });
    expect(evaluation.checks.find((check) => check.path === "fields.invoiceNumber")).toMatchObject({ expected: "GS-2", actual: "GS-1", passed: false });
  });

  it("aggregates pass counts and extraction accuracy", () => {
    const corpusCase: CorpusCase = {
      id: "credit",
      description: "Credit note",
      pdf: "credit.pdf",
      expected: { fields: { invoiceNumber: "GS-1" }, unsupportedCaseCodes: [] },
    };
    const evaluation = evaluateExtraction(corpusCase, extraction);
    const report = createCorpusReport("manifest.json", [evaluation]);
    expect(report.passed).toBe(true);
    expect(report.totals).toEqual({ cases: 1, passedCases: 1, expectedFields: 1, matchedFields: 1, expectedLineItems: 0, matchedLineItems: 0 });
  });

  it("builds a local quality report with accuracy, blocks and needed corrections", () => {
    const blockedExtraction: ExtractionResult = {
      ...extraction,
      warnings: [{ code: "OCR_REQUIRED", message: "OCR erforderlich." }],
    };
    const failed: CorpusCase = {
      id: "scan",
      description: "Scan",
      pdf: "scan.pdf",
      expected: {
        fields: { invoiceNumber: "GS-2" },
        lineItems: [],
        warningCodes: [],
        unsupportedCaseCodes: ["OCR_REQUIRED"],
      },
    };
    const quality = createQualityReport([evaluateExtraction(failed, blockedExtraction)]);
    expect(quality.disclaimer).toMatch(/keine gemessene Kundenquote/);
    expect(quality.fieldAccuracy).toEqual({ expected: 1, matched: 0, rate: 0 });
    expect(quality.misassignments).toEqual([expect.objectContaining({ caseId: "scan", path: "fields.invoiceNumber", expected: "GS-2", actual: "GS-1" })]);
    expect(quality.blocks).toEqual([{ caseId: "scan", codes: ["OCR_REQUIRED"] }]);
    expect(quality.neededCorrections.length).toBeGreaterThan(0);
  });
});
