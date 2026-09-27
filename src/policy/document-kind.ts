import { parseLocalizedDecimal } from "../domain/localized-decimal.js";
import { decimal, money } from "../domain/money.js";
import { isCreditOrCorrection, type InvoiceTypeCode } from "../domain/types.js";
import type { ExtractionResult, TextLine } from "../extraction/types.js";

export interface DetectedPrecedingInvoice {
  invoiceNumber: string;
  issueDate: string;
  paidAmount: string;
  sourceTokenIds: string[];
  sourceText: string;
}

export interface DetectedDocumentKind {
  invoiceType: InvoiceTypeCode;
  finalInvoice: boolean;
  prepaymentInvoice: boolean;
  precedingInvoiceNumber: string;
  precedingInvoiceDate: string;
  precedingInvoices: DetectedPrecedingInvoice[];
  prepaidAmount: string;
  sourceTokenIds: string[];
  sourceText: string;
}

export interface DetectedDocumentAllowance {
  reason: string;
  amount: string;
  percent: string;
  taxRate: string;
  sourceTokenIds: string[];
  sourceText: string;
}

const CREDIT_NOTE = /\b(?:gutschrift|stornorechnung|credit\s+note)\b/i;
const CORRECTION = /\b(?:rechnungskorrektur|korrekturrechnung|corrected\s+invoice)\b/i;
const FINAL_INVOICE = /\bschlussrechnung\b/i;
const PARTIAL_INVOICE = /\babschlagsrechnung\b/i;
const PREPAYMENT_INVOICE = /\b(?:anzahlungsrechnung|vorauszahlungsrechnung)\b/i;
const PRECEDING = /(?:ursprungsrechnung|zu\s+rechnung|zur\s+rechnung|originalrechnung|preceding\s+invoice)\s*:?\s*([A-Z0-9][A-Z0-9/_-]{2,})/i;
const PRECEDING_DATE = /(?:ursprungsrechnung|zu\s+rechnung).{0,40}?(\d{1,2}[./-]\d{1,2}[./-]\d{2,4}|\d{4}-\d{2}-\d{2})/i;
const SETTLEMENT_LINE = /\b(?:abschlag(?:srechnung)?|anzahlung(?:srechnung)?|vorauszahlung(?:srechnung)?|teilrechnung)\s*(?:\d+)?\s*:?\s*(?:nr\.?\s*)?([A-Z0-9][A-Z0-9/_-]{2,})(?:.{0,48}?(\d{1,2}[./-]\d{1,2}[./-]\d{2,4}|\d{4}-\d{2}-\d{2}))?(?:.{0,48}?(\d{1,3}(?:\.\d{3})*,\d{2}|\d+,\d{2}))?/i;
const PREPAID_TOTAL = /\b(?:bereits\s+(?:gezahlt|berechnet|in\s+rechnung\s+gestellt)|abzüglich\s+(?:anzahlung|abschlag|vorauszahlung)|bisherige\s+(?:anzahlungen|abschläge)|bereits\s+gezahlt)\s*:?\s*(\d{1,3}(?:\.\d{3})*,\d{2}|\d+,\d{2})/i;
const ALLOWANCE_LINE = /\b(rabatt|nachlass|rechnungsabschlag|zuschlag|versandkosten)\b(?:\s+(\d+(?:[.,]\d+)?)\s*%)?[:\s]+(-?\d{1,3}(?:\.\d{3})*,\d{2}|\d+,\d{2}|\d+\.\d{2})/i;

function firstMatchingLine(lines: TextLine[], pattern: RegExp): TextLine | undefined {
  return lines.find((line) => pattern.test(line.text));
}

function amountFrom(value: string | undefined): string {
  if (!value) return "";
  const parsed = parseLocalizedDecimal(value);
  return parsed === null ? "" : money(parsed.replace(/^-/, ""));
}

