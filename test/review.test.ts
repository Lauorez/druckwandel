import { describe, expect, it } from "vitest";
import { formatGermanDecimal, parseLocalizedDecimal } from "../src/domain/localized-decimal.js";
import { invoiceInputFromReview, reviewDraftFromExtraction, unitCodeFromText, validateReviewDraft, type ReviewDraft } from "../src/review/draft.js";
import type { ExtractionResult } from "../src/extraction/types.js";

const completeDraft: ReviewDraft = {
  invoiceNumber: "RE-1",
  invoiceType: "380",
  finalInvoice: false,
  prepaymentInvoice: false,
  precedingInvoiceNumber: "",
  precedingInvoiceDate: "",
  precedingInvoices: [],
  prepaidAmount: "",
  issueDate: "2026-08-30",
  dueDate: "2026-09-13",
  serviceDate: "",
  currency: "eur",
  buyerReference: "04011000-12345-03",
  seller: { name: "Anbieter GmbH", addressLine1: "Straße 1", postalCode: "10115", city: "Berlin", countryCode: "de", vatId: "DE123456789", contactName: "Erika Muster", phone: "+49 30 123456", email: "rechnung@muster.invalid" },
  buyer: { name: "Kunde AG", addressLine1: "Weg 2", postalCode: "20095", city: "Hamburg", countryCode: "de", vatId: "" },
  payment: { iban: "DE89 3704 0044 0532 0130 00", bic: "COBADEFFXXX", terms: "14 Tage netto" },
  lines: [
    { id: "1", description: "Entwicklung", quantity: "10", unitCode: "HUR", netUnitPrice: "95", taxRate: "19", taxCase: "S19", exemptionReason: "", sourceTokenIds: [], allowances: [] },
    { id: "2", description: "Einrichtung", quantity: "1", unitCode: "C62", netUnitPrice: "150", taxRate: "19", taxCase: "S19", exemptionReason: "", sourceTokenIds: [], allowances: [] },
  ],
  allowances: [],
};

describe("German decimal input", () => {
  it("parses German grouping and decimal separators into canonical decimals", () => {
    expect(parseLocalizedDecimal("1.234.567,89 €")).toBe("1234567.89");
    expect(parseLocalizedDecimal("10,5")).toBe("10.5");
    expect(parseLocalizedDecimal("95.00")).toBe("95.00");
    expect(parseLocalizedDecimal("abc")).toBeNull();
    expect(parseLocalizedDecimal("12.34,56")).toBeNull();
    expect(parseLocalizedDecimal("1.2.3,45")).toBeNull();
  });

  it("formats canonical values for German display", () => {
    expect(formatGermanDecimal("1234567.89")).toBe("1.234.567,89");
    expect(formatGermanDecimal("10", 0, 3)).toBe("10");
    expect(formatGermanDecimal("1.000", 0, 3)).toBe("1");
    expect(formatGermanDecimal("12.345", 2, 3)).toBe("12,345");
    expect(formatGermanDecimal("12,")).toBe("12,");
    expect(formatGermanDecimal("12345678901234567890.125", 2, 3)).toBe("12.345.678.901.234.567.890,125");
  });
});

