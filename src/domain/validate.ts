import { decimal } from "./money.js";
import type { CalculatedInvoice, InvoiceInput, ValidationIssue, ValidationResult } from "./types.js";

const ISO_CURRENCY = /^[A-Z]{3}$/;
const COUNTRY = /^[A-Z]{2}$/;

export function isIsoDate(value: string): boolean {
  const match = value.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return false;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (month < 1 || month > 12 || day < 1) return false;
  const daysInMonth = [31, year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return day <= (daysInMonth[month - 1] ?? 0);
}

export function validateEn16931InvoiceInput(invoice: InvoiceInput): ValidationResult {
  const issues: ValidationIssue[] = [];
  const required = (value: string | undefined, path: string, code: string) => {
    if (!value?.trim()) issues.push({ severity: "error", code, path, message: "Bitte tragen Sie diese Angabe ein." });
  };
  required(invoice.invoiceNumber, "invoiceNumber", "BR-02");
  required(invoice.seller.name, "seller.name", "BR-06");
  required(invoice.buyer.name, "buyer.name", "BR-07");
  if (!isIsoDate(invoice.issueDate)) issues.push({ severity: "error", code: "DATE", path: "issueDate", message: "Bitte wählen Sie ein gültiges Datum." });
  if (invoice.dueDate && !isIsoDate(invoice.dueDate)) issues.push({ severity: "error", code: "DATE", path: "dueDate", message: "Bitte wählen Sie ein gültiges Datum." });
  if (!ISO_CURRENCY.test(invoice.currency)) issues.push({ severity: "error", code: "BR-05", path: "currency", message: "Bitte tragen Sie eine gültige Währung ein, zum Beispiel EUR." });
  for (const [partyName, party] of [["seller", invoice.seller], ["buyer", invoice.buyer]] as const) {
    required(party.address.line1, `${partyName}.address.line1`, "BR-10");
    required(party.address.city, `${partyName}.address.city`, "BR-10");
    required(party.address.postalCode, `${partyName}.address.postalCode`, "BR-10");
    if (!COUNTRY.test(party.address.countryCode)) issues.push({ severity: "error", code: "COUNTRY", path: `${partyName}.address.countryCode`, message: "Bitte tragen Sie das Land mit zwei Buchstaben ein, zum Beispiel DE." });
  }
  if (invoice.lines.length === 0) issues.push({ severity: "error", code: "BR-16", path: "lines", message: "Mindestens eine Rechnungsposition ist erforderlich." });
  invoice.lines.forEach((line, index) => {
    required(line.id, `lines.${index}.id`, "BR-21");
    required(line.name, `lines.${index}.name`, "BR-25");
    try {
      if (decimal(line.quantity).lte(0)) throw new Error();
    } catch { issues.push({ severity: "error", code: "QUANTITY", path: `lines.${index}.quantity`, message: "Bitte tragen Sie eine Menge größer als 0 ein." }); }
    try {
      if (decimal(line.netUnitPrice).lt(0)) throw new Error();
    } catch { issues.push({ severity: "error", code: "PRICE", path: `lines.${index}.netUnitPrice`, message: "Bitte tragen Sie einen Preis ab 0 ein." }); }
    try { decimal(line.tax.rate); }
    catch { issues.push({ severity: "error", code: "RATE", path: `lines.${index}.tax.rate`, message: "Bitte tragen Sie einen gültigen Steuersatz ein." }); }
    if (["E", "AE"].includes(line.tax.categoryCode) && !line.tax.exemptionReason) issues.push({ severity: "error", code: "BR-E-10", path: `lines.${index}.tax.exemptionReason`, message: "Bitte tragen Sie ein, warum keine Umsatzsteuer berechnet wird." });
  });
  return { valid: !issues.some((issue) => issue.severity === "error"), issues };
}

export function validateInvoiceInput(invoice: InvoiceInput): ValidationResult {
  const base = validateEn16931InvoiceInput(invoice);
  const issues = [...base.issues];
  if (!invoice.buyerReference.trim()) {
    issues.push({ severity: "error", code: "BR-DE-15", path: "buyerReference", message: "Bitte tragen Sie die Bestellnummer oder Leitweg-ID der Behörde ein." });
  }
  return { valid: !issues.some((issue) => issue.severity === "error"), issues };
}

export function validateCalculatedInvoice(invoice: CalculatedInvoice): ValidationResult {
  const base = validateInvoiceInput(invoice);
  return validateCalculatedTotals(invoice, base.issues);
}

export function validateEn16931CalculatedInvoice(invoice: CalculatedInvoice): ValidationResult {
  const base = validateEn16931InvoiceInput(invoice);
  return validateCalculatedTotals(invoice, base.issues);
}

function validateCalculatedTotals(invoice: CalculatedInvoice, baseIssues: ValidationIssue[]): ValidationResult {
  const issues = [...baseIssues];
  const expectedTax = invoice.taxes.reduce((sum, tax) => sum.add(tax.taxAmount), decimal(0));
  if (!expectedTax.eq(invoice.totals.taxTotal)) issues.push({ severity: "error", code: "BR-CO-14", path: "totals.taxTotal", message: "Die Umsatzsteuer passt nicht zu den Rechnungspositionen." });
  if (!decimal(invoice.totals.taxExclusive).add(invoice.totals.taxTotal).eq(invoice.totals.taxInclusive)) issues.push({ severity: "error", code: "BR-CO-15", path: "totals.taxInclusive", message: "Der Rechnungsbetrag passt nicht zu Nettobetrag und Umsatzsteuer." });
  if (!decimal(invoice.totals.taxInclusive).eq(invoice.totals.payable)) issues.push({ severity: "error", code: "BR-CO-16", path: "totals.payable", message: "Der zu zahlende Betrag stimmt nicht mit dem Rechnungsbetrag überein." });
  return { valid: !issues.some((issue) => issue.severity === "error"), issues };
}

export function assertValidInvoice(invoice: CalculatedInvoice): void {
  const result = validateCalculatedInvoice(invoice);
  if (!result.valid) throw new Error(result.issues.map((issue) => `${issue.code} ${issue.path}: ${issue.message}`).join("\n"));
}

export function assertValidEn16931Invoice(invoice: CalculatedInvoice): void {
  const result = validateEn16931CalculatedInvoice(invoice);
  if (!result.valid) throw new Error(result.issues.map((issue) => `${issue.code} ${issue.path}: ${issue.message}`).join("\n"));
}
