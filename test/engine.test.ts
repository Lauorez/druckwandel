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
