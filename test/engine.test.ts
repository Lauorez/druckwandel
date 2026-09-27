import { PDFDocument } from "pdf-lib";
import { describe, expect, it } from "vitest";
import { calculateInvoice } from "../src/domain/calculate.js";
import { EInvoiceEngine } from "../src/engine/e-invoice-engine.js";
import { generateCii, ZUGFERD_EN16931_GUIDELINE } from "../src/engine/cii.js";
import { generateUbl, XRECHNUNG_CUSTOMIZATION_ID } from "../src/engine/ubl.js";
import { embedCiiInPdf } from "../src/engine/hybrid-pdf.js";
import { generateSyntheticInvoice } from "../src/evaluation/synthetic-invoice.js";
import { extractInvoicePdf } from "../src/extraction/index.js";
import { invoiceInputFromReview, reviewDraftFromExtraction, validateReviewDraft } from "../src/review/draft.js";
import { standardInvoice } from "./fixtures/invoice.js";

const invoice = calculateInvoice(standardInvoice);

describe("e-invoice formats", () => {
  it("generates EN16931 CII with escaped content", () => {
    const cii = generateCii({ ...invoice, notes: ["A & B < C"] });
    expect(cii).toContain(ZUGFERD_EN16931_GUIDELINE);
    expect(cii).toContain("A &amp; B &lt; C");
    expect(cii).toContain("<ram:DuePayableAmount>282.08</ram:DuePayableAmount>");
  });

  it("generates XRechnung UBL 2.1", () => {
    const ubl = generateUbl(invoice);
    expect(ubl).toContain('xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"');
    expect(ubl).toContain(XRECHNUNG_CUSTOMIZATION_ID);
    expect(ubl).toContain('<cbc:PayableAmount currencyID="EUR">282.08</cbc:PayableAmount>');
    expect(ubl).not.toContain("AllowanceTotalAmount");
    expect(ubl).not.toContain("BillingReference");
  });

  it("emits a credit note with preceding invoice and document allowance", () => {
    const credit = calculateInvoice({
      ...standardInvoice,
      invoiceNumber: "GS-1",
      invoiceType: "381",
      precedingInvoice: { invoiceNumber: "RE-2026-0001", issueDate: "2026-08-01" },
      allowances: [{ charge: false, amount: "10.00", reason: "Rabatt", tax: { categoryCode: "S", rate: "19" } }],
    });
    const ubl = generateUbl(credit);
    expect(ubl).toContain("CreditNote");
    expect(ubl).toContain("<cbc:CreditNoteTypeCode>381</cbc:CreditNoteTypeCode>");
    expect(ubl).toContain("<cbc:CreditedQuantity");
    expect(ubl).toContain("<cbc:ID>RE-2026-0001</cbc:ID>");
    expect(ubl).toContain("<cbc:ChargeIndicator>false</cbc:ChargeIndicator>");
    expect(ubl).toContain("AllowanceTotalAmount");
    const cii = generateCii(credit);
    expect(cii).toContain("<ram:TypeCode>381</ram:TypeCode>");
    expect(cii).toContain("<ram:IssuerAssignedID>RE-2026-0001</ram:IssuerAssignedID>");
    expect(cii).toContain("<qdt:DateTimeString format=\"102\">20260801</qdt:DateTimeString>");
    expect(cii).toContain("<ram:AllowanceTotalAmount>10.00</ram:AllowanceTotalAmount>");
  });

  it("emits prepaid amounts and multiple preceding invoices for a final invoice", () => {
    const finalInvoice = calculateInvoice({
      ...standardInvoice,
      invoiceNumber: "RE-S-1",
      finalInvoice: true,
      prepaidAmount: "100.00",
      precedingInvoices: [
        { invoiceNumber: "RE-A-1", issueDate: "2026-07-01", paidAmount: "40.00" },
        { invoiceNumber: "RE-A-2", issueDate: "2026-08-01", paidAmount: "60.00" },
      ],
    });
    const ubl = generateUbl(finalInvoice);
    expect(ubl).toContain("<cbc:InvoiceTypeCode>380</cbc:InvoiceTypeCode>");
    expect(ubl).toContain('<cbc:PrepaidAmount currencyID="EUR">100.00</cbc:PrepaidAmount>');
    expect(ubl).toContain('<cbc:PayableAmount currencyID="EUR">182.08</cbc:PayableAmount>');
    expect(ubl).toContain("<cbc:ID>RE-A-1</cbc:ID>");
    expect(ubl).toContain("<cbc:ID>RE-A-2</cbc:ID>");
    const cii = generateCii(finalInvoice);
    expect(cii).toContain("<ram:TotalPrepaidAmount>100.00</ram:TotalPrepaidAmount>");
    expect(cii).toContain("<ram:DuePayableAmount>182.08</ram:DuePayableAmount>");
    expect(cii).toContain("<ram:IssuerAssignedID>RE-A-1</ram:IssuerAssignedID>");
    expect(generateUbl(calculateInvoice({ ...standardInvoice, invoiceType: "326" }))).toContain("<cbc:InvoiceTypeCode>326</cbc:InvoiceTypeCode>");
    const prepayment = generateUbl(calculateInvoice({ ...standardInvoice, invoiceNumber: "RE-ANZ-1", prepaymentInvoice: true, notes: ["Anzahlungsrechnung"] }));
    expect(prepayment).toContain("<cbc:InvoiceTypeCode>380</cbc:InvoiceTypeCode>");
    expect(prepayment).toContain("Anzahlungsrechnung");
  });

  it("emits exemption reason and code for reverse charge", () => {
    const reverseCharge = calculateInvoice({
      ...standardInvoice,
      notes: ["Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG"],
      lines: [{
        ...standardInvoice.lines[0]!,
        tax: {
          categoryCode: "AE",
          rate: "0",
          exemptionReason: "Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG",
          exemptionReasonCode: "VATEX-EU-AE",
        },
      }],
    });
    const ubl = generateUbl(reverseCharge);
    expect(ubl).toContain("<cbc:TaxExemptionReasonCode>VATEX-EU-AE</cbc:TaxExemptionReasonCode>");
    expect(ubl).toContain("<cbc:TaxExemptionReason>Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG</cbc:TaxExemptionReason>");
    expect(ubl).toContain("<cbc:ID>AE</cbc:ID>");
    const cii = generateCii(reverseCharge);
    expect(cii).toContain("<ram:ExemptionReasonCode>VATEX-EU-AE</ram:ExemptionReasonCode>");
    expect(cii).toContain("<ram:ExemptionReason>Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG</ram:ExemptionReason>");
    expect(cii).toContain("<ram:CategoryCode>AE</ram:CategoryCode>");
  });

  it("requires the German buyer reference only for XRechnung", () => {
    const engine = new EInvoiceEngine();
    const withoutBuyerReference = engine.calculate({ ...standardInvoice, buyerReference: "" });
    expect(engine.cii(withoutBuyerReference)).not.toContain("<ram:BuyerReference>");
    expect(() => engine.xrechnung(withoutBuyerReference)).toThrow("BR-DE-15");
  });

  it("embeds factur-x.xml with Alternative relationship", async () => {
    const source = await PDFDocument.create();
    source.addPage([420, 595]);
    const sourceBytes = await source.save();
    const result = await new EInvoiceEngine().zugferd(sourceBytes, invoice);
    const text = new TextDecoder("latin1").decode(result);
    const reopened = await PDFDocument.load(result);
    expect(result.length).toBeGreaterThan(sourceBytes.length);
    expect(reopened.getPageCount()).toBe(1);
    expect(reopened.getPage(0).getSize()).toEqual({ width: 420, height: 595 });
    expect(text).toContain("factur-x.xml");
    expect(text).toContain("/Alternative");
    expect(text).toContain("pdfaid:part=\"3\"");
    expect(text).toContain("fx:ConformanceLevel=\"EN 16931\"");
  });

  it("returns the exact embedded XML together with the ZUGFeRD PDF", async () => {
    const source = await PDFDocument.create();
    source.addPage();
    const result = await new EInvoiceEngine().zugferdPackage(await source.save(), invoice);
    expect(result.xml).toBe(generateCii(invoice));
    expect(new TextDecoder("latin1").decode(result.pdf)).toContain("factur-x.xml");
  });

  it("preserves the visible invoice through extraction, review and ZUGFeRD export", async () => {
    const fixture = await generateSyntheticInvoice(1_042);
    const sourceExtraction = await extractInvoicePdf(fixture.pdf);
    const draft = reviewDraftFromExtraction(sourceExtraction);
    expect(validateReviewDraft(draft).valid).toBe(true);

    const engine = new EInvoiceEngine();
    const reviewedInvoice = engine.calculate(invoiceInputFromReview(draft));
    const hybridPdf = await embedCiiInPdf(fixture.pdf, generateCii(reviewedInvoice));
    const hybridExtraction = await extractInvoicePdf(hybridPdf);

    expect(hybridExtraction.fields.invoiceNumber?.value).toBe(sourceExtraction.fields.invoiceNumber?.value);
    expect(hybridExtraction.fields.payable?.value).toBe(sourceExtraction.fields.payable?.value);
    expect(hybridExtraction.lineItems.map(({ description, quantity, netAmount }) => ({ description, quantity, netAmount })))
      .toEqual(sourceExtraction.lineItems.map(({ description, quantity, netAmount }) => ({ description, quantity, netAmount })));
  });
});
