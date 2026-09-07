export type InvoiceTypeCode = "380" | "381";
export type TaxCategoryCode = "S" | "Z" | "E" | "AE";
export type UnitCode = "C62" | "HUR" | "DAY" | "KGM" | "LTR" | "MTR";

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

export interface InvoiceLine {
  id: string;
  name: string;
  description?: string;
  quantity: string;
  unitCode: UnitCode;
  netUnitPrice: string;
  tax: { categoryCode: TaxCategoryCode; rate: string; exemptionReason?: string };
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
  currency: string;
  buyerReference: string;
  seller: Party;
  buyer: Party;
  lines: InvoiceLine[];
  payment: PaymentInfo;
  notes?: string[];
}

export interface TaxBreakdown {
  categoryCode: TaxCategoryCode;
  rate: string;
  taxableAmount: string;
  taxAmount: string;
  exemptionReason?: string;
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
