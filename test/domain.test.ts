import { describe, expect, it } from "vitest";
import { calculateInvoice } from "../src/domain/calculate.js";
import { validateCalculatedInvoice, validateEn16931InvoiceInput, validateInvoiceInput } from "../src/domain/validate.js";
import { standardInvoice } from "./fixtures/invoice.js";

describe("invoice domain", () => {
  it("rejects consistently forged totals and tax groups", () => {
    const invoice = calculateInvoice(standardInvoice);
    invoice.totals = { lineNet: "100.00", taxExclusive: "100.00", taxTotal: "19.00", taxInclusive: "119.00", payable: "119.00" };
    invoice.taxes = [{ categoryCode: "S", rate: "19", taxableAmount: "100.00", taxAmount: "19.00" }];
    expect(validateCalculatedInvoice(invoice).issues.map(issue => issue.code)).toEqual(expect.arrayContaining(["BR-CO-10", "BR-CO-17"]));
  });

  it("reports malformed calculated values instead of throwing", () => {
    const invoice = calculateInvoice(standardInvoice);
    invoice.totals.lineNet = "not a number";
    expect(validateCalculatedInvoice(invoice)).toMatchObject({ valid: false });
    invoice.allowances = [{ charge: false, amount: "invalid", tax: { categoryCode: "S", rate: "19" } }];
    expect(validateCalculatedInvoice(invoice).valid).toBe(false);
  });

  it("rejects overprecise adjustments and invalid service dates", () => {
    const invoice = { ...standardInvoice, serviceDate: "2026-02-30", allowances: [{ charge: false, amount: "0.005", tax: { categoryCode: "S" as const, rate: "19" } }] };
    expect(validateEn16931InvoiceInput(invoice).issues.map(issue => issue.code)).toEqual(expect.arrayContaining(["DATE", "AMOUNT_PRECISION"]));
    expect(validateEn16931InvoiceInput({ ...standardInvoice, issueDate: "0000-01-01" }).valid).toBe(false);
  });
  it("calculates money deterministically without binary floating point errors", () => {
    const invoice = calculateInvoice(standardInvoice);
    expect(invoice.calculatedLines.map((line) => line.netAmount)).toEqual(["200.00", "37.04"]);
    expect(invoice.totals).toEqual({ lineNet: "237.04", taxExclusive: "237.04", taxTotal: "45.04", taxInclusive: "282.08", payable: "282.08" });
  });

  it("groups taxes by category and rate", () => {
    const invoice = calculateInvoice({ ...standardInvoice, lines: [...standardInvoice.lines, { id: "3", name: "Pfand", quantity: "1", unitCode: "C62", netUnitPrice: "10", tax: { categoryCode: "Z", rate: "0" } }] });
    expect(invoice.taxes).toHaveLength(2);
    expect(invoice.taxes[1]).toMatchObject({ categoryCode: "Z", taxableAmount: "10.00", taxAmount: "0.00" });
  });

  it("rejects missing XRechnung buyer reference and exemption reason", () => {
    const invalid = { ...standardInvoice, buyerReference: "", lines: [{ ...standardInvoice.lines[0]!, tax: { categoryCode: "E" as const, rate: "0" } }] };
    const result = validateInvoiceInput(invalid);
    expect(result.valid).toBe(false);
    expect(result.issues.map((issue) => issue.code)).toEqual(expect.arrayContaining(["BR-DE-15", "BR-E-10"]));
  });

  it("allows a missing buyer reference for the base EN16931 ZUGFeRD profile", () => {
    const result = validateEn16931InvoiceInput({ ...standardInvoice, buyerReference: "" });
    expect(result.valid).toBe(true);
    expect(result.issues.map((issue) => issue.code)).not.toContain("BR-DE-15");
  });

  it("rejects calendar dates that only look like ISO dates", () => {
    expect(validateEn16931InvoiceInput({ ...standardInvoice, issueDate: "2026-02-29" }).valid).toBe(false);
    expect(validateEn16931InvoiceInput({ ...standardInvoice, issueDate: "2024-02-29" }).valid).toBe(true);
  });

  it("detects tampered totals", () => {
    const invoice = calculateInvoice(standardInvoice);
    invoice.totals.taxInclusive = "999.00";
    expect(validateCalculatedInvoice(invoice).issues.map((issue) => issue.code)).toContain("BR-CO-15");
  });

  it("applies document and line allowances to tax groups without inventing rounding rows", () => {
    const invoice = calculateInvoice({
      ...standardInvoice,
      lines: [{
        ...standardInvoice.lines[0]!,
        allowances: [{ charge: false, amount: "5.00", reason: "Positionsrabatt", tax: { categoryCode: "S", rate: "19" } }],
      }, standardInvoice.lines[1]!],
      allowances: [{ charge: false, amount: "10.00", reason: "Rabatt", tax: { categoryCode: "S", rate: "19" } }],
    });
    expect(invoice.calculatedLines[0]?.netAmount).toBe("195.00");
    expect(invoice.totals).toEqual({ lineNet: "232.04", taxExclusive: "222.04", taxTotal: "42.19", taxInclusive: "264.23", payable: "264.23" });
    expect(invoice.taxes[0]).toMatchObject({ taxableAmount: "222.04", taxAmount: "42.19" });
  });

  it("rejects standard tax at 0 % and reverse charge without the buyer's VAT ID", () => {
    expect(validateEn16931InvoiceInput({
      ...standardInvoice,
      lines: [{ ...standardInvoice.lines[0]!, tax: { categoryCode: "S", rate: "0" } }],
    }).issues.map((issue) => issue.code)).toContain("BR-S-5");
    const withoutBuyerVat = {
      ...standardInvoice,
      buyer: { ...standardInvoice.buyer },
      lines: [{ ...standardInvoice.lines[0]!, tax: { categoryCode: "AE" as const, rate: "0", exemptionReason: "Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG" } }],
    };
    delete withoutBuyerVat.buyer.vatId;
    expect(validateEn16931InvoiceInput(withoutBuyerVat).issues.map((issue) => issue.code)).toContain("BR-AE-2");
  });

  it("accepts reverse charge with both VAT IDs and keeps tax at 0", () => {
    const invoice = calculateInvoice({
      ...standardInvoice,
      lines: standardInvoice.lines.map((line) => ({
        ...line,
        tax: { categoryCode: "AE" as const, rate: "0", exemptionReason: "Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG", exemptionReasonCode: "vatex-eu-ae" },
      })),
    });
    expect(validateEn16931InvoiceInput(invoice).valid).toBe(true);
    expect(invoice.taxes[0]).toMatchObject({ categoryCode: "AE", taxAmount: "0.00", exemptionReasonCode: "vatex-eu-ae" });
    expect(invoice.totals.taxTotal).toBe("0.00");
  });

  it("requires a preceding invoice for credit notes and corrections", () => {
    expect(validateInvoiceInput({ ...standardInvoice, invoiceType: "381" }).issues.map((issue) => issue.code)).toContain("BT-25");
    expect(validateInvoiceInput({
      ...standardInvoice,
      invoiceType: "384",
      precedingInvoice: { invoiceNumber: "RE-ALT", issueDate: "2026-08-01" },
    }).valid).toBe(true);
  });

  it("reduces the payable amount by confirmed prepaid amounts and preceding invoices", () => {
    const finalInvoice = calculateInvoice({
      ...standardInvoice,
      finalInvoice: true,
      prepaidAmount: "100.00",
      precedingInvoices: [{ invoiceNumber: "RE-A-1", issueDate: "2026-07-01", paidAmount: "100.00" }],
    });
    expect(finalInvoice.totals).toEqual({ lineNet: "237.04", taxExclusive: "237.04", taxTotal: "45.04", taxInclusive: "282.08", payable: "182.08" });
    expect(validateInvoiceInput(finalInvoice).valid).toBe(true);
    expect(validateInvoiceInput({ ...standardInvoice, invoiceType: "326" }).valid).toBe(true);
    expect(validateInvoiceInput({ ...standardInvoice, prepaidAmount: "10.00" }).issues.map((issue) => issue.code)).toContain("BT-25");
    expect(validateInvoiceInput({ ...standardInvoice, finalInvoice: true, precedingInvoices: [{ invoiceNumber: "RE-A-1" }] }).issues.map((issue) => issue.code)).toContain("BT-113");
  });
});