export function detectPrecedingSettlements(extraction: ExtractionResult, currentInvoiceNumber = ""): DetectedPrecedingInvoice[] {
  const current = currentInvoiceNumber.trim().toUpperCase();
  const found: DetectedPrecedingInvoice[] = [];
  const seen = new Set<string>();
  for (const line of extraction.lines) {
    if (FINAL_INVOICE.test(line.text) || PARTIAL_INVOICE.test(line.text) || PREPAYMENT_INVOICE.test(line.text)) continue;
    const match = line.text.match(SETTLEMENT_LINE);
    const invoiceNumber = match?.[1]?.trim() ?? "";
    if (!invoiceNumber || invoiceNumber.toUpperCase() === current) continue;
    const key = invoiceNumber.toUpperCase();
    if (seen.has(key)) continue;
    seen.add(key);
    found.push({
      invoiceNumber,
      issueDate: match?.[2] ?? "",
      paidAmount: amountFrom(match?.[3]),
      sourceTokenIds: line.tokenIds,
      sourceText: line.text,
    });
  }
  const precedingLine = firstMatchingLine(extraction.lines, PRECEDING);
  const precedingMatch = precedingLine?.text.match(PRECEDING);
  const precedingNumber = precedingMatch?.[1]?.trim() ?? "";
  if (precedingNumber && precedingNumber.toUpperCase() !== current && !seen.has(precedingNumber.toUpperCase())) {
    const dateLine = firstMatchingLine(extraction.lines, PRECEDING_DATE);
    found.unshift({
      invoiceNumber: precedingNumber,
      issueDate: dateLine?.text.match(PRECEDING_DATE)?.[1] ?? "",
      paidAmount: "",
      sourceTokenIds: precedingLine?.tokenIds ?? [],
      sourceText: precedingLine?.text ?? precedingNumber,
    });
  }
  return found;
}

export function detectDocumentKind(extraction: ExtractionResult): DetectedDocumentKind | undefined {
  const correction = firstMatchingLine(extraction.lines, CORRECTION);
  const credit = firstMatchingLine(extraction.lines, CREDIT_NOTE);
  const finalInvoice = firstMatchingLine(extraction.lines, FINAL_INVOICE);
  const partial = firstMatchingLine(extraction.lines, PARTIAL_INVOICE);
  const prepayment = firstMatchingLine(extraction.lines, PREPAYMENT_INVOICE);
  const typeLine = correction ?? credit ?? finalInvoice ?? partial ?? prepayment;
  const preceding = detectPrecedingSettlements(extraction, extraction.fields.invoiceNumber?.value ?? "");
  const prepaidLine = firstMatchingLine(extraction.lines, PREPAID_TOTAL);
  const prepaidFromRefs = preceding.reduce((sum, item) => item.paidAmount ? sum.add(item.paidAmount) : sum, decimal(0));
  const prepaidAmount = amountFrom(prepaidLine?.text.match(PREPAID_TOTAL)?.[1])
    || (prepaidFromRefs.gt(0) ? money(prepaidFromRefs) : "");
  if (!typeLine && !preceding.some((item) => item.paidAmount) && !prepaidAmount) return undefined;
  const invoiceType: InvoiceTypeCode = correction ? "384" : credit ? "381" : partial ? "326" : "380";
  const kindLine = typeLine ?? prepaidLine ?? extraction.lines.find((line) => preceding.some((item) => item.sourceTokenIds[0] && line.tokenIds.includes(item.sourceTokenIds[0]))) ?? extraction.lines[0];
  return {
    invoiceType,
    finalInvoice: Boolean(finalInvoice) && invoiceType === "380",
    prepaymentInvoice: Boolean(prepayment) && invoiceType === "380" && !finalInvoice,
    precedingInvoiceNumber: preceding[0]?.invoiceNumber ?? "",
    precedingInvoiceDate: preceding[0]?.issueDate ?? "",
    precedingInvoices: isCreditOrCorrection(invoiceType) ? [] : preceding,
    prepaidAmount: isCreditOrCorrection(invoiceType) ? "" : prepaidAmount,
    sourceTokenIds: kindLine?.tokenIds ?? [],
    sourceText: kindLine?.text ?? "",
  };
}

export function detectDocumentAllowances(extraction: ExtractionResult): DetectedDocumentAllowance[] {
  const taxHint = extraction.lineItems.find((line) => line.taxRate && !/^0+(?:\.0+)?$/.test(line.taxRate))?.taxRate
    ?? extraction.fields.taxTotal?.sourceText.match(/(\d+(?:[.,]\d+)?)\s*%/)?.[1];
  const taxRate = taxHint?.replace(",", ".") ?? "19";
  const found: DetectedDocumentAllowance[] = [];
  for (const line of extraction.lines) {
    const match = line.text.match(ALLOWANCE_LINE);
    if (!match?.[1] || !match[3]) continue;
    if (extraction.lineItems.some((item) => item.sourceTokenIds.some((id) => line.tokenIds.includes(id)))) continue;
    const amount = parseLocalizedDecimal(match[3]);
    if (amount === null) continue;
    const reason = /zuschlag|versandkosten/i.test(match[1]) ? "Zuschlag" : "Rabatt";
    found.push({
      reason,
      amount: money(amount.replace(/^-/, "")),
      percent: match[2] ? match[2].replace(",", ".") : "",
      taxRate,
      sourceTokenIds: line.tokenIds,
      sourceText: line.text,
    });
  }
  return found;
}
