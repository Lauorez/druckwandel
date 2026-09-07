import type { InvoiceInput } from "../../src/domain/types.js";

export const standardInvoice: InvoiceInput = {
  invoiceNumber: "RE-2026-0001",
  invoiceType: "380",
  issueDate: "2026-08-20",
  dueDate: "2026-09-03",
  serviceDate: "2026-08-19",
  currency: "EUR",
  buyerReference: "04011000-12345-03",
  seller: {
    name: "Muster Elektro GmbH",
    address: { line1: "Werkstraße 1", city: "Berlin", postalCode: "10115", countryCode: "DE" },
    vatId: "DE123456789",
    electronicAddress: { value: "0204:DE123456789", schemeId: "0204" },
    contact: { name: "Erika Muster", email: "rechnung@muster.invalid", phone: "+49 30 123456" },
  },
  buyer: {
    name: "Beispiel Bau AG",
    address: { line1: "Kundenweg 9", city: "Hamburg", postalCode: "20095", countryCode: "DE" },
    vatId: "DE987654321",
    electronicAddress: { value: "buyer@example.invalid", schemeId: "EM" },
  },
  lines: [
    { id: "1", name: "Montagestunde", quantity: "2.5", unitCode: "HUR", netUnitPrice: "80.00", tax: { categoryCode: "S", rate: "19" } },
    { id: "2", name: "Kabel", quantity: "3", unitCode: "MTR", netUnitPrice: "12.345", tax: { categoryCode: "S", rate: "19" } },
  ],
  payment: { meansCode: "58", iban: "DE02120300000000202051", bic: "BYLADEM1001", accountName: "Muster Elektro GmbH", paymentReference: "RE-2026-0001", terms: "Zahlbar ohne Abzug innerhalb von 14 Tagen." },
  notes: ["Vielen Dank für Ihren Auftrag."],
};
