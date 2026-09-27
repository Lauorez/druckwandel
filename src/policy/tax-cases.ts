import type { TaxCategoryCode } from "../domain/types.js";
import type { ExtractionResult, TextLine } from "../extraction/types.js";

export type ReviewTaxCaseId = "S19" | "S7" | "AE" | "E" | "K" | "Z";

export interface TaxCase {
  id: ReviewTaxCaseId;
  categoryCode: TaxCategoryCode;
  rate: string;
  label: string;
  shortLabel: string;
  exemptionReasonRequired: boolean;
  defaultExemptionReason?: string;
  exemptionReasonCode?: string;
  requireSellerVatId: boolean;
  requireBuyerVatId: boolean;
}

/**
 * Freigegebene Steuerfälle für WP11b. 0 % allein ist kein Fall:
 * Reverse Charge, Steuerbefreiung und innergemeinschaftliche Lieferung
 * müssen ausdrücklich gewählt werden.
 */
export const TAX_CASES: readonly TaxCase[] = [
  { id: "S19", categoryCode: "S", rate: "19", label: "19 % Umsatzsteuer", shortLabel: "19 %", exemptionReasonRequired: false, requireSellerVatId: false, requireBuyerVatId: false },
  { id: "S7", categoryCode: "S", rate: "7", label: "7 % Umsatzsteuer", shortLabel: "7 %", exemptionReasonRequired: false, requireSellerVatId: false, requireBuyerVatId: false },
  {
    id: "AE",
    categoryCode: "AE",
    rate: "0",
    label: "Reverse Charge (§ 13b UStG)",
    shortLabel: "Reverse Charge",
    exemptionReasonRequired: true,
    defaultExemptionReason: "Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG",
    exemptionReasonCode: "VATEX-EU-AE",
    requireSellerVatId: true,
    requireBuyerVatId: true,
  },
  {
    id: "E",
    categoryCode: "E",
    rate: "0",
    label: "Steuerfrei (§ 4 UStG)",
    shortLabel: "Steuerfrei",
    exemptionReasonRequired: true,
    requireSellerVatId: false,
    requireBuyerVatId: false,
  },
  {
    id: "K",
    categoryCode: "K",
    rate: "0",
    label: "Innergemeinschaftliche Lieferung",
    shortLabel: "Innergem. Lieferung",
    exemptionReasonRequired: true,
    defaultExemptionReason: "Innergemeinschaftliche Lieferung",
    exemptionReasonCode: "VATEX-EU-IC",
    requireSellerVatId: true,
    requireBuyerVatId: true,
  },
  { id: "Z", categoryCode: "Z", rate: "0", label: "0 % steuerbar", shortLabel: "0 % steuerbar", exemptionReasonRequired: false, requireSellerVatId: false, requireBuyerVatId: false },
];

export interface DetectedTaxCase {
  id: ReviewTaxCaseId;
  sourceTokenIds: string[];
  sourceText: string;
  suggestedReason: string;
}

const REVERSE_CHARGE = /\b(?:reverse\s+charge|steuerschuldnerschaft\s+des\s+leistungsempfängers)\b|§\s*13b\s*ustg/i;
const INTRA_COMMUNITY = /\b(?:innergemeinschaftliche\s+lieferung|ig\s*lieferung|intra-?community)\b/i;
const EXEMPT = /\b(?:steuerfrei|steuerbefreit|tax\s+exempt)\b|§\s*4(?:\s*nr\.?\s*\d+)?\s*ustg/i;

function firstMatchingLine(lines: TextLine[], pattern: RegExp): TextLine | undefined {
  return lines.find((line) => pattern.test(line.text));
}

export function taxCaseById(id: string | undefined): TaxCase | undefined {
  return TAX_CASES.find((item) => item.id === id);
}

export function isZeroRate(value: string): boolean {
  return /^0+(?:\.0+)?$/.test(value.trim());
}

/** Alte Entwürfe: 19/7 bleiben Standardsteuer. 0 % wird nicht geraten. */
export function taxCaseFromLegacy(taxRate: string, taxCase?: string, categoryCode?: string): ReviewTaxCaseId | "" {
  if (taxCase && TAX_CASES.some((item) => item.id === taxCase)) return taxCase as ReviewTaxCaseId;
  if (categoryCode === "AE") return "AE";
  if (categoryCode === "E") return "E";
  if (categoryCode === "K") return "K";
  if (categoryCode === "Z") return "Z";
  const normalized = taxRate.trim().replace(",", ".");
  if (normalized === "19" || normalized === "19.0" || normalized === "19.00") return "S19";
  if (normalized === "7" || normalized === "7.0" || normalized === "7.00") return "S7";
  return "";
}

export function applyReviewTaxCase(id: string, previousReason = ""): { taxCase: ReviewTaxCaseId | ""; taxRate: string; exemptionReason: string } {
  const selected = taxCaseById(id);
  if (!selected) return { taxCase: "", taxRate: "0", exemptionReason: "" };
  const canned = TAX_CASES.map((item) => item.defaultExemptionReason).filter((value): value is string => Boolean(value));
  let exemptionReason = "";
  if (selected.defaultExemptionReason) exemptionReason = selected.defaultExemptionReason;
  else if (selected.exemptionReasonRequired) exemptionReason = canned.includes(previousReason) ? "" : previousReason;
  return { taxCase: selected.id, taxRate: selected.rate, exemptionReason };
}

export function taxTreatmentFromCase(id: ReviewTaxCaseId | "", taxRate: string, exemptionReason: string): { categoryCode: TaxCategoryCode; rate: string; exemptionReason?: string; exemptionReasonCode?: string } {
  const selected = taxCaseById(id);
  if (!selected) return { categoryCode: "S", rate: taxRate || "0" };
  return {
    categoryCode: selected.categoryCode,
    rate: selected.rate,
    ...(selected.exemptionReasonRequired && exemptionReason.trim() ? { exemptionReason: exemptionReason.trim() } : {}),
    ...(selected.exemptionReasonCode ? { exemptionReasonCode: selected.exemptionReasonCode } : {}),
  };
}

export function detectTaxCase(extraction: ExtractionResult): DetectedTaxCase | undefined {
  const reverse = firstMatchingLine(extraction.lines, REVERSE_CHARGE);
  if (reverse) {
    const ae = taxCaseById("AE")!;
    return { id: "AE", sourceTokenIds: reverse.tokenIds, sourceText: reverse.text, suggestedReason: ae.defaultExemptionReason ?? reverse.text };
  }
  const intra = firstMatchingLine(extraction.lines, INTRA_COMMUNITY);
  if (intra) {
    const k = taxCaseById("K")!;
    return { id: "K", sourceTokenIds: intra.tokenIds, sourceText: intra.text, suggestedReason: k.defaultExemptionReason ?? intra.text };
  }
  const exempt = firstMatchingLine(extraction.lines, EXEMPT);
  if (exempt) {
    return { id: "E", sourceTokenIds: exempt.tokenIds, sourceText: exempt.text, suggestedReason: "" };
  }
  return undefined;
}
