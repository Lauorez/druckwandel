import type { Party } from "../../domain/types.js";

export interface RevenueAccount {
  id: string;
  label: string;
  taxRate: "7"|"19";
  account: string;
  mode: "automatic"|"tax-key";
  taxKey: string;
}
export interface DatevProfile {
  schemaVersion: 1;
  name: string;
  consultant: string;
  client: string;
  fiscalYearStart: string;
  accountLength: number;
  chart: "03"|"04";
  seller: Party;
  confirmed: boolean;
  accountingMethod: "accrual"|"cash"|"";
  periodRule: "invoice-date"|"service-date"|"";
  locking: "0"|"1"|"";
  collectiveDebtor: string;
  collectiveDebtorConfirmed: boolean;
  debtors: Array<{buyerIdentity:string;account:string}>;
  revenueAccounts: RevenueAccount[];
}
export interface DatevSource {
  archiveId: string;
  documentId: string;
  contentHash: string;
  originalHash: string;
  snapshot: string;
  xml: string;
  format: "xrechnung"|"zugferd";
}
export interface InvoiceAssignment {
  archiveId: string;
  /** Optional explicit allocation of invoice lines to configured revenue accounts. */
  lineAccounts?: Record<string,string>;
  bookingText?: string;
}
export interface Booking {
  archiveId: string;
  documentId: string;
  contentHash: string;
  duplicateKey: string;
  invoiceNumber: string;
  issueDate: string;
  dueDate: string;
  serviceDate: string;
  taxPeriodDate: string;
  debtor: string;
  revenueAccount: string;
  taxKey: string;
  taxRate: string;
  net: string;
  tax: string;
  gross: string;
  text: string;
  belegGuid: string;
}
export interface DatevDocumentFile {
  archiveId: string;
  guid: string;
  pdfName: string;
  xmlName: string;
}
export interface DatevDocumentPackage {
  xml: string;
  files: DatevDocumentFile[];
}
export interface DatevBatch {
  fiscalYearStart: string;
  dateFrom: string;
  dateTo: string;
  bookings: Booking[];
  gross: string;
}
export interface DatevPreview { batches: DatevBatch[]; issues: string[]; invoiceCount:number; gross:string }
