import type { TextItem } from "pdfjs-dist/types/src/display/api.js";
import { analyzeDocumentPages } from "./analyze.js";
import { loadBrowserPdfJs } from "./browser-pdf.js";
import type { DocumentPage, ExtractionResult, SourceToken } from "./types.js";

function isTextItem(value: unknown): value is TextItem {
  return typeof value === "object" && value !== null && "str" in value && "transform" in value;
}

export async function extractInvoicePdfInBrowser(pdf: Uint8Array): Promise<ExtractionResult> {
  const { getDocument } = await loadBrowserPdfJs();
  const task = getDocument({ data: Uint8Array.from(pdf) });
  const document = await task.promise;
  const pages: DocumentPage[] = [];
  try {
    for (let pageNumber = 1; pageNumber <= document.numPages; pageNumber += 1) {
      const page = await document.getPage(pageNumber);
      const viewport = page.getViewport({ scale: 1 });
      const content = await page.getTextContent({ disableNormalization: false });
      const tokens: SourceToken[] = content.items.filter(isTextItem).flatMap((item, index) => {
        const text = item.str.trim();
        if (!text) return [];
        const x = item.transform[4] ?? 0;
        const baseline = item.transform[5] ?? 0;
        const height = Math.abs(item.height || item.transform[3] || 0);
        return [{ id: `p${pageNumber}-t${index}`, page: pageNumber, text, box: { x, y: viewport.height - baseline - height, width: Math.abs(item.width), height }, origin: "text-layer" as const }];
      });
      pages.push({ page: pageNumber, width: viewport.width, height: viewport.height, tokens });
      page.cleanup();
    }
  } finally { await task.destroy(); }

  const warnings: ExtractionResult["warnings"] = [];
  const characters = pages.reduce((sum, page) => sum + page.tokens.reduce((count, token) => count + token.text.length, 0), 0);
  if (characters < 20) warnings.push({ code: "OCR_REQUIRED", message: "Die Rechnung besteht aus einem Bild. Der Text konnte nicht automatisch gelesen werden." });
  return analyzeDocumentPages(pages, warnings, false);
}
