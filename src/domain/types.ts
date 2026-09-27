export const INVOICE_TYPE_CODES = ["380", "381", "384", "326"] as const;
export type InvoiceTypeCode = (typeof INVOICE_TYPE_CODES)[number];
export type TaxCategoryCode = "S" | "Z" | "E" | "AE" | "K";
export type UnitCode = "C62" | "HUR" | "DAY" | "KGM" | "LTR" | "MTR";

export function isInvoiceTypeCode(value: string): value is InvoiceTypeCode {
  return (INVOICE_TYPE_CODES as readonly string[]).includes(value);
}

export function isCreditOrCorrection(type: InvoiceTypeCode): boolean {
  return type === "381" || type === "384";
}

export function isPartialInvoice(type: InvoiceTypeCode): boolean {
  return type === "326";
}

export interface Address {
  line1: string;
  line2?: string;
  city: string;
  postalCode: string;
  countryCode: string;
}

export interface Party {
  name: string;
  address: Address;
  vatId?: string;
  taxRegistrationId?: string;
  electronicAddress?: { value: string; schemeId: string };
  contact?: { name?: string; email?: string; phone?: string };
}

export interface AllowanceCharge {
  charge: boolean;
  amount: string;
  reason?: string;
  reasonCode?: string;
  percent?: string;
  baseAmount?: string;
  tax: { categoryCode: TaxCategoryCode; rate: string; exemptionReason?: string; exemptionReasonCode?: string };
}

export interface PrecedingInvoiceReference {
  invoiceNumber: string;
  issueDate?: string;
  paidAmount?: string;
}

export interface InvoiceLine {
  id: string;
  name: string;
  description?: string;
  quantity: string;
  unitCode: UnitCode;
  netUnitPrice: string;
  tax: { categoryCode: TaxCategoryCode; rate: string; exemptionReason?: string; exemptionReasonCode?: string };
  allowances?: AllowanceCharge[];
}

export interface PaymentInfo {
  meansCode: string;
  iban?: string;
  bic?: string;
  accountName?: string;
  paymentReference?: string;
  terms?: string;
}

export interface InvoiceInput {
  invoiceNumber: string;
  invoiceType: InvoiceTypeCode;
  issueDate: string;
  dueDate?: string;
  serviceDate?: string;
  deliveryAddress?: Address;
  currency: string;
  buyerReference: string;
  seller: Party;
  buyer: Party;
  lines: InvoiceLine[];
  payment: PaymentInfo;
  notes?: string[];
  precedingInvoice?: PrecedingInvoiceReference;
  precedingInvoices?: PrecedingInvoiceReference[];
  prepaidAmount?: string;
  finalInvoice?: boolean;
  prepaymentInvoice?: boolean;
  allowances?: AllowanceCharge[];
}

export function resolvedPrecedingInvoices(invoice: {
  precedingInvoice?: PrecedingInvoiceReference;
  precedingInvoices?: PrecedingInvoiceReference[];
}): PrecedingInvoiceReference[] {
  if (invoice.precedingInvoices?.length) {
    return invoice.precedingInvoices.filter((item) => item.invoiceNumber.trim());
  }
  if (invoice.precedingInvoice?.invoiceNumber.trim()) return [invoice.precedingInvoice];
  return [];
}

export interface TaxBreakdown {
  categoryCode: TaxCategoryCode;
  rate: string;
  taxableAmount: string;
  taxAmount: string;
  exemptionReason?: string;
  exemptionReasonCode?: string;
}

export interface InvoiceTotals {
  lineNet: string;
  taxExclusive: string;
  taxTotal: string;
  taxInclusive: string;
  payable: string;
}

export interface CalculatedInvoice extends InvoiceInput {
  calculatedLines: Array<InvoiceLine & { netAmount: string }>;
  taxes: TaxBreakdown[];
  totals: InvoiceTotals;
}

export interface ValidationIssue {
  severity: "error" | "warning";
  code: string;
  path: string;
  message: string;
}

export interface ValidationResult {
  valid: boolean;
  issues: ValidationIssue[];
}
