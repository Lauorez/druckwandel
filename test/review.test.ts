import { describe, expect, it } from "vitest";
import { formatGermanDecimal, parseLocalizedDecimal } from "../src/domain/localized-decimal.js";
import { invoiceInputFromReview, unitCodeFromText, validateReviewDraft, type ReviewDraft } from "../src/review/draft.js";

const completeDraft: ReviewDraft = {
  invoiceNumber: "RE-1",
  issueDate: "2026-08-30",
  dueDate: "2026-09-13",
  serviceDate: "",
  currency: "eur",
  buyerReference: "04011000-12345-03",
  seller: { name: "Anbieter GmbH", addressLine1: "Straße 1", postalCode: "10115", city: "Berlin", countryCode: "de", vatId: "DE123456789", contactName: "Erika Muster", phone: "+49 30 123456", email: "rechnung@muster.invalid" },
  buyer: { name: "Kunde AG", addressLine1: "Weg 2", postalCode: "20095", city: "Hamburg", countryCode: "de", vatId: "" },
  payment: { iban: "DE89 3704 0044 0532 0130 00", bic: "COBADEFFXXX", terms: "14 Tage netto" },
  lines: [
    { id: "1", description: "Entwicklung", quantity: "10", unitCode: "HUR", netUnitPrice: "95", taxRate: "19", sourceTokenIds: [] },
    { id: "2", description: "Einrichtung", quantity: "1", unitCode: "C62", netUnitPrice: "150", taxRate: "19", sourceTokenIds: [] },
  ],
};

describe("German decimal input", () => {
  it("parses German grouping and decimal separators into canonical decimals", () => {
    expect(parseLocalizedDecimal("1.234.567,89 €")).toBe("1234567.89");
    expect(parseLocalizedDecimal("10,5")).toBe("10.5");
    expect(parseLocalizedDecimal("95.00")).toBe("95.00");
    expect(parseLocalizedDecimal("abc")).toBeNull();
  });

  it("formats canonical values for German display", () => {
    expect(formatGermanDecimal("1234567.89")).toBe("1.234.567,89");
    expect(formatGermanDecimal("10", 0, 3)).toBe("10");
    expect(formatGermanDecimal("12345678901234567890.125", 2, 3)).toBe("12.345.678.901.234.567.890,125");
  });
});

describe("review draft", () => {
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
      code: "REVERSE_CHARGE",
      message: "Reverse Charge wird noch nicht unterstützt.",
      sourceTokenIds: ["source-1"],
      sourceText: "Reverse Charge",
    }]);
    expect(result.valid).toBe(false);
    expect(result.issues).toContainEqual(expect.objectContaining({ code: "UNSUPPORTED_REVERSE_CHARGE", path: "document" }));
  });

  it("maps common German units to UNECE codes", () => {
    expect(unitCodeFromText("Std.")).toBe("HUR");
    expect(unitCodeFromText("Stk.")).toBe("C62");
    expect(unitCodeFromText("Meter")).toBe("MTR");
  });
});
