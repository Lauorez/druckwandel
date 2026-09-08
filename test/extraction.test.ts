import { PDFDocument, StandardFonts } from "pdf-lib";
import { describe, expect, it } from "vitest";
import { extractInvoicePdf } from "../src/extraction/index.js";
import { reconstructLines, reconstructTableRows } from "../src/extraction/layout.js";
import type { DocumentPage, OcrAdapter } from "../src/extraction/types.js";
import { parseTesseractTsv } from "../src/extraction/ocr-tesseract.js";

async function invoicePdf(): Promise<Uint8Array> {
  const document = await PDFDocument.create();
  const page = document.addPage([595, 842]);
  const font = await document.embedFont(StandardFonts.Helvetica);
  const rows: Array<[string, string]> = [
    ["Rechnungsnummer:", "RE-2026-0042"],
    ["Rechnungsdatum:", "20.08.2026"],
    ["Leistungsdatum:", "19.08.2026"],
    ["Faellig am:", "03.09.2026"],
    ["USt-IdNr.:", "DE123456789"],
    ["IBAN:", "DE02 1203 0000 0000 2020 51    BIC: BYLADEM1001"],
    ["Summe Netto:", "1.234,50 EUR"],
    ["MwSt. 19%:", "234,56 EUR"],
    ["Zahlbetrag:", "1.469,06 EUR"],
  ];
  rows.forEach(([label, value], index) => {
    const y = 790 - index * 30;
    page.drawText(label, { x: 50, y, size: 11, font });
    page.drawText(value, { x: 250, y, size: 11, font });
  });
  return document.save();
}

async function multiColumnInvoicePdf(): Promise<Uint8Array> {
  const document = await PDFDocument.create();
  const page = document.addPage([595, 842]);
  const font = await document.embedFont(StandardFonts.Helvetica);
  const draw = (text: string, x: number, y: number) => page.drawText(text, { x, y, size: 10, font });
  draw("Beleg-Nr:", 450, 700); draw("1367098", 526, 700);
  draw("Beleg-Datum: 03.08.2026", 450, 680);
  draw("31.07.26 mtl. Grundgebuehr", 66, 560); draw("1,000 Stk", 336, 560); draw("10,00", 418, 560); draw("10,00", 490, 560); draw("19,0", 540, 560);
  draw("31.08.26 mtl. Grundgebuehr", 66, 545); draw("1,000 Stk", 336, 545); draw("10,00", 418, 545); draw("10,00", 490, 545); draw("19,0", 540, 545);
  draw("30.09.26 mtl. Grundgebuehr", 66, 530); draw("1,000 Stk", 336, 530); draw("10,00", 418, 530); draw("10,00", 490, 530); draw("19,0", 540, 530);
  draw("Umsatzsteuer 19,00 %", 66, 470); draw("30,00", 298, 470); draw("19,0", 354, 470); draw("5,70", 422, 470); draw("35,70", 485, 470); draw("EUR", 534, 470);
  draw("USt.-IdNr.:DE 812 968 520", 66, 70);
  return document.save();
}

async function headerGuidedInvoicePdf(): Promise<Uint8Array> {
  const document = await PDFDocument.create();
  const page = document.addPage([595, 842]);
  const font = await document.embedFont(StandardFonts.Helvetica);
  const draw = (text: string, x: number, y: number) => page.drawText(text, { x, y, size: 10, font });
  draw("RECHNUNG", 60, 790);
  draw("Anbieter GmbH", 70, 760); draw("Beispielstrasse 12", 70, 745); draw("10115 Berlin", 70, 730);
  draw("Rechnung an:", 70, 700); draw("Kunde AG", 70, 685); draw("Kundenweg 5", 70, 670); draw("20095 Hamburg", 70, 655);
  draw("Pos.", 70, 600); draw("Beschreibung", 105, 600); draw("Menge", 280, 600); draw("Einzelpreis", 327, 600); draw("Gesamt", 423, 600);
  draw("1", 70, 585); draw("Softwareentwicklung", 105, 585); draw("10 Std.", 280, 585); draw("95,00 EUR", 357, 585); draw("950,00 EUR", 429, 585);
  draw("2", 70, 570); draw("Einrichtung und Konfiguration 1 Stk.", 105, 570); draw("150,00 EUR", 351, 570); draw("150,00 EUR", 429, 570);
  draw("Netto: 1.100,00 EUR", 339, 530); draw("USt. 19 %: 209,00 EUR", 339, 515); draw("Gesamt: 1.309,00 EUR", 339, 500);
  return document.save();
}

