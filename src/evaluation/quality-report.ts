import type { CaseEvaluation } from "./corpus.js";

export const QUALITY_REPORT_DISCLAIMER =
  "Synthetische Treffer und Referenzfälle sind keine gemessene Kundenquote.";

export interface QualityFinding {
  caseId: string;
  path: string;
  expected: unknown;
  actual: unknown;
}

export interface LocalQualityReport {
  schemaVersion: 1;
  generatedAt: string;
  disclaimer: string;
  fieldAccuracy: { expected: number; matched: number; rate: number };
  lineItemAccuracy: { expected: number; matched: number; rate: number };
  misassignments: QualityFinding[];
  blocks: Array<{ caseId: string; codes: string[] }>;
  neededCorrections: QualityFinding[];
  totals: { cases: number; passedCases: number; failedCases: number };
}

function rate(matched: number, expected: number): number {
  return expected === 0 ? 1 : Number((matched / expected).toFixed(4));
}

export function createQualityReport(
  evaluations: CaseEvaluation[],
  generatedAt = new Date().toISOString(),
): LocalQualityReport {
  const fieldExpected = evaluations.reduce((sum, item) => sum + item.metrics.expectedFields, 0);
  const fieldMatched = evaluations.reduce((sum, item) => sum + item.metrics.matchedFields, 0);
  const lineExpected = evaluations.reduce((sum, item) => sum + item.metrics.expectedLineItems, 0);
  const lineMatched = evaluations.reduce((sum, item) => sum + item.metrics.matchedLineItems, 0);
  const neededCorrections = evaluations.flatMap((item) => item.checks
    .filter((check) => !check.passed)
    .map((check) => ({ caseId: item.id, path: check.path, expected: check.expected, actual: check.actual })));
  const misassignments = neededCorrections.filter((finding) => finding.path.startsWith("fields.") && finding.actual !== undefined);
  const blocks = evaluations.flatMap((item) => {
    const check = item.checks.find((candidate) => candidate.path === "unsupportedCaseCodes");
    const codes = Array.isArray(check?.actual)
      ? check.actual.filter((code): code is string => typeof code === "string")
      : [];
    return codes.length > 0 ? [{ caseId: item.id, codes }] : [];
  });
  const passedCases = evaluations.filter((item) => item.passed).length;
  return {
    schemaVersion: 1,
    generatedAt,
    disclaimer: QUALITY_REPORT_DISCLAIMER,
    fieldAccuracy: { expected: fieldExpected, matched: fieldMatched, rate: rate(fieldMatched, fieldExpected) },
    lineItemAccuracy: { expected: lineExpected, matched: lineMatched, rate: rate(lineMatched, lineExpected) },
    misassignments,
    blocks,
    neededCorrections,
    totals: { cases: evaluations.length, passedCases, failedCases: evaluations.length - passedCases },
  };
}
