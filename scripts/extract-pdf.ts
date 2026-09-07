import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { extractInvoicePdf } from "../src/extraction/index.js";
import { TesseractCliOcrAdapter } from "../src/extraction/ocr-tesseract.js";

const args = process.argv.slice(2);
const useOcr = args.includes("--ocr");
const input = args.find((argument) => argument !== "--ocr");
if (!input) {
  console.error("Usage: npm run extract:pdf -- path/to/invoice.pdf [--ocr]");
  process.exit(2);
}
const result = await extractInvoicePdf(await readFile(resolve(input)), useOcr ? { ocr: new TesseractCliOcrAdapter() } : {});
console.log(JSON.stringify({ fields: result.fields, lineItems: result.lineItems, warnings: result.warnings, pages: result.pages.length, usedOcr: result.usedOcr }, null, 2));
