import { calculateInvoice } from "../domain/calculate.js";
import type { CalculatedInvoice, InvoiceInput, UnitCode, ValidationIssue } from "../domain/types.js";
import { validateEn16931InvoiceInput, validateInvoiceInput } from "../domain/validate.js";
import type { ExtractedFieldName, ExtractionResult } from "../extraction/types.js";
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

export interface ReviewLineDraft {
  id: string;
  description: string;
  quantity: string;
  unitCode: UnitCode;
  netUnitPrice: string;
  taxRate: string;
  sourceTokenIds: string[];
}

export interface ReviewDraft {
  invoiceNumber: string;
  issueDate: string;
  dueDate: string;
  serviceDate: string;
  currency: string;
  buyerReference: string;
  seller: ReviewSellerDraft;
  buyer: ReviewPartyDraft;
  payment: { iban: string; bic: string; terms: string };
  lines: ReviewLineDraft[];
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

export function emptyReviewLine(id: string): ReviewLineDraft {
  return { id, description: "", quantity: "1", unitCode: "C62", netUnitPrice: "0", taxRate: "19", sourceTokenIds: [] };
}

export function reviewDraftFromExtraction(result: ExtractionResult): ReviewDraft {
  return {
    invoiceNumber: value(result, "invoiceNumber"),
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
      taxRate: line.taxRate ?? "19",
      sourceTokenIds: line.sourceTokenIds,
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

export function invoiceInputFromReview(draft: ReviewDraft): InvoiceInput {
  const sellerVat = draft.seller.vatId.replaceAll(" ", "").toUpperCase();
  const buyerVat = draft.buyer.vatId.replaceAll(" ", "").toUpperCase();
  const sellerContact = {
    name: draft.seller.contactName?.trim() ?? "",
    phone: draft.seller.phone?.trim() ?? "",
    email: draft.seller.email?.trim() ?? "",
  };
  const sellerEndpoint = germanElectronicAddress(sellerVat);
  const buyerEndpoint = germanElectronicAddress(buyerVat, draft.buyerReference);
  return {
    invoiceNumber: draft.invoiceNumber.trim(),
    invoiceType: "380",
    issueDate: draft.issueDate.trim(),
    ...(draft.dueDate.trim() ? { dueDate: draft.dueDate.trim() } : {}),
    ...(draft.serviceDate.trim() ? { serviceDate: draft.serviceDate.trim() } : {}),
    currency: draft.currency.trim().toUpperCase(),
    buyerReference: draft.buyerReference.trim(),
    seller: {
      name: draft.seller.name.trim(),
      address: {
        line1: draft.seller.addressLine1.trim(),
        city: draft.seller.city.trim(),
        postalCode: draft.seller.postalCode.trim(),
        countryCode: draft.seller.countryCode.trim().toUpperCase(),
      },
      ...(sellerVat ? { vatId: sellerVat } : {}),
      ...(sellerEndpoint ? { electronicAddress: sellerEndpoint } : {}),
      ...(sellerContact.name || sellerContact.phone || sellerContact.email ? { contact: sellerContact } : {}),
    },
    buyer: {
      name: draft.buyer.name.trim(),
      address: {
        line1: draft.buyer.addressLine1.trim(),
        city: draft.buyer.city.trim(),
        postalCode: draft.buyer.postalCode.trim(),
        countryCode: draft.buyer.countryCode.trim().toUpperCase(),
      },
      ...(buyerVat ? { vatId: buyerVat } : {}),
      ...(buyerEndpoint ? { electronicAddress: buyerEndpoint } : {}),
    },
    lines: draft.lines.map((line) => ({
      id: line.id,
      name: line.description.trim(),
      quantity: line.quantity,
      unitCode: line.unitCode,
      netUnitPrice: line.netUnitPrice,
      tax: { categoryCode: /^0+(?:\.0+)?$/.test(line.taxRate) ? "Z" : "S", rate: line.taxRate },
    })),
    payment: {
      meansCode: "58",
      ...(draft.payment.iban.trim() ? { iban: draft.payment.iban.replaceAll(" ", "").toUpperCase() } : {}),
      ...(draft.payment.bic.trim() ? { bic: draft.payment.bic.replaceAll(" ", "").toUpperCase() } : {}),
      ...(draft.seller.name.trim() ? { accountName: draft.seller.name.trim() } : {}),
      ...(draft.invoiceNumber.trim() ? { paymentReference: draft.invoiceNumber.trim() } : {}),
      ...(draft.payment.terms.trim() ? { terms: draft.payment.terms.trim() } : {}),
    },
  };
}

export function validateReviewDraft(
  draft: ReviewDraft,
  unsupportedCases: UnsupportedCase[] = [],
  format: ReviewExportFormat = "xrechnung",
): ReviewValidation {
  const input = invoiceInputFromReview(draft);
  const validation = format === "zugferd" ? validateEn16931InvoiceInput(input) : validateInvoiceInput(input);
  const unsupportedIssues: ValidationIssue[] = unsupportedCases.map((unsupportedCase) => ({
    severity: "error",
    code: `UNSUPPORTED_${unsupportedCase.code}`,
    path: "document",
    message: unsupportedCase.message,
  }));
  const issues = [...unsupportedIssues, ...validation.issues];
  if (issues.some((issue) => issue.severity === "error")) return { valid: false, issues };
  try {
    return { valid: true, issues, invoice: calculateInvoice(input) };
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
