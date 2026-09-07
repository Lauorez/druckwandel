import { describe, expect, it } from "vitest";
import { calculateInvoice } from "../src/domain/calculate.js";
import { validateCalculatedInvoice, validateEn16931InvoiceInput, validateInvoiceInput } from "../src/domain/validate.js";
import { standardInvoice } from "./fixtures/invoice.js";

describe("invoice domain", () => {
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
});
