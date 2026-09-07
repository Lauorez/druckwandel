import { useEffect, useRef, useState, type PointerEvent } from "react";
import { loadBrowserPdfJs } from "../../../src/extraction/browser-pdf.js";
import type { BoundingBox, DocumentPage } from "../../../src/extraction/types.js";
import { tokensInSelection } from "./pdfSelection.js";

interface PdfReviewProps {
  pdfBytes: Uint8Array;
  page: DocumentPage;
  selectedTokenIds: Set<string>;
  selectionEnabled?: boolean;
  onSelectSource?: (tokenIds: string[]) => void;
}

export function PdfReview({ pdfBytes, page, selectedTokenIds, selectionEnabled = false, onSelectSource }: PdfReviewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const start = useRef<{ x: number; y: number } | null>(null);
  const [selectionBox, setSelectionBox] = useState<BoundingBox>();
  const [renderError, setRenderError] = useState(false);
  useEffect(() => {
    let cancelled = false;
    let task: ReturnType<(typeof import("pdfjs-dist/legacy/build/pdf.mjs"))["getDocument"]> | undefined;
    setRenderError(false);
    void (async () => {
      const { getDocument } = await loadBrowserPdfJs();
      if (cancelled) return;
      task = getDocument({ data: Uint8Array.from(pdfBytes) });
      const document = await task.promise;
      const pdfPage = await document.getPage(page.page);
      const viewport = pdfPage.getViewport({ scale: 1.5 });
      const canvas = canvasRef.current;
      if (!canvas || cancelled) return;
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      await pdfPage.render({ canvas, canvasContext: canvas.getContext("2d")!, viewport }).promise;
    })().catch(() => { if (!cancelled) setRenderError(true); });
    return () => { cancelled = true; void task?.destroy(); };
  }, [pdfBytes, page.page]);

  useEffect(() => { start.current = null; setSelectionBox(undefined); }, [page.page, selectionEnabled]);
  const point = (event: PointerEvent<HTMLDivElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(page.width, (event.clientX - bounds.left) / bounds.width * page.width)),
      y: Math.max(0, Math.min(page.height, (event.clientY - bounds.top) / bounds.height * page.height)),
    };
  };
  const boxTo = (end: { x: number; y: number }): BoundingBox => ({
    x: Math.min(start.current!.x, end.x), y: Math.min(start.current!.y, end.y),
    width: Math.max(1, Math.abs(start.current!.x - end.x)), height: Math.max(1, Math.abs(start.current!.y - end.y)),
  });
  const boxStyle = (box: BoundingBox) => ({ left: `${box.x / page.width * 100}%`, top: `${box.y / page.height * 100}%`, width: `${box.width / page.width * 100}%`, height: `${box.height / page.height * 100}%` });

  const selected = page.tokens.filter((token) => selectedTokenIds.has(token.id));
  return <div className="pdf-page" style={{ aspectRatio: `${page.width}/${page.height}` }}>
    <canvas ref={canvasRef} aria-label={`Rechnung, Seite ${page.page}`} />
    {renderError && <div className="warning" role="alert">Die Vorschau konnte nicht angezeigt werden. Bitte öffnen Sie die Rechnung erneut.</div>}
    {selectionEnabled && <div className="pdf-selection-layer" aria-label="Stelle in der Rechnung markieren"
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        start.current = point(event);
        event.currentTarget.setPointerCapture(event.pointerId);
        setSelectionBox(boxTo(start.current));
        event.preventDefault();
      }}
      onPointerMove={(event) => { if (start.current) setSelectionBox(boxTo(point(event))); }}
      onPointerUp={(event) => {
        if (!start.current) return;
        const box = boxTo(point(event));
        start.current = null;
        setSelectionBox(undefined);
        event.currentTarget.releasePointerCapture(event.pointerId);
        onSelectSource?.(tokensInSelection(page, box));
      }}
      onPointerCancel={() => { start.current = null; setSelectionBox(undefined); }}>
      {page.tokens.map((token) => <button key={token.id} type="button" className="pdf-source-token"
        style={boxStyle(token.box)} aria-label={token.text} title={token.text}
        onClick={(event) => { if (event.detail === 0) onSelectSource?.([token.id]); }} />)}
    </div>}
    <div className="overlay" aria-hidden="true">
      {selected.map((token) => <span key={token.id} className="source-highlight" style={boxStyle(token.box)} />)}
      {selectionBox && <span className="selection-rectangle" style={boxStyle(selectionBox)} />}
    </div>
  </div>;
}
