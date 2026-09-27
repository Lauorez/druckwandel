import type { ExtractionResult, TextLine } from "../extraction/types.js";

export type UnsupportedCaseCode =
  | "ROUNDING_ADJUSTMENT"
  | "NEGATIVE_LINE"
  | "OCR_REQUIRED";

export interface UnsupportedCase {
  code: UnsupportedCaseCode;
  message: string;
  sourceTokenIds: string[];
  sourceText: string;
}

interface TextRule {
  code: UnsupportedCaseCode;
  pattern: RegExp;
  message: string;
}

const TEXT_RULES: TextRule[] = [
  { code: "ROUNDING_ADJUSTMENT", pattern: /\b(?:rundungsdifferenz|rundungsausgleich)\b/i, message: "Zusätzliche Beträge zum Ausgleichen von Rundungen werden noch nicht unterstützt." },
];

function firstMatchingLine(lines: TextLine[], pattern: RegExp): TextLine | undefined {
  return lines.find((line) => pattern.test(line.text));
}

export function detectUnsupportedCases(extraction: ExtractionResult): UnsupportedCase[] {
  const cases: UnsupportedCase[] = [];
  for (const rule of TEXT_RULES) {
    const line = firstMatchingLine(extraction.lines, rule.pattern);
    if (line) cases.push({ code: rule.code, message: rule.message, sourceTokenIds: line.tokenIds, sourceText: line.text });
  }

  const negativeLine = extraction.lineItems.find((line) =>
    [line.quantity, line.netUnitPrice, line.netAmount].some((value) => value.trim().startsWith("-")));
  if (negativeLine) {
    cases.push({
      code: "NEGATIVE_LINE",
      message: "Negative Rechnungspositionen werden noch nicht unterstützt. Nachlässe bitte als Zu-/Abschlag erfassen.",
      sourceTokenIds: negativeLine.sourceTokenIds,
      sourceText: negativeLine.sourceText,
    });
  }

  const ocrWarning = extraction.warnings.find((warning) => warning.code === "OCR_REQUIRED");
  if (ocrWarning) {
    cases.push({
      code: "OCR_REQUIRED",
      message: "Der Text dieser eingescannten Rechnung konnte nicht gelesen werden.",
      sourceTokenIds: [],
      sourceText: ocrWarning.message,
    });
  }
  return cases;
}
