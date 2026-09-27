import { calculateInvoice } from "../domain/calculate.js";
import type { Address, AllowanceCharge, CalculatedInvoice, InvoiceInput, InvoiceTypeCode, PrecedingInvoiceReference, UnitCode, ValidationIssue } from "../domain/types.js";
import { isCreditOrCorrection, isInvoiceTypeCode } from "../domain/types.js";
import { validateCalculatedInvoice, validateEn16931CalculatedInvoice, validateEn16931InvoiceInput, validateInvoiceInput } from "../domain/validate.js";
import { parseInvoiceDate } from "../extraction/dates.js";
import type { ExtractedFieldName, ExtractionResult } from "../extraction/types.js";
import { detectDocumentAllowances, detectDocumentKind } from "../policy/document-kind.js";
import {
  applyReviewTaxCase,
  detectTaxCase,
  isZeroRate,
  taxCaseById,
  taxCaseFromLegacy,
  taxTreatmentFromCase,
  type ReviewTaxCaseId,
} from "../policy/tax-cases.js";
import type { UnsupportedCase } from "../policy/unsupported-cases.js";

export interface ReviewPartyDraft {
  name: string;
  addressLine1: string;
  postalCode: string;
  city: string;
  countryCode: string;
  vatId: string;
}

export interface ReviewSellerDraft extends ReviewPartyDraft {
  contactName: string;
  phone: string;
  email: string;
}

export interface ReviewAllowanceDraft {
  id: string;
  charge: boolean;
  reason: string;
  amount: string;
  taxRate: string;
  taxCase: ReviewTaxCaseId | "";
  exemptionReason: string;
  percent?: string;
}

export interface ReviewLineDraft {
  id: string;
  description: string;
  quantity: string;
  unitCode: UnitCode;
  netUnitPrice: string;
  taxRate: string;
  taxCase: ReviewTaxCaseId | "";
  exemptionReason: string;
  sourceTokenIds: string[];
  allowances: ReviewAllowanceDraft[];
}

export interface ReviewPrecedingInvoiceDraft {
  invoiceNumber: string;
  issueDate: string;
  paidAmount: string;
}

export interface ReviewDraft {
  invoiceNumber: string;
  invoiceType: InvoiceTypeCode;
  finalInvoice: boolean;
  prepaymentInvoice: boolean;
  precedingInvoiceNumber: string;
  precedingInvoiceDate: string;
  precedingInvoices: ReviewPrecedingInvoiceDraft[];
  prepaidAmount: string;
  issueDate: string;
  dueDate: string;
  serviceDate: string;
  deliveryAddress?: Address;
  currency: string;
  buyerReference: string;
  seller: ReviewSellerDraft;
  buyer: ReviewPartyDraft;
  payment: { iban: string; bic: string; terms: string };
  lines: ReviewLineDraft[];
  allowances: ReviewAllowanceDraft[];
}

export interface ReviewValidation {
  valid: boolean;
  issues: ValidationIssue[];
  invoice?: CalculatedInvoice;
}

export type ReviewExportFormat = "xrechnung" | "zugferd";

function value(result: ExtractionResult, name: ExtractedFieldName, fallback = ""): string {
  return result.fields[name]?.value ?? fallback;
}

export function unitCodeFromText(unit: string | undefined): UnitCode {
  const normalized = unit?.replace(/\.$/, "").toLocaleLowerCase("de-DE") ?? "";
  if (/^(?:h|std|stunde|stunden|hour|hours)$/.test(normalized)) return "HUR";
  if (/^(?:tag|tage|day|days)$/.test(normalized)) return "DAY";
  if (/^(?:kg|kilogramm)$/.test(normalized)) return "KGM";
  if (/^(?:l|ltr|liter)$/.test(normalized)) return "LTR";
  if (/^(?:m|mtr|meter)$/.test(normalized)) return "MTR";
  return "C62";
}

