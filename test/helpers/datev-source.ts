import { createHash } from "node:crypto";
import { calculateInvoice } from "../../src/domain/calculate.js";
import type { InvoiceInput } from "../../src/domain/types.js";
import { generateCii } from "../../src/engine/cii.js";
import { generateUbl } from "../../src/engine/ubl.js";
import { invoiceSnapshot } from "../../src/export/invoice-snapshot.js";
import type { DatevProfile, DatevSource } from "../../src/export/datev/types.js";
import { standardInvoice } from "../fixtures/invoice.js";

export function profile(): DatevProfile {
  return {schemaVersion:1,name:"Testkanzlei",consultant:"1001",client:"12345",fiscalYearStart:"2026-01-01",accountLength:4,chart:"03",seller:structuredClone(standardInvoice.seller),confirmed:true,accountingMethod:"accrual",periodRule:"invoice-date",locking:"0",collectiveDebtor:"10000",collectiveDebtorConfirmed:true,debtors:[],revenueAccounts:[{id:"19",label:"Erlöse 19 %",taxRate:"19",account:"8400",mode:"automatic",taxKey:""},{id:"7",label:"Erlöse 7 %",taxRate:"7",account:"8300",mode:"automatic",taxKey:""}]};
}

export function source(input: InvoiceInput = standardInvoice, format: DatevSource["format"] = "xrechnung", documentId = "document-1"): DatevSource {
  const invoice = calculateInvoice(input);
  const snapshot = invoiceSnapshot(invoice);
  return {archiveId:`${documentId}-${format}`,documentId,contentHash:createHash("sha256").update(snapshot).digest("hex"),originalHash:"original",snapshot,format,xml:format==="xrechnung"?generateUbl(invoice):generateCii(invoice)};
}
