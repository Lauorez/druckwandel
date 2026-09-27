import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { PDFDocument, StandardFonts, rgb, type PDFFont, type PDFPage } from "pdf-lib";

const outputDirectory = resolve("test/corpus/pdf");
await mkdir(outputDirectory, { recursive: true });

type Draw = (text: string, x: number, y: number, size?: number) => void;

async function invoiceDocument(drawInvoice: (draw: Draw, page: PDFPage) => void): Promise<Uint8Array> {
  const document = await PDFDocument.create();
  const page = document.addPage([595, 842]);
  const font: PDFFont = await document.embedFont(StandardFonts.Helvetica);
  const draw: Draw = (text, x, y, size = 10) => page.drawText(text, { x, y, size, font });
  drawInvoice(draw, page);
  return document.save();
}

async function multiPageInvoice(drawPages: (addPage: () => Draw) => void): Promise<Uint8Array> {
  const document = await PDFDocument.create();
  const font: PDFFont = await document.embedFont(StandardFonts.Helvetica);
  const addPage = (): Draw => {
    const page = document.addPage([595, 842]);
    return (text, x, y, size = 10) => page.drawText(text, { x, y, size, font });
  };
  drawPages(addPage);
  return document.save();
}

async function save(name: string, bytes: Uint8Array) {
  const path = resolve(outputDirectory, name);
  await writeFile(path, bytes);
  console.log(path);
}

function drawParties(draw: Draw) {
  draw("Anbieter GmbH", 69, 765);
  draw("Beispielstrasse 12", 69, 750);
  draw("10115 Berlin", 69, 735);
  draw("USt-IdNr.: DE123456789", 69, 720);
  draw("Rechnung an:", 69, 690);
  draw("Kunde AG", 69, 675);
  draw("Kundenweg 5", 69, 660);
  draw("20095 Hamburg", 69, 645);
}

function drawMetadata(draw: Draw, invoiceNumber: string) {
  draw(`Rechnungsnummer: ${invoiceNumber}`, 69, 615);
  draw("Rechnungsdatum: 30.08.2026", 69, 600);
  draw("Leistungsdatum: 30.08.2026", 69, 585);
  draw("Zahlbar bis: 13.09.2026", 69, 570);
  draw("Leitweg-ID: 04011000-12345-03", 300, 615);
}

function drawTableHeader(draw: Draw, y: number) {
  draw("Pos.", 69, y);
  draw("Beschreibung", 105, y);
  draw("Menge", 279, y);
  draw("Einzelpreis", 327, y);
  draw("Gesamt", 423, y);
}

await save("standard-header-merged.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 57, 800, 16);
  drawParties(draw);
  drawMetadata(draw, "RE-2026-001");
  drawTableHeader(draw, 525);
  draw("1", 69, 510); draw("Softwareentwicklung", 105, 510); draw("10 Std.", 279, 510); draw("95,00 EUR", 357, 510); draw("950,00 EUR", 429, 510);
  draw("2", 69, 495); draw("Einrichtung und Konfiguration 1 Stk.", 105, 495); draw("150,00 EUR", 351, 495); draw("150,00 EUR", 429, 495);
  draw("Netto:", 339, 455); draw("1.100,00 EUR", 417, 455);
  draw("USt. 19 %:", 339, 440); draw("209,00 EUR", 429, 440);
  draw("Gesamt:", 339, 425); draw("1.309,00 EUR", 417, 425);
  draw("Zahlungsbedingungen:", 69, 385);
  draw("Bitte innerhalb von 14 Tagen ohne Abzug ueberweisen.", 69, 370);
  draw("IBAN: DE89 3704 0044 0532 0130 00", 69, 340);
  draw("BIC: COBADEFFXXX", 69, 325);
}));

await save("legacy-columns.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 66, 760, 16);
  draw("Beleg-Nr:", 450, 700); draw("1367098", 526, 700);
  draw("Beleg-Datum: 03.08.2026", 450, 680);
  draw("31.07.26 mtl. Grundgebuehr", 66, 560); draw("1,000 Stk", 336, 560); draw("10,00", 418, 560); draw("10,00", 490, 560); draw("19,0", 540, 560);
  draw("31.08.26 mtl. Grundgebuehr", 66, 545); draw("1,000 Stk", 336, 545); draw("10,00", 418, 545); draw("10,00", 490, 545); draw("19,0", 540, 545);
  draw("30.09.26 mtl. Grundgebuehr", 66, 530); draw("1,000 Stk", 336, 530); draw("10,00", 418, 530); draw("10,00", 490, 530); draw("19,0", 540, 530);
  draw("Umsatzsteuer 19,00 %", 66, 470); draw("30,00", 298, 470); draw("19,0", 354, 470); draw("5,70", 422, 470); draw("35,70", 485, 470); draw("EUR", 534, 470);
  draw("USt.-IdNr.:DE 812 968 520", 66, 70);
}));