export function emptyPrecedingInvoice(): ReviewPrecedingInvoiceDraft {
  return { invoiceNumber: "", issueDate: "", paidAmount: "" };
}

export function reviewKindValue(draft: ReviewDraft): InvoiceTypeCode | "final" | "prepayment" {
  if (draft.finalInvoice) return "final";
  if (draft.prepaymentInvoice) return "prepayment";
  return draft.invoiceType;
}

export function applyReviewDocumentKind(draft: ReviewDraft, value: string): ReviewDraft {
  const invoiceType: InvoiceTypeCode = value === "final" || value === "prepayment" || value === "386"
    ? "380"
    : isInvoiceTypeCode(value) ? value : "380";
  const next: ReviewDraft = {
    ...draft,
    invoiceType,
    finalInvoice: value === "final",
    prepaymentInvoice: value === "prepayment" || value === "386",
  };
  if (!isCreditOrCorrection(invoiceType) && next.precedingInvoices.length === 0 && next.precedingInvoiceNumber.trim()) {
    next.precedingInvoices = [{ invoiceNumber: next.precedingInvoiceNumber, issueDate: next.precedingInvoiceDate, paidAmount: next.prepaidAmount }];
  }
  if (isCreditOrCorrection(invoiceType) && !next.precedingInvoiceNumber.trim() && next.precedingInvoices[0]?.invoiceNumber) {
    next.precedingInvoiceNumber = next.precedingInvoices[0].invoiceNumber;
    next.precedingInvoiceDate = next.precedingInvoices[0].issueDate;
  }
  return next;
}

export function emptyReviewAllowance(id: string, charge = false, taxCase: ReviewTaxCaseId | "" = "S19"): ReviewAllowanceDraft {
  const tax = applyReviewTaxCase(taxCase);
  return { id, charge, reason: charge ? "Zuschlag" : "Rabatt", amount: "0", taxRate: tax.taxRate, taxCase: tax.taxCase, exemptionReason: tax.exemptionReason };
}

export function emptyReviewLine(id: string): ReviewLineDraft {
  const tax = applyReviewTaxCase("S19");
  return { id, description: "", quantity: "1", unitCode: "C62", netUnitPrice: "0", taxRate: tax.taxRate, taxCase: tax.taxCase, exemptionReason: tax.exemptionReason, sourceTokenIds: [], allowances: [] };
}

function asTaxFields(row: { taxRate?: unknown; taxCase?: unknown; exemptionReason?: unknown }): { taxRate: string; taxCase: ReviewTaxCaseId | ""; exemptionReason: string } {
  const taxRate = typeof row.taxRate === "string" && row.taxRate ? row.taxRate : "19";
  const taxCase = taxCaseFromLegacy(taxRate, typeof row.taxCase === "string" ? row.taxCase : undefined);
  const selected = taxCaseById(taxCase);
  const exemptionReason = typeof row.exemptionReason === "string"
    ? row.exemptionReason
    : selected?.defaultExemptionReason ?? "";
  return {
    taxCase,
    taxRate: selected?.rate ?? taxRate,
    exemptionReason: selected && !selected.exemptionReasonRequired ? "" : exemptionReason,
  };
}

function asInvoiceType(value: unknown): InvoiceTypeCode {
  if (value === "386") return "380";
  return typeof value === "string" && isInvoiceTypeCode(value) ? value : "380";
}

function asPrecedingInvoices(value: unknown): ReviewPrecedingInvoiceDraft[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (typeof item !== "object" || item === null) return [];
    const row = item as Partial<ReviewPrecedingInvoiceDraft>;
    return [{
      invoiceNumber: typeof row.invoiceNumber === "string" ? row.invoiceNumber : "",
      issueDate: typeof row.issueDate === "string" ? row.issueDate : "",
      paidAmount: typeof row.paidAmount === "string" ? row.paidAmount : "",
    }];
  }).slice(0, 50);
}

