import { describe, expect, it } from "vitest";
import { PDFDocument, StandardFonts } from "pdf-lib";
import { analyzeDocumentPages } from "../src/extraction/analyze.js";
import { extractInvoicePdf } from "../src/extraction/index.js";
import type { DocumentPage } from "../src/extraction/types.js";
import {
  applyLearnedCorrections,
  emptyCorrectionMemory,
  learnCorrections,
  parseCorrectionMemory,
  upgradeLegacyFieldRules,
} from "../src/learning/correction-memory.js";
import { reviewDraftFromExtraction } from "../src/review/draft.js";

function template(reference: string, issueDate = "01.09.2026", correctedDate = "02.09.2026"): DocumentPage[] {
  const values = [
    ["Rechnung", 50, 40],
    ["USt-ID:", 50, 70], ["DE123456789", 180, 70],
    ["Rechnungsdatum:", 50, 100], [issueDate, 180, 100],
    ["Abrechnungsstichtag:", 50, 130], [correctedDate, 180, 130],
    ["Auftragskennung:", 50, 160], [reference, 180, 160],
  ] as const;
  return [{
    page: 1,
    width: 600,
    height: 800,
    tokens: values.map(([text, x, y], index) => ({
      id: `t${index}`,
      page: 1,
      text,
      box: { x, y, width: Math.max(35, text.length * 6), height: 12 },
      origin: "text-layer" as const,
    })),
  }];
}

function unrecognizedTable(description: string, quantity: string, price: string, total: string): DocumentPage[] {
  const values = [
    ["Rechnung", 50, 40, 60],
    ["USt-ID:", 50, 70, 55], ["DE123456789", 120, 70, 80],
    ["Tätigkeit", 80, 170, 120], ["Anzahl", 210, 170, 30], ["Satz", 250, 170, 35], ["Wert", 295, 170, 40],
    [description, 80, 195, 120], [quantity, 210, 195, 30], [price, 250, 195, 35], [total, 295, 195, 40],
    ["Endbetrag", 250, 240, 60], [total, 320, 240, 40],
  ] as const;
  return [{
    page: 1,
    width: 600,
    height: 800,
    tokens: values.map(([text, x, y, width], index) => ({
      id: `table-${index}`,
      page: 1,
      text,
      box: { x, y, width, height: 12 },
      origin: "text-layer" as const,
    })),
  }];
}

function footerTemplate(company: string, street: string, postalCity: string, vatId: string): DocumentPage[] {
  const pages = template("K-4711");
  pages[0]!.tokens.find((token) => token.text === "DE123456789")!.text = vatId;
  const rows = [
    ["Unternehmen", "Bankverbindung", "Steuerangaben"],
    [company, "IBAN: DE89 3704 0044 0532 0130 00", "Steuernr.: 12/345/67890"],
    [street, "BIC: COBADEFFXXX", `USt-IdNr.: ${vatId}`],
    [postalCity, "Bank: Musterbank", "Geschäftsführer: Beispiel"],
  ];
  rows.forEach((row, rowIndex) => row.forEach((text, column) => {
    pages[0]!.tokens.push({ id: `footer-${rowIndex}-${column}`, page: 1, text,
      box: { x: 50 + column * 185, y: 704 + rowIndex * 14, width: 160, height: 10 }, origin: "text-layer" });
  }));
  return pages;
}

async function learningPdf(reference: string): Promise<Uint8Array> {
  const document = await PDFDocument.create();
  const page = document.addPage([595, 842]);
  const font = await document.embedFont(StandardFonts.Helvetica);
  page.drawText("RECHNUNG", { x: 60, y: 790, size: 16, font });
  page.drawText("USt-ID: DE123456789", { x: 60, y: 750, size: 10, font });
  page.drawText(`Rechnungsnummer: RE-${reference}`, { x: 60, y: 710, size: 10, font });
  page.drawText(`Auftragskennung: ${reference}`, { x: 320, y: 710, size: 10, font });
  return document.save();
}

