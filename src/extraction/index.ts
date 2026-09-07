import { analyzeDocumentPages } from "./analyze.js";
import { extractPdfText } from "./pdf-text.js";
import type { ExtractionResult, OcrAdapter } from "./types.js";

export interface ExtractionOptions {
  ocr?: OcrAdapter;
  minimumTextCharacters?: number;
}

export async function extractInvoicePdf(pdf: Uint8Array, options: ExtractionOptions = {}): Promise<ExtractionResult> {
  let pages = await extractPdfText(pdf);
  const warnings: ExtractionResult["warnings"] = [];
  const characterCount = pages.reduce((count, page) => count + page.tokens.reduce((sum, token) => sum + token.text.length, 0), 0);
  let usedOcr = false;
  if (characterCount < (options.minimumTextCharacters ?? 20)) {
    if (options.ocr) {
      pages = await options.ocr.recognize(pdf);
      usedOcr = true;
      warnings.push({ code: "OCR_USED", message: "Der Text wurde aus dem Bild der Rechnung gelesen. Bitte prüfen Sie die übernommenen Angaben besonders sorgfältig." });
    } else {
      warnings.push({ code: "OCR_REQUIRED", message: "Die Rechnung besteht aus einem Bild. Der Text konnte nicht automatisch gelesen werden." });
    }
  }
  return analyzeDocumentPages(pages, warnings, usedOcr);
}

export * from "./types.js";
export * from "./pdf-text.js";
export * from "./layout.js";
export * from "./analyze.js";
export * from "./classify.js";
export * from "./invoice-table.js";
export * from "./ocr-tesseract.js";