function asAllowances(value: unknown): ReviewAllowanceDraft[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item, index) => {
    if (typeof item !== "object" || item === null) return [];
    const row = item as Partial<ReviewAllowanceDraft>;
    if (typeof row.amount !== "string") return [];
    return [{
      id: typeof row.id === "string" && row.id ? row.id : `allowance-${index + 1}`,
      charge: row.charge === true,
      reason: typeof row.reason === "string" ? row.reason : row.charge === true ? "Zuschlag" : "Rabatt",
      amount: row.amount,
      ...asTaxFields(row),
      ...(typeof row.percent === "string" && row.percent ? { percent: row.percent } : {}),
    }];
  });
}

export function normalizeReviewDraft(draft: ReviewDraft): ReviewDraft {
  const rawType = draft.invoiceType as string;
  const invoiceType = asInvoiceType(rawType);
  return {
    ...draft,
    invoiceType,
    finalInvoice: draft.finalInvoice === true && invoiceType === "380",
    prepaymentInvoice: (draft.prepaymentInvoice === true || rawType === "386") && invoiceType === "380" && draft.finalInvoice !== true,
    precedingInvoiceNumber: draft.precedingInvoiceNumber ?? "",
    precedingInvoiceDate: draft.precedingInvoiceDate ?? "",
    precedingInvoices: asPrecedingInvoices(draft.precedingInvoices),
    prepaidAmount: typeof draft.prepaidAmount === "string" ? draft.prepaidAmount : "",
    allowances: asAllowances(draft.allowances),
    lines: draft.lines.map((line) => ({
      ...line,
      ...asTaxFields(line),
      allowances: asAllowances(line.allowances),
    })),
  };
}

function taxFieldsFromExtraction(taxRate: string, detected?: ReturnType<typeof detectTaxCase>): { taxRate: string; taxCase: ReviewTaxCaseId | ""; exemptionReason: string } {
  if (isZeroRate(taxRate) && detected) {
    const selected = taxCaseById(detected.id)!;
    return { taxCase: detected.id, taxRate: selected.rate, exemptionReason: detected.suggestedReason };
  }
  if (isZeroRate(taxRate)) return { taxCase: "", taxRate: "0", exemptionReason: "" };
  return asTaxFields({ taxRate });
}

export function reviewDraftFromExtraction(result: ExtractionResult): ReviewDraft {
  const kind = detectDocumentKind(result);
  const detectedAllowances = detectDocumentAllowances(result);
  const detectedTax = detectTaxCase(result);
  return {
    invoiceNumber: value(result, "invoiceNumber"),
    invoiceType: kind?.invoiceType ?? "380",
    finalInvoice: kind?.finalInvoice ?? false,
    prepaymentInvoice: kind?.prepaymentInvoice ?? false,
    precedingInvoiceNumber: kind?.precedingInvoiceNumber ?? "",
    precedingInvoiceDate: parseInvoiceDate(kind?.precedingInvoiceDate ?? "") ?? "",
    precedingInvoices: (kind?.precedingInvoices ?? []).map((item) => ({
      invoiceNumber: item.invoiceNumber,
      issueDate: parseInvoiceDate(item.issueDate) ?? item.issueDate,
      paidAmount: item.paidAmount,
    })),
    prepaidAmount: kind?.prepaidAmount ?? "",
    issueDate: value(result, "issueDate"),
    dueDate: value(result, "dueDate"),
    serviceDate: value(result, "serviceDate"),
    currency: value(result, "currency", "EUR"),
    buyerReference: value(result, "buyerReference"),
    seller: {
      name: value(result, "sellerName"),
      addressLine1: value(result, "sellerAddressLine1"),
      postalCode: value(result, "sellerPostalCode"),
      city: value(result, "sellerCity"),
      countryCode: value(result, "sellerCountryCode", "DE"),
      vatId: value(result, "sellerVatId"),
      contactName: value(result, "sellerContact"),
      phone: value(result, "sellerPhone"),
      email: value(result, "sellerEmail"),
    },
    buyer: {
      name: value(result, "buyerName"),
      addressLine1: value(result, "buyerAddressLine1"),
      postalCode: value(result, "buyerPostalCode"),
      city: value(result, "buyerCity"),
      countryCode: value(result, "buyerCountryCode", "DE"),
      vatId: value(result, "buyerVatId"),
    },
    payment: {
      iban: value(result, "iban"),
      bic: value(result, "bic"),
      terms: value(result, "paymentTerms"),
    },
    lines: result.lineItems.map((line, index) => ({
      id: String(index + 1),
      description: line.description,
      quantity: line.quantity,
      unitCode: unitCodeFromText(line.unit),
      netUnitPrice: line.netUnitPrice,
      sourceTokenIds: line.sourceTokenIds,
      allowances: [],
      ...taxFieldsFromExtraction(line.taxRate ?? "19", detectedTax),
    })),
    allowances: detectedAllowances.map((item, index) => ({
      id: `detected-${index + 1}`,
      charge: item.reason === "Zuschlag",
      reason: item.reason,
      amount: item.amount,
      ...asTaxFields({ taxRate: item.taxRate }),
      ...(item.percent ? { percent: item.percent } : {}),
    })),
  };
}