describe("review draft", () => {
  it("blocks discounts that make a position or tax base negative", () => {
    const allowance = { id: "a1", charge: false, reason: "Rabatt", amount: "99999", taxRate: "19", taxCase: "S19" as const, exemptionReason: "" };
    expect(validateReviewDraft({ ...completeDraft, allowances: [allowance] }).valid).toBe(false);
    expect(validateReviewDraft({ ...completeDraft, lines: [{ ...completeDraft.lines[0]!, allowances: [allowance] }] }).valid).toBe(false);
  });

  it("requires explicit delivery data for intra-community supply", () => {
    const draft = { ...completeDraft, buyer: { ...completeDraft.buyer, vatId: "ATU12345678" }, serviceDate: "",
      lines: [{ ...completeDraft.lines[0]!, taxCase: "K" as const, taxRate: "0", exemptionReason: "Innergemeinschaftliche Lieferung" }] };
    expect(validateReviewDraft(draft).issues.map(issue => issue.code)).toEqual(expect.arrayContaining(["BR-IC-11", "BR-IC-12"]));
    const ready = validateReviewDraft({ ...draft, serviceDate: "2026-08-19", deliveryAddress: { line1: "", city: "Wien", postalCode: "1010", countryCode: "AT" } });
    expect(ready.valid).toBe(true);
    expect(ready.invoice?.deliveryAddress?.countryCode).toBe("AT");
  });
  it("normalizes reviewed data and calculates a valid XRechnung input", () => {
    const input = invoiceInputFromReview(completeDraft);
    expect(input.currency).toBe("EUR");
    expect(input.seller.address.countryCode).toBe("DE");
    expect(input.payment.iban).toBe("DE89370400440532013000");
    expect(input.seller.electronicAddress).toEqual({ value: "DE123456789", schemeId: "9930" });
    expect(input.buyer.electronicAddress).toEqual({ value: "04011000-12345-03", schemeId: "0204" });
    expect(input.seller.contact).toEqual({ name: "Erika Muster", phone: "+49 30 123456", email: "rechnung@muster.invalid" });
    expect(validateReviewDraft(completeDraft)).toMatchObject({
      valid: true,
      invoice: { totals: { lineNet: "1100.00", taxTotal: "209.00", payable: "1309.00" } },
    });
  });

  it("keeps the create action invalid until required values are present", () => {
    const result = validateReviewDraft({ ...completeDraft, buyerReference: "", lines: [] });
    expect(result.valid).toBe(false);
    expect(result.issues.map((issue) => issue.code)).toEqual(expect.arrayContaining(["BR-DE-15", "BR-16"]));
  });

  it("requires seller contact and electronic addresses only for XRechnung", () => {
    const withoutContact = { ...completeDraft, seller: { ...completeDraft.seller, email: "" } };
    expect(validateReviewDraft(withoutContact).valid).toBe(false);
    expect(validateReviewDraft(withoutContact).issues.map((issue) => issue.code)).toContain("BR-DE-2");
    expect(validateReviewDraft(withoutContact, [], "zugferd").valid).toBe(true);
  });

  it("uses separate XRechnung and ZUGFeRD validation gates", () => {
    const withoutBuyerReference = { ...completeDraft, buyerReference: "" };
    expect(validateReviewDraft(withoutBuyerReference).valid).toBe(false);
    expect(validateReviewDraft(withoutBuyerReference, [], "zugferd").valid).toBe(true);
  });

  it("keeps generation blocked for an explicitly unsupported source document", () => {
    const result = validateReviewDraft(completeDraft, [{
      code: "OCR_REQUIRED",
      message: "Der Text dieser eingescannten Rechnung konnte nicht gelesen werden.",
      sourceTokenIds: [],
      sourceText: "OCR erforderlich.",
    }]);
    expect(result.valid).toBe(false);
    expect(result.issues).toContainEqual(expect.objectContaining({ code: "UNSUPPORTED_OCR_REQUIRED", path: "document" }));
  });

  it("maps common German units to UNECE codes", () => {
    expect(unitCodeFromText("Std.")).toBe("HUR");
    expect(unitCodeFromText("Stk.")).toBe("C62");
    expect(unitCodeFromText("Meter")).toBe("MTR");
  });

  it("turns a detected credit note and discount into a complete invoice input", () => {
    const credit = validateReviewDraft({
      ...completeDraft,
      invoiceType: "381",
      precedingInvoiceNumber: "RE-100",
      precedingInvoiceDate: "01.08.2026",
      allowances: [{ id: "a1", charge: false, reason: "Rabatt", amount: "10", taxRate: "19", taxCase: "S19", exemptionReason: "" }],
    });
    expect(credit.valid).toBe(true);
    expect(credit.invoice?.invoiceType).toBe("381");
    expect(credit.invoice?.precedingInvoice).toEqual({ invoiceNumber: "RE-100", issueDate: "2026-08-01" });
    expect(credit.invoice?.totals.taxExclusive).toBe("1090.00");
    expect(validateReviewDraft({ ...completeDraft, invoiceType: "381" }).issues.map((issue) => issue.code)).toContain("BT-25");
  });

  it("turns detected advance and final invoices into prepaid amounts and document references", () => {
    const extraction: ExtractionResult = {
      pages: [{ page: 1, width: 500, height: 700, tokens: [] }],
      lines: [
        { id: "title", page: 1, text: "Schlussrechnung", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["title"] },
        { id: "prev", page: 1, text: "Abschlag 1: RE-A-1 vom 01.07.2026 100,00", box: { x: 10, y: 30, width: 180, height: 10 }, tokenIds: ["prev"] },
      ],
      fields: {},
      lineItems: [],
      warnings: [],
      usedOcr: false,
    };
    const detected = reviewDraftFromExtraction(extraction);
    expect(detected.finalInvoice).toBe(true);
    expect(detected.invoiceType).toBe("380");
    expect(detected.precedingInvoices[0]).toMatchObject({ invoiceNumber: "RE-A-1", paidAmount: "100.00" });
    const finalDraft = validateReviewDraft({
      ...completeDraft,
      finalInvoice: true,
      prepaidAmount: "100",
      precedingInvoices: [{ invoiceNumber: "RE-A-1", issueDate: "2026-07-01", paidAmount: "100" }],
    });
    expect(finalDraft.valid).toBe(true);
    expect(finalDraft.invoice?.totals.payable).toBe("1209.00");
    expect(finalDraft.invoice?.precedingInvoices).toEqual([{ invoiceNumber: "RE-A-1", issueDate: "2026-07-01", paidAmount: "100" }]);
    expect(validateReviewDraft({ ...completeDraft, finalInvoice: true }).issues.map((issue) => issue.code)).toEqual(expect.arrayContaining(["BT-25", "BT-113"]));
    expect(reviewDraftFromExtraction({
      ...extraction,
      lines: [{ id: "title", page: 1, text: "Abschlagsrechnung", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["title"] }],
    }).invoiceType).toBe("326");
    const prepayment = validateReviewDraft({ ...completeDraft, prepaymentInvoice: true });
    expect(prepayment.valid).toBe(true);
    expect(prepayment.invoice?.invoiceType).toBe("380");
    expect(prepayment.invoice?.prepaymentInvoice).toBe(true);
    expect(prepayment.invoice?.notes).toContain("Anzahlungsrechnung");
  });

  it("does not treat a bare 0 % rate as Reverse Charge, exemption or zero-rated", () => {
    const draft = {
      ...completeDraft,
      lines: [{ ...completeDraft.lines[0]!, taxRate: "0", taxCase: "" as const, exemptionReason: "" }],
    };
    const result = validateReviewDraft(draft);
    expect(result.valid).toBe(false);
    expect(result.issues.map((issue) => issue.code)).toContain("TAX_CASE");
    expect(invoiceInputFromReview(draft).lines[0]?.tax.categoryCode).not.toBe("Z");
  });

  it("requires VAT IDs and an exemption reason for reverse charge", () => {
    const withoutBuyerVat = {
      ...completeDraft,
      lines: [{ ...completeDraft.lines[0]!, taxCase: "AE" as const, taxRate: "0", exemptionReason: "Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG" }],
    };
    expect(validateReviewDraft(withoutBuyerVat).issues.map((issue) => issue.code)).toEqual(expect.arrayContaining(["BR-AE-2"]));
    const complete = {
      ...withoutBuyerVat,
      buyer: { ...completeDraft.buyer, vatId: "DE987654321" },
    };
    const result = validateReviewDraft(complete);
    expect(result.valid).toBe(true);
    expect(result.invoice?.lines[0]?.tax).toMatchObject({ categoryCode: "AE", rate: "0", exemptionReasonCode: "VATEX-EU-AE" });
    expect(result.invoice?.notes).toEqual([expect.stringMatching(/13b/)]);
  });

  it("requires a typed reason for tax exemption and VAT IDs for intra-community supply", () => {
    const exempt = {
      ...completeDraft,
      lines: [{ ...completeDraft.lines[0]!, taxCase: "E" as const, taxRate: "0", exemptionReason: "" }],
    };
    expect(validateReviewDraft(exempt).issues.map((issue) => issue.code)).toContain("BR-E-10");
    const intra = {
      ...completeDraft,
      lines: [{ ...completeDraft.lines[0]!, taxCase: "K" as const, taxRate: "0", exemptionReason: "Innergemeinschaftliche Lieferung" }],
    };
    expect(validateReviewDraft(intra).issues.map((issue) => issue.code)).toEqual(expect.arrayContaining(["BR-IC-2"]));
  });

  it("preselects reverse charge only on 0 % lines", () => {
    const extraction: ExtractionResult = {
      pages: [{ page: 1, width: 500, height: 700, tokens: [] }],
      lines: [{ id: "hint", page: 1, text: "Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG", box: { x: 10, y: 10, width: 80, height: 10 }, tokenIds: ["hint"] }],
      fields: {},
      lineItems: [
        { description: "Beratung", quantity: "2", netUnitPrice: "100", netAmount: "200", taxRate: "0.00", confidence: 0.9, sourceTokenIds: [], sourceText: "Beratung" },
        { description: "Hardware", quantity: "1", netUnitPrice: "10", netAmount: "10", taxRate: "19.00", confidence: 0.9, sourceTokenIds: [], sourceText: "Hardware" },
      ],
      warnings: [],
      usedOcr: false,
    };
    const draft = reviewDraftFromExtraction(extraction);
    expect(draft.lines[0]).toMatchObject({ taxCase: "AE", taxRate: "0", exemptionReason: "Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG" });
    expect(draft.lines[1]).toMatchObject({ taxCase: "S19", taxRate: "19", exemptionReason: "" });
  });
});
