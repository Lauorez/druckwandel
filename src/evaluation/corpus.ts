import type { ExtractedFieldName, ExtractedLineItem, ExtractionResult } from "../extraction/types.js";
import { detectUnsupportedCases, type UnsupportedCaseCode } from "../policy/unsupported-cases.js";

export interface ExpectedLineItem extends Partial<Pick<
  ExtractedLineItem,
  "description" | "serviceDate" | "quantity" | "unit" | "netUnitPrice" | "netAmount" | "taxRate"
>> {}

export interface ExpectedExtraction {
  fields?: Partial<Record<ExtractedFieldName, string>>;
  absentFields?: ExtractedFieldName[];
  lineItems?: ExpectedLineItem[];
  warningCodes?: string[];
  unsupportedCaseCodes?: UnsupportedCaseCode[];
}

export interface CorpusCase {
  id: string;
  description: string;
  pdf: string;
  expected: ExpectedExtraction;
}

export interface CorpusManifest {
  schemaVersion: 1;
  cases: CorpusCase[];
}

export interface EvaluationCheck {
  path: string;
  expected: unknown;
  actual: unknown;
  passed: boolean;
}

export interface CaseEvaluation {
  id: string;
  description: string;
  passed: boolean;
  checks: EvaluationCheck[];
  metrics: {
    expectedFields: number;
    matchedFields: number;
    expectedLineItems: number;
    matchedLineItems: number;
  };
}

export interface CorpusEvaluationReport {
  schemaVersion: 1;
  generatedAt: string;
  manifestPath: string;
  cases: CaseEvaluation[];
  totals: {
    cases: number;
    passedCases: number;
    expectedFields: number;
    matchedFields: number;
    expectedLineItems: number;
    matchedLineItems: number;
  };
  passed: boolean;
}

function sorted(values: string[]): string[] {
  return [...values].sort((left, right) => left.localeCompare(right));
}

function sameStringArray(left: string[], right: string[]): boolean {
  return JSON.stringify(sorted(left)) === JSON.stringify(sorted(right));
}

export function evaluateExtraction(corpusCase: CorpusCase, extraction: ExtractionResult): CaseEvaluation {
  const checks: EvaluationCheck[] = [];
  let expectedFields = 0;
  let matchedFields = 0;
  let expectedLineItems = 0;
  let matchedLineItems = 0;

  for (const [name, expected] of Object.entries(corpusCase.expected.fields ?? {}) as Array<[ExtractedFieldName, string]>) {
    const actual = extraction.fields[name]?.value;
    const passed = actual === expected;
    checks.push({ path: `fields.${name}`, expected, actual, passed });
    expectedFields += 1;
    if (passed) matchedFields += 1;
  }
  for (const name of corpusCase.expected.absentFields ?? []) {
    const actual = extraction.fields[name]?.value;
    const passed = actual === undefined;
    checks.push({ path: `absentFields.${name}`, expected: undefined, actual, passed });
  }

  if (corpusCase.expected.lineItems) {
    checks.push({
      path: "lineItems.length",
      expected: corpusCase.expected.lineItems.length,
      actual: extraction.lineItems.length,
      passed: extraction.lineItems.length === corpusCase.expected.lineItems.length,
    });
    corpusCase.expected.lineItems.forEach((expectedLine, index) => {
      expectedLineItems += 1;
      const actualLine = extraction.lineItems[index];
      const lineChecks = Object.entries(expectedLine).map(([property, expected]) => {
        const actual = actualLine?.[property as keyof ExpectedLineItem];
        return { path: `lineItems.${index}.${property}`, expected, actual, passed: actual === expected };
      });
      checks.push(...lineChecks);
      if (actualLine && lineChecks.every((check) => check.passed)) matchedLineItems += 1;
    });
  }

  if (corpusCase.expected.warningCodes) {
    const actual = extraction.warnings.map((warning) => warning.code);
    checks.push({
      path: "warningCodes",
      expected: sorted(corpusCase.expected.warningCodes),
      actual: sorted(actual),
      passed: sameStringArray(corpusCase.expected.warningCodes, actual),
    });
  }
  if (corpusCase.expected.unsupportedCaseCodes) {
    const actual = detectUnsupportedCases(extraction).map((unsupportedCase) => unsupportedCase.code);
    checks.push({
      path: "unsupportedCaseCodes",
      expected: sorted(corpusCase.expected.unsupportedCaseCodes),
      actual: sorted(actual),
      passed: sameStringArray(corpusCase.expected.unsupportedCaseCodes, actual),
    });
  }

  return {
    id: corpusCase.id,
    description: corpusCase.description,
    passed: checks.every((check) => check.passed),
    checks,
    metrics: { expectedFields, matchedFields, expectedLineItems, matchedLineItems },
  };
}

export function createCorpusReport(manifestPath: string, cases: CaseEvaluation[]): CorpusEvaluationReport {
  const totals = cases.reduce((result, corpusCase) => ({
    cases: result.cases + 1,
    passedCases: result.passedCases + (corpusCase.passed ? 1 : 0),
    expectedFields: result.expectedFields + corpusCase.metrics.expectedFields,
    matchedFields: result.matchedFields + corpusCase.metrics.matchedFields,
    expectedLineItems: result.expectedLineItems + corpusCase.metrics.expectedLineItems,
    matchedLineItems: result.matchedLineItems + corpusCase.metrics.matchedLineItems,
  }), { cases: 0, passedCases: 0, expectedFields: 0, matchedFields: 0, expectedLineItems: 0, matchedLineItems: 0 });

  return {
    schemaVersion: 1,
    generatedAt: new Date().toISOString(),
    manifestPath,
    cases,
    totals,
    passed: totals.passedCases === totals.cases,
  };
}