function germanElectronicAddress(vatId: string, leitwegId = ""): { value: string; schemeId: string } | undefined {
  const vat = vatId.replaceAll(" ", "").toUpperCase();
  if (/^[A-Z]{2}[A-Z0-9]{8,12}$/.test(vat)) return { value: vat, schemeId: "9930" };
  const leitweg = leitwegId.trim();
  if (/^\d{2,12}-\d+-\d{2}$/.test(leitweg)) return { value: leitweg, schemeId: "0204" };
  return undefined;
}

function allowanceFromReview(item: ReviewAllowanceDraft): AllowanceCharge | undefined {
  if (!item.amount.trim() || isZeroRate(item.amount)) return undefined;
  return {
    charge: item.charge,
    amount: item.amount,
    ...(item.reason.trim() ? { reason: item.reason.trim() } : {}),
    ...(item.percent?.trim() ? { percent: item.percent.trim() } : {}),
    tax: taxTreatmentFromCase(item.taxCase, item.taxRate, item.exemptionReason),
  };
}

function reviewTaxTreatments(draft: ReviewDraft): Array<{ taxCase: ReviewTaxCaseId | ""; taxRate: string; path: string }> {
  return [
    ...draft.lines.map((line, index) => ({ taxCase: line.taxCase, taxRate: line.taxRate, path: `lines.${index}.tax.rate` })),
    ...draft.allowances.flatMap((item, index) => item.amount.trim() && !isZeroRate(item.amount)
      ? [{ taxCase: item.taxCase, taxRate: item.taxRate, path: `allowances.${index}.tax.rate` }]
      : []),
  ];
}

export function draftRequiresVatIds(draft: ReviewDraft): boolean {
  return reviewTaxTreatments(draft).some((item) => {
    const selected = taxCaseById(item.taxCase);
    return Boolean(selected?.requireSellerVatId || selected?.requireBuyerVatId);
  });
}

function reviewTaxChoiceIssues(draft: ReviewDraft): ValidationIssue[] {
  return reviewTaxTreatments(draft).flatMap((item) => {
    if (taxCaseById(item.taxCase)) return [];
    return [{
      severity: "error" as const,
      code: "TAX_CASE",
      path: item.path,
      message: isZeroRate(item.taxRate) || !item.taxRate.trim()
        ? "0 % allein entscheidet nicht zwischen Reverse Charge, Steuerfreiheit und innergemeinschaftlicher Lieferung. Bitte wählen Sie den Steuerfall."
        : "Bitte wählen Sie den Steuerfall.",
    }];
  });
}

