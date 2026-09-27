/** Shared with the UI without loading the PDF conversion implementation. */
export class HybridPdfError extends Error {
  constructor(readonly code: string, message: string) {
    super(message);
    this.name = "HybridPdfError";
  }
}
