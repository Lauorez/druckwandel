import { describe, expect, it } from "vitest";
import { tokensInSelection, assignSourceValue } from "../apps/desktop/src/pdfSelection.js";
import { reviewDraftFromExtraction } from "../src/review/draft.js";
import { analyzeDocumentPages } from "../src/extraction/analyze.js";
import type { DocumentPage } from "../src/extraction/types.js";

describe("PDF source selection", () => {
  const page: DocumentPage = { page: 1, width: 600, height: 800, tokens: [
    { id: "company", page: 1, text: "Firma GmbH", box: { x: 50, y: 720, width: 120, height: 12 }, origin: "text-layer" },
    { id: "bank", page: 1, text: "IBAN", box: { x: 240, y: 720, width: 100, height: 12 }, origin: "text-layer" },
  ] };
  it("limits a dragged rectangle to the chosen footer column", () => {
    expect(tokensInSelection(page, { x: 40, y: 715, width: 150, height: 22 })).toEqual(["company"]);
    expect(tokensInSelection(page, { x: 400, y: 715, width: 100, height: 22 })).toEqual([]);
  });
  it("writes the assigned party and payment values without changing other fields", () => {
    const draft = reviewDraftFromExtraction(analyzeDocumentPages([page]));
    const next = assignSourceValue(assignSourceValue(draft, "buyerName", "Kunde GmbH"), "paymentTerms", "14 Tage");
    expect(next.buyer.name).toBe("Kunde GmbH");
    expect(next.payment.terms).toBe("14 Tage");
    expect(next.seller).toEqual(draft.seller);
    expect(draft.buyer.name).not.toBe("Kunde GmbH");
  });
});