function precedingFromReview(draft: ReviewDraft): PrecedingInvoiceReference[] {
  if (isCreditOrCorrection(draft.invoiceType)) {
    const invoiceNumber = draft.precedingInvoiceNumber.trim();
    if (!invoiceNumber) return [];
    const issueDate = parseInvoiceDate(draft.precedingInvoiceDate.trim()) ?? draft.precedingInvoiceDate.trim();
    return [{ invoiceNumber, ...(issueDate ? { issueDate } : {}) }];
  }
  return draft.precedingInvoices.flatMap((item) => {
    const invoiceNumber = item.invoiceNumber.trim();
    if (!invoiceNumber) return [];
    const issueDate = parseInvoiceDate(item.issueDate.trim()) ?? item.issueDate.trim();
    const paidAmount = item.paidAmount.trim();
    return [{
      invoiceNumber,
      ...(issueDate ? { issueDate } : {}),
      ...(paidAmount && !isZeroRate(paidAmount) ? { paidAmount } : {}),
    }];
  });
}

export function invoiceInputFromReview(draft: ReviewDraft): InvoiceInput {
  const normalized = normalizeReviewDraft(draft);
  const sellerVat = normalized.seller.vatId.replaceAll(" ", "").toUpperCase();
  const buyerVat = normalized.buyer.vatId.replaceAll(" ", "").toUpperCase();
  const sellerContact = {
    name: normalized.seller.contactName?.trim() ?? "",
    phone: normalized.seller.phone?.trim() ?? "",
    email: normalized.seller.email?.trim() ?? "",
  };
  const sellerEndpoint = germanElectronicAddress(sellerVat);
  const buyerEndpoint = germanElectronicAddress(buyerVat, normalized.buyerReference);
  const documentAllowances = normalized.allowances.map(allowanceFromReview).filter((item): item is AllowanceCharge => Boolean(item));
  const refs = precedingFromReview(normalized);
  const prepaid = normalized.prepaidAmount.trim();
  const reverseChargeNote = taxCaseById("AE")?.defaultExemptionReason;
  const hasReverseCharge = reviewTaxTreatments(normalized).some((item) => item.taxCase === "AE");
  return {
    invoiceNumber: normalized.invoiceNumber.trim(),
    invoiceType: normalized.invoiceType,
    issueDate: normalized.issueDate.trim(),
    ...(normalized.dueDate.trim() ? { dueDate: normalized.dueDate.trim() } : {}),
    ...(normalized.serviceDate.trim() ? { serviceDate: normalized.serviceDate.trim() } : {}),
    ...(normalized.deliveryAddress ? { deliveryAddress: {
      line1: normalized.deliveryAddress.line1.trim(),
      city: normalized.deliveryAddress.city.trim(),
      postalCode: normalized.deliveryAddress.postalCode.trim(),
      countryCode: normalized.deliveryAddress.countryCode.trim().toUpperCase(),
    } } : {}),
    currency: normalized.currency.trim().toUpperCase(),
    buyerReference: normalized.buyerReference.trim(),
    seller: {
      name: normalized.seller.name.trim(),
      address: {
        line1: normalized.seller.addressLine1.trim(),
        city: normalized.seller.city.trim(),
        postalCode: normalized.seller.postalCode.trim(),
        countryCode: normalized.seller.countryCode.trim().toUpperCase(),
      },
      ...(sellerVat ? { vatId: sellerVat } : {}),
      ...(sellerEndpoint ? { electronicAddress: sellerEndpoint } : {}),
      ...(sellerContact.name || sellerContact.phone || sellerContact.email ? { contact: sellerContact } : {}),
    },
    buyer: {
      name: normalized.buyer.name.trim(),
      address: {
        line1: normalized.buyer.addressLine1.trim(),
        city: normalized.buyer.city.trim(),
        postalCode: normalized.buyer.postalCode.trim(),
        countryCode: normalized.buyer.countryCode.trim().toUpperCase(),
      },
      ...(buyerVat ? { vatId: buyerVat } : {}),
      ...(buyerEndpoint ? { electronicAddress: buyerEndpoint } : {}),
    },
    lines: normalized.lines.map((line) => {
      const lineAllowances = line.allowances.map(allowanceFromReview).filter((item): item is AllowanceCharge => Boolean(item));
      return {
        id: line.id,
        name: line.description.trim(),
        quantity: line.quantity,
        unitCode: line.unitCode,
        netUnitPrice: line.netUnitPrice,
        tax: taxTreatmentFromCase(line.taxCase, line.taxRate, line.exemptionReason),
        ...(lineAllowances.length ? { allowances: lineAllowances } : {}),
      };
    }),
    payment: {
      meansCode: "58",
      ...(normalized.payment.iban.trim() ? { iban: normalized.payment.iban.replaceAll(" ", "").toUpperCase() } : {}),
      ...(normalized.payment.bic.trim() ? { bic: normalized.payment.bic.replaceAll(" ", "").toUpperCase() } : {}),
      ...(normalized.seller.name.trim() ? { accountName: normalized.seller.name.trim() } : {}),
      ...(normalized.invoiceNumber.trim() ? { paymentReference: normalized.invoiceNumber.trim() } : {}),
      ...(normalized.payment.terms.trim() ? { terms: normalized.payment.terms.trim() } : {}),
    },
    ...(normalized.finalInvoice ? { finalInvoice: true } : {}),
    ...(normalized.prepaymentInvoice ? { prepaymentInvoice: true } : {}),
    ...(prepaid && !isZeroRate(prepaid) ? { prepaidAmount: prepaid } : {}),
    ...(refs.length ? { precedingInvoices: refs, precedingInvoice: refs[0] } : {}),
    ...(documentAllowances.length ? { allowances: documentAllowances } : {}),
    ...((hasReverseCharge && reverseChargeNote) || normalized.prepaymentInvoice
      ? { notes: [
        ...(hasReverseCharge && reverseChargeNote ? [reverseChargeNote] : []),
        ...(normalized.prepaymentInvoice ? ["Anzahlungsrechnung"] : []),
      ] }
      : {}),
  };
}

