import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { resolve } from "node:path";
import fontkit from "@pdf-lib/fontkit";
import { PDFDocument } from "pdf-lib";
import { convertToPdfA3 } from "../src/engine/hybrid-pdf.js";
import { extractInvoicePdf } from "../src/extraction/index.js";
import { invoiceInputFromReview, reviewDraftFromExtraction, validateReviewDraft } from "../src/review/draft.js";

const outputDirectory = resolve("artifacts/demo");
const fontCandidates = [
  "/System/Library/Fonts/Supplemental/Arial.ttf",
  "/Library/Fonts/Arial Unicode.ttf",
  "C:\\Windows\\Fonts\\arial.ttf",
  `${homedir()}/Library/Fonts/Arial.ttf`,
];

async function embeddedFontBytes(): Promise<Uint8Array> {
  for (const candidate of fontCandidates) {
    try {
      return new Uint8Array(await readFile(candidate));
    } catch {
      continue;
    }
  }
  throw new Error("Keine einbettbare TrueType-Schrift gefunden (Arial). Für die Demo-PDF wird eine eingebettete Schrift benötigt.");
}

const document = await PDFDocument.create();
document.registerFontkit(fontkit);
const page = document.addPage([595, 842]);
const font = await document.embedFont(await embeddedFontBytes(), { subset: true });
const draw = (text: string, x: number, y: number, size = 10) => page.drawText(text, { x, y, size, font });

draw("RECHNUNG", 57, 800, 16);
draw("Muster Elektro GmbH", 69, 765);
draw("Werkstraße 1", 69, 750);
draw("10115 Berlin", 69, 735);
draw("USt-IdNr.: DE123456789", 69, 720);
draw("Rechnung an:", 69, 690);
draw("Beispiel Bau AG", 69, 675);
draw("Kundenweg 9", 69, 660);
draw("20095 Hamburg", 69, 645);
draw("Rechnungsnummer: RE-2026-0001", 69, 615);
draw("Rechnungsdatum: 20.08.2026", 69, 600);
draw("Leistungsdatum: 19.08.2026", 69, 585);
draw("Zahlbar bis: 03.09.2026", 69, 570);
draw("Leitweg-ID: 04011000-12345-03", 300, 615);
draw("Ansprechpartner: Erika Muster", 300, 600);
draw("Telefon: +49 30 123456", 300, 585);
draw("E-Mail: rechnung@muster.invalid", 300, 570);
draw("Pos.", 69, 525);
draw("Beschreibung", 105, 525);
draw("Menge", 279, 525);
draw("Einzelpreis", 327, 525);
draw("Gesamt", 423, 525);
draw("1", 69, 510); draw("Montagestunde", 105, 510); draw("2,5 Std.", 279, 510); draw("80,00 EUR", 357, 510); draw("200,00 EUR", 429, 510);
draw("2", 69, 495); draw("Kabel", 105, 495); draw("3 m", 279, 495); draw("12,35 EUR", 351, 495); draw("37,04 EUR", 429, 495);
draw("Netto:", 339, 455); draw("237,04 EUR", 417, 455);
draw("USt. 19 %:", 339, 440); draw("45,04 EUR", 429, 440);
draw("Gesamt:", 339, 425); draw("282,08 EUR", 417, 425);
draw("Zahlungsbedingungen:", 69, 385);
draw("Zahlbar ohne Abzug innerhalb von 14 Tagen.", 69, 370);
draw("IBAN: DE02 1203 0000 0000 2020 51", 69, 340);
draw("BIC: BYLADEM1001", 69, 325);

const pdf = await document.save();
await mkdir(outputDirectory, { recursive: true });
const pdfPath = resolve(outputDirectory, "muster-rechnung.pdf");
await writeFile(pdfPath, pdf);

const extraction = await extractInvoicePdf(pdf);
const draft = reviewDraftFromExtraction(extraction);
const xrechnung = validateReviewDraft(draft);
const zugferd = validateReviewDraft(draft, [], "zugferd");
await convertToPdfA3(pdf);

console.log(pdfPath);
console.log(`Felder: ${Object.keys(extraction.fields).sort().join(", ")}`);
console.log(`Positionen: ${extraction.lineItems.length}`);
console.log(`XRechnung bereit: ${xrechnung.valid}; PDF-Rechnung bereit: ${zugferd.valid}`);
if (!xrechnung.valid) {
  console.log(xrechnung.issues.map((issue) => `${issue.code} ${issue.path}: ${issue.message}`).join("\n"));
}
invoiceInputFromReview(draft);