await save("credit-note.pdf", await invoiceDocument((draw) => {
  draw("GUTSCHRIFT", 69, 790, 17);
  draw("Rechnungsnummer: GS-2026-003", 69, 740);
  draw("Rechnungsdatum: 30.08.2026", 69, 725);
  draw("Ursprungsrechnung: RE-2026-001", 69, 710);
  draw("Gesamt: 119,00 EUR", 350, 650);
}));

await save("reverse-charge.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 69, 790, 17);
  drawParties(draw);
  drawMetadata(draw, "RE-RC-001");
  drawTableHeader(draw, 525);
  draw("1", 69, 510); draw("Beratungsleistung", 105, 510); draw("2 Std.", 279, 510); draw("100,00 EUR", 351, 510); draw("200,00 EUR", 423, 510);
  draw("Netto: 200,00 EUR", 339, 470);
  draw("USt. 0 %: 0,00 EUR", 339, 455);
  draw("Gesamt: 200,00 EUR", 339, 440);
  draw("Steuerschuldnerschaft des Leistungsempfängers gemäß § 13b UStG", 69, 400);
}));

await save("tax-exempt.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 69, 790, 17);
  drawParties(draw);
  drawMetadata(draw, "RE-STFR-001");
  drawTableHeader(draw, 525);
  draw("1", 69, 510); draw("Unterrichtsleistung", 105, 510); draw("1 Stk.", 279, 510); draw("150,00 EUR", 351, 510); draw("150,00 EUR", 423, 510);
  draw("Netto: 150,00 EUR", 339, 470);
  draw("USt. 0 %: 0,00 EUR", 339, 455);
  draw("Gesamt: 150,00 EUR", 339, 440);
  draw("steuerfrei gemäß § 4 UStG", 69, 400);
}));

await save("invoice-discount.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 69, 790, 17);
  drawParties(draw);
  drawMetadata(draw, "RE-RABATT-001");
  drawTableHeader(draw, 525);
  draw("1", 69, 510); draw("Wartung", 105, 510); draw("1 Stk.", 279, 510); draw("100,00 EUR", 351, 510); draw("100,00 EUR", 423, 510);
  draw("Rabatt 10 %: 10,00 EUR", 339, 480);
  draw("Netto: 90,00 EUR", 339, 465);
  draw("USt. 19 %: 17,10 EUR", 339, 450);
  draw("Gesamt: 107,10 EUR", 339, 435);
}));

await save("scan-without-text.pdf", await invoiceDocument((_draw, page) => {
  page.drawRectangle({ x: 60, y: 120, width: 475, height: 620, color: rgb(0.92, 0.92, 0.92) });
  page.drawRectangle({ x: 90, y: 650, width: 220, height: 25, color: rgb(0.55, 0.55, 0.55) });
  page.drawRectangle({ x: 90, y: 590, width: 390, height: 8, color: rgb(0.72, 0.72, 0.72) });
  page.drawRectangle({ x: 90, y: 560, width: 350, height: 8, color: rgb(0.72, 0.72, 0.72) });
}));

await save("two-page-totals.pdf", await multiPageInvoice((addPage) => {
  const first = addPage();
  first("RECHNUNG", 57, 800, 16);
  drawParties(first);
  drawMetadata(first, "RE-2026-002");
  drawTableHeader(first, 525);
  first("1", 69, 510); first("Softwareentwicklung", 105, 510); first("10 Std.", 279, 510); first("95,00 EUR", 357, 510); first("950,00 EUR", 429, 510);
  first("2", 69, 495); first("Einrichtung und Konfiguration 1 Stk.", 105, 495); first("150,00 EUR", 351, 495); first("150,00 EUR", 429, 495);
  const second = addPage();
  second("Seite 2", 69, 800);
  second("Netto:", 339, 760); second("1.100,00 EUR", 417, 760);
  second("USt. 19 %:", 339, 745); second("209,00 EUR", 429, 745);
  second("Gesamt:", 339, 730); second("1.309,00 EUR", 417, 730);
  second("Zahlungsbedingungen:", 69, 680);
  second("Bitte innerhalb von 14 Tagen ohne Abzug ueberweisen.", 69, 665);
  second("IBAN: DE89 3704 0044 0532 0130 00", 69, 635);
  second("BIC: COBADEFFXXX", 69, 620);
}));

await save("similar-parties.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 57, 800, 16);
  draw("Mueller Technik GmbH", 69, 765);
  draw("Werkweg 8", 69, 750);
  draw("50667 Koeln", 69, 735);
  draw("USt-IdNr.: DE123456789", 69, 720);
  draw("Rechnung an:", 69, 690);
  draw("Mueller Handel GmbH", 69, 675);
  draw("Werkstrasse 8", 69, 660);
  draw("50667 Koeln", 69, 645);
  draw("Rechnungsnummer: RE-AEHN-001", 69, 615);
  draw("Rechnungsdatum: 30.08.2026", 69, 600);
  draw("Leistungsdatum: 30.08.2026", 69, 585);
  draw("Zahlbar bis: 13.09.2026", 69, 570);
  draw("Leitweg-ID: 04011000-12345-03", 300, 615);
  drawTableHeader(draw, 525);
  draw("1", 69, 510); draw("Wartung", 105, 510); draw("1 Stk.", 279, 510); draw("100,00 EUR", 351, 510); draw("100,00 EUR", 423, 510);
  draw("Netto:", 339, 455); draw("100,00 EUR", 417, 455);
  draw("USt. 19 %:", 339, 440); draw("19,00 EUR", 429, 440);
  draw("Gesamt:", 339, 425); draw("119,00 EUR", 417, 425);
  draw("IBAN: DE89 3704 0044 0532 0130 00", 69, 340);
  draw("BIC: COBADEFFXXX", 69, 325);
}));