export function validateReviewDraft(
  draft: ReviewDraft,
  unsupportedCases: UnsupportedCase[] = [],
  format: ReviewExportFormat = "xrechnung",
): ReviewValidation {
  const normalized = normalizeReviewDraft(draft);
  const input = invoiceInputFromReview(normalized);
  const validation = format === "zugferd" ? validateEn16931InvoiceInput(input) : validateInvoiceInput(input);
  const taxChoice = reviewTaxChoiceIssues(normalized);
  const unsupportedIssues: ValidationIssue[] = unsupportedCases.map((unsupportedCase) => ({
    severity: "error",
    code: `UNSUPPORTED_${unsupportedCase.code}`,
    path: "document",
    message: unsupportedCase.message,
  }));
  const issues = [
    ...unsupportedIssues,
    ...taxChoice,
    ...validation.issues.filter((issue) => !(issue.code === "BR-S-5" && taxChoice.some((item) => item.path === issue.path))),
  ];
  if (issues.some((issue) => issue.severity === "error")) return { valid: false, issues };
  try {
    const invoice = calculateInvoice(input);
    const calculated = format === "zugferd" ? validateEn16931CalculatedInvoice(invoice) : validateCalculatedInvoice(invoice);
    return { valid: calculated.valid, issues: [...issues, ...calculated.issues], ...(calculated.valid ? { invoice } : {}) };
  } catch (reason) {
    return {
      valid: false,
      issues: [{ severity: "error", code: "CALCULATION", path: "lines", message: reason instanceof Error ? reason.message : "Rechnung konnte nicht berechnet werden." }],
    };
  }
}

export function calculateReviewDraft(draft: ReviewDraft): CalculatedInvoice | undefined {
  try {
    return calculateInvoice(invoiceInputFromReview(draft));
  } catch {
    return undefined;
  }
}
