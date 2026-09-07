type PdfJsModule = typeof import("pdfjs-dist/legacy/build/pdf.mjs");

let pdfJsPromise: Promise<PdfJsModule> | undefined;

/** Loads the sizeable PDF renderer only when a document is actually opened. */
export function loadBrowserPdfJs(): Promise<PdfJsModule> {
  pdfJsPromise ??= Promise.all([
    import("pdfjs-dist/legacy/build/pdf.mjs"),
    import("pdfjs-dist/legacy/build/pdf.worker.mjs?url"),
  ]).then(([pdfJs, worker]) => {
    pdfJs.GlobalWorkerOptions.workerSrc = worker.default;
    return pdfJs;
  });
  return pdfJsPromise;
}
