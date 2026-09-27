import type { InvoiceInput } from "../../src/domain/types.js";
import { standardInvoice } from "./invoice.js";

/** Exercise every supported document and tax branch with real validators. */
export const exportInvoices: Record<string, InvoiceInput> = {
  standard: standardInvoice,
  adjustments: {
    ...standardInvoice,
    allowances: [
      { charge: false, amount: "10.00", reason: "Rabatt", tax: { categoryCode: "S", rate: "19" } },
      { charge: true, amount: "5.00", reason: "Versand", tax: { categoryCode: "S", rate: "19" } },
    ],
    lines: standardInvoice.lines.map(line => ({ ...line, allowances: [
      { charge: false, amount: "1.00", reason: "Positionsrabatt", tax: { categoryCode: "S", rate: "19" } },
    ] })),
  },
  credit: { ...standardInvoice, invoiceType: "381", precedingInvoice: { invoiceNumber: "RE-ALT", issueDate: "2026-08-01" } },
  correction: { ...standardInvoice, invoiceType: "384", precedingInvoice: { invoiceNumber: "RE-ALT", issueDate: "2026-08-01" } },
  partial: { ...standardInvoice, invoiceNumber: "RE-A-1", invoiceType: "326" },
  prepayment: { ...standardInvoice, invoiceNumber: "RE-ANZ-1", prepaymentInvoice: true },
  final: {
    ...standardInvoice,
    invoiceNumber: "RE-S-1",
    finalInvoice: true,
    prepaidAmount: "100.00",
    precedingInvoice: { invoiceNumber: "RE-A-1", issueDate: "2026-07-01", paidAmount: "100.00" },
    precedingInvoices: [{ invoiceNumber: "RE-A-1", issueDate: "2026-07-01", paidAmount: "100.00" }],
  },
  reverseCharge: { ...standardInvoice, lines: standardInvoice.lines.map(line => ({ ...line,
    tax: { categoryCode: "AE", rate: "0", exemptionReason: "Reverse charge", exemptionReasonCode: "vatex-eu-ae" },
  })) },
  exempt: { ...standardInvoice, lines: standardInvoice.lines.map(line => ({ ...line,
    tax: { categoryCode: "E", rate: "0", exemptionReason: "Steuerbefreite Leistung" },
  })) },
  intraCommunity: { ...standardInvoice, deliveryAddress: { line1: "Lieferweg 1", city: "Wien", postalCode: "1010", countryCode: "AT" }, lines: standardInvoice.lines.map(line => ({ ...line,
    tax: { categoryCode: "K", rate: "0", exemptionReason: "Intra-community supply", exemptionReasonCode: "vatex-eu-ic" },
  })) },
  zero: { ...standardInvoice, lines: standardInvoice.lines.map(line => ({ ...line, tax: { categoryCode: "Z", rate: "0" } })) },
};
