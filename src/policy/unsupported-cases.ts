import type { ExtractionResult, TextLine } from "../extraction/types.js";

export type UnsupportedCaseCode =
  | "CREDIT_NOTE"
  | "REVERSE_CHARGE"
  | "TAX_EXEMPTION"
  | "ALLOWANCE_OR_DISCOUNT"
  | "ADVANCE_OR_FINAL_INVOICE"
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
  { code: "CREDIT_NOTE", pattern: /\b(?:gutschrift|stornorechnung|credit\s+note)\b/i, message: "Gutschriften und Stornorechnungen werden noch nicht sicher unterstützt." },
  { code: "REVERSE_CHARGE", pattern: /\b(?:reverse\s+charge|steuerschuldnerschaft\s+des\s+leistungsempfängers)\b|§\s*13b\s*ustg/i, message: "Rechnungen, bei denen der Empfänger die Umsatzsteuer schuldet, werden noch nicht unterstützt." },
  { code: "TAX_EXEMPTION", pattern: /\b(?:steuerfrei|steuerbefreit|tax\s+exempt)\b|§\s*4\s*ustg/i, message: "Rechnungen ohne Umsatzsteuer brauchen zusätzliche Angaben und werden noch nicht unterstützt." },
  { code: "ALLOWANCE_OR_DISCOUNT", pattern: /\b(?:rabatt|nachlass|rechnungsabschlag|bonus)\b/i, message: "Rabatte oder Zuschläge auf die gesamte Rechnung werden noch nicht unterstützt." },
  { code: "ADVANCE_OR_FINAL_INVOICE", pattern: /\b(?:abschlagsrechnung|schlussrechnung|anzahlungsrechnung|vorauszahlungsrechnung)\b/i, message: "Abschlags-, Anzahlungs- und Schlussrechnungen werden noch nicht unterstützt." },
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
      message: "Negative Rechnungspositionen werden noch nicht unterstützt.",
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