describe("local correction memory", () => {
  it("learns individual footer columns across changed issuers and persists the template", () => {
    const first = analyzeDocumentPages(footerTemplate("Erste Firma GmbH", "Hauptstraße 12", "10115 Berlin", "DE123456789"));
    const initial = reviewDraftFromExtraction(first);
    const corrected = { ...initial, seller: { ...initial.seller, name: "Erste Firma GmbH", addressLine1: "Hauptstraße 12", postalCode: "10115", city: "Berlin" } };
    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, corrected);
    expect(learned.learnedFields).toEqual(["sellerName", "sellerAddressLine1", "sellerPostalCode", "sellerCity"]);
    const memory = parseCorrectionMemory(JSON.stringify(learned.memory));
    expect(JSON.stringify(memory)).not.toContain("Berlin");
    expect(JSON.stringify(memory)).not.toContain("10115");
    const secondPages = footerTemplate("Zweite Firma GmbH", "Bauhofstraße 18", "50667 Köln", "DE000000001");
    // A shorter letterhead shifts the flowing body but not the fixed footer.
    for (const token of secondPages[0]!.tokens) {
      if (token.box.y >= 100 && token.box.y < 700) token.box.y -= 14;
    }
    const second = analyzeDocumentPages(secondPages);
    const applied = applyLearnedCorrections(second, memory);
    expect(reviewDraftFromExtraction(applied.extraction).seller).toMatchObject({ name: "Zweite Firma GmbH", addressLine1: "Bauhofstraße 18", postalCode: "50667", city: "Köln" });
    expect(applied.extraction.fields.sellerName?.sourceTokenIds).toEqual(["footer-1-0"]);
  });

  it("uses an explicit source even when the value repeats and has not changed", () => {
    const pages = template("K-4711");
    pages[0]!.tokens.push({ id: "chosen", page: 1, text: "K-4711", box: { x: 180, y: 190, width: 70, height: 12 }, origin: "text-layer" });
    const first = analyzeDocumentPages(pages);
    const initial = { ...reviewDraftFromExtraction(first), buyerReference: "K-4711" };
    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, initial, undefined, { buyerReference: ["chosen"] });
    expect(learned.learnedFields).toEqual(["buyerReference"]);
    const secondPages = structuredClone(pages);
    secondPages[0]!.tokens.find((token) => token.id === "chosen")!.text = "NEU-2026";
    const second = analyzeDocumentPages(secondPages);
    expect(applyLearnedCorrections(second, learned.memory).extraction.fields.buyerReference?.value).toBe("NEU-2026");
  });

  it("rejects a source assignment whose value is not in the chosen text", () => {
    const first = analyzeDocumentPages(template("K-4711"));
    const initial = reviewDraftFromExtraction(first);
    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, { ...initial, buyerReference: "K-4711" }, undefined, { buyerReference: ["t0"] });
    expect(learned.skippedFields).toEqual(["buyerReference"]);
    expect(learned.memory.rules).toEqual([]);
  });

  it("asks for an explicit source when an added value occurs in two equally plausible places", () => {
    const pages = template("K-4711");
    pages[0]!.tokens.push({ id: "duplicate", page: 1, text: "K-4711", box: { x: 180, y: 190, width: 70, height: 12 }, origin: "text-layer" });
    const first = analyzeDocumentPages(pages);
    const initial = reviewDraftFromExtraction(first);
    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, { ...initial, buyerReference: "K-4711" });
    expect(learned.skippedFields).toEqual(["buyerReference"]);
    expect(learned.memory.rules).toEqual([]);
  });

  it("reads old whole-row rules without copying neighboring footer columns", () => {
    const first = analyzeDocumentPages(footerTemplate("Erste Firma GmbH", "Hauptstraße 12", "10115 Berlin", "DE123456789"));
    const initial = reviewDraftFromExtraction(first);
    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, { ...initial, seller: { ...initial.seller, name: "Erste Firma GmbH" } });
    const rule = learned.memory.rules[0]!;
    delete rule.region;
    delete rule.templateKey;
    rule.box.width = 530 / 600;
    expect(applyLearnedCorrections(first, learned.memory).extraction.fields.sellerName?.value).toBe("Erste Firma GmbH");
    const upgraded = upgradeLegacyFieldRules(first, learned.memory);
    expect(upgraded.rules[0]?.region).toBe(true);
    expect(upgraded.rules[0]?.templateKey).toBeTruthy();
    const second = analyzeDocumentPages(footerTemplate("Zweite Firma GmbH", "Bauhofstraße 18", "50667 Köln", "DE000000001"));
    expect(applyLearnedCorrections(second, parseCorrectionMemory(JSON.stringify(upgraded))).extraction.fields.sellerName?.value).toBe("Zweite Firma GmbH");
    expect(upgradeLegacyFieldRules(second, upgraded)).toBe(upgraded);
  });

  it("learns the location of a missing value and reads the changed value on a later invoice", () => {
    const first = analyzeDocumentPages(template("K-4711"));
    expect(first.fields.buyerReference).toBeUndefined();
    const initial = reviewDraftFromExtraction(first);
    const corrected = { ...initial, buyerReference: "K-4711" };

    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, corrected, "2026-08-31T10:00:00.000Z");
    expect(learned.learnedFields).toEqual(["buyerReference"]);
    expect(JSON.stringify(learned.memory)).not.toContain("K-4711");

    const second = analyzeDocumentPages(template("K-5000"));
    const applied = applyLearnedCorrections(second, learned.memory);
    expect(applied.appliedFields).toEqual(["buyerReference"]);
    expect(applied.extraction.fields.buyerReference).toMatchObject({ value: "K-5000", confidence: 0.84 });
    expect(applied.extraction.fields.buyerReference?.transformations).toContainEqual(expect.objectContaining({ operation: "learned-layout" }));
  });

  it("learns across two real PDF extraction runs", async () => {
    const first = await extractInvoicePdf(await learningPdf("K-1001"));
    const initial = reviewDraftFromExtraction(first);
    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, { ...initial, buyerReference: "K-1001" });
    expect(learned.learnedFields).toEqual(["buyerReference"]);

    const second = await extractInvoicePdf(await learningPdf("K-1002"));
    const applied = applyLearnedCorrections(second, learned.memory);
    expect(applied.extraction.fields.buyerReference?.value).toBe("K-1002");
  });

  it("can replace a previously misclassified value with the learned source position", () => {
    const first = analyzeDocumentPages(template("K-4711", "01.09.2026", "02.09.2026"));
    const initial = reviewDraftFromExtraction(first);
    expect(initial.issueDate).toBe("2026-09-01");
    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, { ...initial, issueDate: "2026-09-02" });

    const second = analyzeDocumentPages(template("K-5000", "03.09.2026", "04.09.2026"));
    expect(second.fields.issueDate?.value).toBe("2026-09-03");
    const applied = applyLearnedCorrections(second, learned.memory);
    expect(applied.extraction.fields.issueDate?.value).toBe("2026-09-04");
  });

  it("does not learn values that are absent from the source document", () => {
    const extraction = analyzeDocumentPages(template("K-4711"));
    const initial = reviewDraftFromExtraction(extraction);
    const learned = learnCorrections(emptyCorrectionMemory(), extraction, initial, { ...initial, buyerReference: "NICHT-IM-DOKUMENT" });
    expect(learned.learnedFields).toEqual([]);
    expect(learned.skippedFields).toEqual(["buyerReference"]);
    expect(learned.memory.rules).toEqual([]);
  });

  it("learns an unrecognized item table without storing its concrete values", () => {
    const first = analyzeDocumentPages(unrecognizedTable("Montage", "2,00 Std.", "80,00", "160,00"));
    expect(first.lineItems).toEqual([]);
    const initial = reviewDraftFromExtraction(first);
    const corrected = {
      ...initial,
      lines: [{ id: "1", description: "Montage", quantity: "2", unitCode: "HUR" as const, netUnitPrice: "80", taxRate: "19", sourceTokenIds: [] }],
    };
    const learned = learnCorrections(emptyCorrectionMemory(), first, initial, corrected);
    expect(learned.tableLearned).toBe(true);
    expect(JSON.stringify(learned.memory)).not.toContain("Montage");

    const second = analyzeDocumentPages(unrecognizedTable("Wartung", "3,00 Std.", "90,00", "270,00"));
    const applied = applyLearnedCorrections(second, learned.memory);
    expect(applied.appliedTable).toBe(true);
    expect(applied.extraction.lineItems).toEqual([expect.objectContaining({
      description: "Wartung",
      quantity: "3.000",
      unit: "Std",
      netUnitPrice: "90.00",
      netAmount: "270.00",
      taxRate: "19.00",
    })]);
  });

  it("ignores malformed persisted data", () => {
    expect(parseCorrectionMemory("not json")).toEqual(emptyCorrectionMemory());
    expect(parseCorrectionMemory('{"schemaVersion":2,"rules":[]}')).toEqual(emptyCorrectionMemory());
  });
});
