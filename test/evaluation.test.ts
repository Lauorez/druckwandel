import { describe, expect, it } from "vitest";
import { createCorpusReport, evaluateExtraction, type CorpusCase } from "../src/evaluation/corpus.js";
import type { ExtractionResult } from "../src/extraction/types.js";
import { detectUnsupportedCases } from "../src/policy/unsupported-cases.js";

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
  it("returns a sourced, stable code for blocked document types", () => {
    expect(detectUnsupportedCases(extraction)).toEqual([{
      code: "CREDIT_NOTE",
      message: "Gutschriften und Stornorechnungen werden noch nicht sicher unterstützt.",
      sourceTokenIds: ["credit"],
      sourceText: "GUTSCHRIFT",
    }]);
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
        unsupportedCaseCodes: ["CREDIT_NOTE"],
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
      expected: { fields: { invoiceNumber: "GS-1" }, unsupportedCaseCodes: ["CREDIT_NOTE"] },
    };
    const evaluation = evaluateExtraction(corpusCase, extraction);
    const report = createCorpusReport("manifest.json", [evaluation]);
    expect(report.passed).toBe(true);
    expect(report.totals).toEqual({ cases: 1, passedCases: 1, expectedFields: 1, matchedFields: 1, expectedLineItems: 0, matchedLineItems: 0 });
  });
});