describe("PDF extraction", () => {
  it("extracts positioned tokens, lines, normalized values and provenance", async () => {
    const result = await extractInvoicePdf(await invoicePdf());
    expect(result.usedOcr).toBe(false);
    expect(result.pages[0]?.tokens.length).toBeGreaterThan(10);
    expect(result.fields.invoiceNumber?.value).toBe("RE-2026-0042");
    expect(result.fields.issueDate?.value).toBe("2026-08-20");
    expect(result.fields.serviceDate?.value).toBe("2026-08-19");
    expect(result.fields.sellerVatId?.value).toBe("DE123456789");
    expect(result.fields.iban?.value).toBe("DE02120300000000202051");
    expect(result.fields.bic?.value).toBe("BYLADEM1001");
    expect(result.fields.lineNet?.value).toBe("1234.50");
    expect(result.fields.taxTotal?.value).toBe("234.56");
    expect(result.fields.payable?.value).toBe("1469.06");
    expect(result.fields.payable?.sourceTokenIds.length).toBeGreaterThan(0);
    expect(result.fields.payable?.confidence).toBeGreaterThanOrEqual(0.9);
    expect(result.fields.dueDate?.value).toBe("2026-09-03");
  });

  it("accepts a Node.js Buffer as returned by readFile", async () => {
    const pdf = Buffer.from(await invoicePdf());
    const result = await extractInvoicePdf(pdf);
    expect(result.fields.invoiceNumber?.value).toBe("RE-2026-0042");
  });

  it("maps multi-column German tax summaries and invoice lines by position", async () => {
    const result = await extractInvoicePdf(await multiColumnInvoicePdf());
    expect(result.fields.invoiceNumber?.value).toBe("1367098");
    expect(result.fields.sellerVatId?.value).toBe("DE812968520");
    expect(result.fields.lineNet?.value).toBe("30.00");
    expect(result.fields.taxTotal?.value).toBe("5.70");
    expect(result.fields.taxInclusive?.value).toBe("35.70");
    expect(result.fields.payable?.value).toBe("35.70");
    expect(result.lineItems).toHaveLength(3);
    expect(result.lineItems[0]).toMatchObject({ description: "mtl. Grundgebuehr", serviceDate: "2026-07-31", quantity: "1.000", unit: "Stk", netUnitPrice: "10.00", netAmount: "10.00", taxRate: "19.00" });
    expect(result.warnings).toEqual([]);
  });

  it("uses table headers when PDF tokens merge description and quantity", async () => {
    const result = await extractInvoicePdf(await headerGuidedInvoicePdf());
    expect(result.fields.sellerName?.value).toBe("Anbieter GmbH");
    expect(result.fields.buyerName?.value).toBe("Kunde AG");
    expect(result.fields.lineNet?.value).toBe("1100.00");
    expect(result.fields.taxTotal?.value).toBe("209.00");
    expect(result.fields.payable?.value).toBe("1309.00");
    expect(result.lineItems).toHaveLength(2);
    expect(result.lineItems[0]).toMatchObject({ description: "Softwareentwicklung", quantity: "10.000", unit: "Std", netUnitPrice: "95.00", netAmount: "950.00", taxRate: "19.00" });
    expect(result.lineItems[1]).toMatchObject({ description: "Einrichtung und Konfiguration", quantity: "1.000", unit: "Stk", netUnitPrice: "150.00", netAmount: "150.00", taxRate: "19.00" });
    expect(result.warnings).toEqual([]);
  });

  it("reads German month names, relative due dates and labelled addresses", async () => {
    const document = await PDFDocument.create();
    const page = document.addPage([595, 842]);
    const font = await document.embedFont(StandardFonts.Helvetica);
    const draw = (text: string, x: number, y: number) => page.drawText(text, { x, y, size: 10, font });
    draw("Absender GmbH", 70, 780);
    draw("z.Hd. Buchhaltung", 70, 765);
    draw("Beispielstrasse 12", 70, 750);
    draw("10115 Berlin", 70, 735);
    draw("Empfaenger:", 70, 700);
    draw("Kunde AG", 70, 685);
    draw("Kundenweg 5", 70, 670);
    draw("20095 Hamburg", 70, 655);
    draw("Rechnungsdatum: 8. September 2026", 70, 620);
    draw("Faellig in 14 Tagen", 70, 605);
    const result = await extractInvoicePdf(await document.save());
    expect(result.fields.sellerName?.value).toBe("Absender GmbH");
    expect(result.fields.sellerAddressLine1?.value).toBe("Beispielstrasse 12");
    expect(result.fields.buyerName?.value).toBe("Kunde AG");
    expect(result.fields.issueDate?.value).toBe("2026-09-08");
    expect(result.fields.dueDate?.value).toBe("2026-09-22");
  });

  it("reconstructs columns from horizontal gaps", () => {
    const pages: DocumentPage[] = [{ page: 1, width: 500, height: 500, tokens: [
      { id: "a", page: 1, text: "2", box: { x: 10, y: 10, width: 5, height: 10 }, origin: "text-layer" },
      { id: "b", page: 1, text: "Stunden", box: { x: 50, y: 10, width: 45, height: 10 }, origin: "text-layer" },
      { id: "c", page: 1, text: "160,00", box: { x: 200, y: 10, width: 40, height: 10 }, origin: "text-layer" },
    ] }];
    const lines = reconstructLines(pages);
    expect(reconstructTableRows(lines, pages).at(0)?.cells.map((cell) => cell.text)).toEqual(["2", "Stunden", "160,00"]);
  });

  it("uses a local OCR adapter only when the text layer is insufficient", async () => {
    const empty = await PDFDocument.create();
    empty.addPage();
    const ocr: OcrAdapter = {
      name: "mock-local-ocr",
      async recognize() {
        return [{ page: 1, width: 100, height: 100, tokens: [{ id: "ocr-1", page: 1, text: "Rechnungsnummer: OCR-123", box: { x: 1, y: 1, width: 80, height: 10 }, origin: "ocr" }] }];
      },
    };
    const result = await extractInvoicePdf(await empty.save(), { ocr });
    expect(result.usedOcr).toBe(true);
    expect(result.fields.invoiceNumber?.value).toBe("OCR-123");
    expect(result.warnings[0]?.code).toBe("OCR_USED");
  });

  it("maps Tesseract TSV words back to PDF coordinates and filters weak guesses", () => {
    const tsv = [
      "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext",
      "5\t1\t1\t1\t1\t1\t100\t200\t80\t20\t95.5\tRechnung",
      "5\t1\t1\t1\t1\t2\t200\t200\t60\t20\t12.0\tRauschen",
    ].join("\n");
    const tokens = parseTesseractTsv(tsv, 1, 2, 35);
    expect(tokens).toEqual([{ id: "p1-ocr0", page: 1, text: "Rechnung", box: { x: 50, y: 100, width: 40, height: 10 }, origin: "ocr" }]);
  });
});
