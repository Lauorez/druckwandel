import { getDocument } from "pdfjs-dist/legacy/build/pdf.mjs";
import type { TextItem } from "pdfjs-dist/types/src/display/api.js";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import type { DocumentPage, SourceToken } from "./types.js";

const require = createRequire(import.meta.url);
const pdfJsBuild = require.resolve("pdfjs-dist/legacy/build/pdf.mjs");
const standardFontDataUrl = join(dirname(pdfJsBuild), "..", "..", "standard_fonts").replaceAll("\\", "/") + "/";

function isTextItem(value: unknown): value is TextItem {
  return typeof value === "object" && value !== null && "str" in value && "transform" in value;
}

export async function extractPdfText(pdf: Uint8Array): Promise<DocumentPage[]> {
  // Node.js Buffer extends Uint8Array, but PDF.js deliberately rejects Buffer.
  // Uint8Array.from always creates a plain Uint8Array and also isolates input data.
  const data = Uint8Array.from(pdf);
  const task = getDocument({
    data,
    useWorkerFetch: false,
    standardFontDataUrl,
  });
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
        const transform = item.transform;
        const x = transform[4] ?? 0;
        const baseline = transform[5] ?? 0;
        const height = Math.abs(item.height || transform[3] || 0);
        return [{
          id: `p${pageNumber}-t${index}`,
          page: pageNumber,
          text,
          box: { x, y: viewport.height - baseline - height, width: Math.abs(item.width), height },
          origin: "text-layer" as const,
        }];
      });
      pages.push({ page: pageNumber, width: viewport.width, height: viewport.height, tokens });
      page.cleanup();
    }
  } finally {
    await task.destroy();
  }
  return pages;
}