await save("repeated-values.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 57, 800, 16);
  drawParties(draw);
  draw("Rechnungsnummer: RE-2026-001", 69, 615);
  draw("Rechnungsdatum: 30.08.2026", 69, 600);
  draw("Leistungsdatum: 30.08.2026", 69, 585);
  draw("Zahlbar bis: 13.09.2026", 69, 570);
  draw("Leitweg-ID: 04011000-54321-03", 300, 615);
  draw("Kunden-Nr.: RE-2026-001", 300, 600);
  drawTableHeader(draw, 525);
  draw("1", 69, 510); draw("Softwareentwicklung", 105, 510); draw("10 Std.", 279, 510); draw("95,00 EUR", 357, 510); draw("950,00 EUR", 429, 510);
  draw("2", 69, 495); draw("Einrichtung und Konfiguration 1 Stk.", 105, 495); draw("150,00 EUR", 351, 495); draw("150,00 EUR", 429, 495);
  draw("Netto:", 339, 455); draw("1.100,00 EUR", 417, 455);
  draw("USt. 19 %:", 339, 440); draw("209,00 EUR", 429, 440);
  draw("Gesamt:", 339, 425); draw("1.309,00 EUR", 417, 425);
  draw("Zahlungsbedingungen:", 69, 385);
  draw("Bitte innerhalb von 14 Tagen ohne Abzug ueberweisen.", 69, 370);
  draw("IBAN: DE89 3704 0044 0532 0130 00", 69, 340);
  draw("BIC: COBADEFFXXX", 69, 325);
  draw("Rechnungsnummer: RE-2026-001", 69, 40);
}));

await save("handwerk-right-meta.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 66, 800, 16);
  draw("Handwerk Nord GmbH", 69, 765);
  draw("Hafenallee 21", 69, 750);
  draw("20095 Hamburg", 69, 735);
  draw("USt-IdNr.: DE123456789", 69, 720);
  draw("Rechnung an:", 69, 690);
  draw("Kunde AG", 69, 675);
  draw("Kundenweg 5", 69, 660);
  draw("20095 Hamburg", 69, 645);
  draw("Beleg-Nr: HW-2026-014", 330, 615);
  draw("Leitweg-ID: 04011000-12345-03", 330, 600);
  draw("Belegdatum: 30.08.2026", 330, 585);
  drawTableHeader(draw, 525);
  draw("1", 69, 510); draw("Geraetepruefung", 105, 510); draw("2 Std.", 279, 510); draw("80,00 EUR", 351, 510); draw("160,00 EUR", 423, 510);
  draw("Netto:", 339, 455); draw("160,00 EUR", 417, 455);
  draw("USt. 19 %:", 339, 440); draw("30,40 EUR", 429, 440);
  draw("Gesamt:", 339, 425); draw("190,40 EUR", 417, 425);
  draw("IBAN: DE89 3704 0044 0532 0130 00", 69, 340);
  draw("BIC: COBADEFFXXX", 69, 325);
}));

await save("changing-footer.pdf", await invoiceDocument((draw) => {
  draw("RECHNUNG", 57, 800, 16);
  drawParties(draw);
  drawMetadata(draw, "RE-2026-003");
  drawTableHeader(draw, 525);
  draw("1", 69, 510); draw("Softwareentwicklung", 105, 510); draw("10 Std.", 279, 510); draw("95,00 EUR", 357, 510); draw("950,00 EUR", 429, 510);
  draw("2", 69, 495); draw("Einrichtung und Konfiguration 1 Stk.", 105, 495); draw("150,00 EUR", 351, 495); draw("150,00 EUR", 429, 495);
  draw("Netto:", 339, 455); draw("1.100,00 EUR", 417, 455);
  draw("USt. 19 %:", 339, 440); draw("209,00 EUR", 429, 440);
  draw("Gesamt:", 339, 425); draw("1.309,00 EUR", 417, 425);
  draw("Unternehmen", 69, 200);
  draw("Anbieter GmbH", 69, 185);
  draw("Beispielstrasse 12", 69, 170);
  draw("10115 Berlin", 69, 155);
  draw("Bankverbindung", 230, 200);
  draw("IBAN: DE89 3704 0044 0532 0130 00", 230, 185);
  draw("BIC: COBADEFFXXX", 230, 170);
  draw("Bank: Musterbank", 230, 155);
  draw("Steuerangaben", 420, 200);
  draw("USt-IdNr.: DE123456789", 420, 185);
  draw("Steuernr.: 12/345/67890", 420, 170);
}));
