import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { PDFDocument, StandardFonts, type PDFFont, type PDFPage } from "pdf-lib";

const outputDirectory = resolve("artifacts/learning-smoke");
await mkdir(outputDirectory, { recursive: true });

async function createInvoice(invoiceNumber: string, orderReference: string, issueDate: string): Promise<Uint8Array> {
  const document = await PDFDocument.create();
  const page = document.addPage([595, 842]);
  const font = await document.embedFont(StandardFonts.Helvetica);
  const draw = (text: string, x: number, y: number, size = 10) => drawText(page, font, text, x, y, size);

  draw("RECHNUNG", 60, 800, 16);
  draw("Lernbeispiel Handwerk GmbH", 69, 765);
  draw("Werkstrasse 12", 69, 750);
  draw("10115 Berlin", 69, 735);
  draw("USt-ID: DE987654321", 69, 720);
  draw("Rechnung an:", 69, 690);
  draw("Musterkunde AG", 69, 675);
  draw("Kundenweg 5", 69, 660);
  draw("20095 Hamburg", 69, 645);
  draw(`Rechnungsnummer: ${invoiceNumber}`, 69, 615);
  draw(`Auftragskennung: ${orderReference}`, 320, 615);
  draw(`Rechnungsdatum: ${issueDate}`, 69, 600);
  draw("Zahlbar bis: 15.09.2026", 69, 585);
  draw("Beschreibung", 105, 525);
  draw("Menge", 279, 525);
  draw("Einzelpreis", 345, 525);
  draw("Gesamt", 435, 525);
  draw("Wartung", 105, 510);
  draw("2 Std.", 279, 510);
  draw("80,00 EUR", 345, 510);
  draw("160,00 EUR", 435, 510);
  draw("Netto: 160,00 EUR", 340, 470);
  draw("USt. 19 %: 30,40 EUR", 340, 455);
  draw("Gesamt: 190,40 EUR", 340, 440);
  return document.save();
}

function drawText(page: PDFPage, font: PDFFont, text: string, x: number, y: number, size: number) {
  page.drawText(text, { x, y, size, font });
}

const first = resolve(outputDirectory, "lernrechnung-a.pdf");
const second = resolve(outputDirectory, "lernrechnung-b.pdf");
await writeFile(first, await createInvoice("LERN-1001", "AUF-1001", "31.08.2026"));
await writeFile(second, await createInvoice("LERN-1002", "AUF-1002", "01.09.2026"));
console.log(first);
console.log(second);
