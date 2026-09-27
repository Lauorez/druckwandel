// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ReviewPanel } from "../apps/desktop/src/ReviewPanel.js";
import { analyzeDocumentPages } from "../src/extraction/analyze.js";
import { reviewDraftFromExtraction, validateReviewDraft } from "../src/review/draft.js";
import type { ReviewDraft } from "../src/review/draft.js";

const extraction = analyzeDocumentPages([{
  page: 1, width: 600, height: 800,
  tokens: [
    { id: "t1", page: 1, text: "GUTSCHRIFT", box: { x: 50, y: 40, width: 120, height: 12 }, origin: "text-layer" },
    { id: "t2", page: 1, text: "Ursprungsrechnung: RE-100", box: { x: 50, y: 60, width: 180, height: 10 }, origin: "text-layer" },
    { id: "t3", page: 1, text: "Rabatt 10 %: 10,00 EUR", box: { x: 50, y: 400, width: 160, height: 10 }, origin: "text-layer" },
  ],
}]);

function filled(draft: ReviewDraft): ReviewDraft {
  return {
    ...draft,
    invoiceNumber: draft.invoiceNumber || "GS-1",
    issueDate: draft.issueDate || "2026-08-30",
    currency: "EUR",
    buyerReference: "04011000-12345-03",
    seller: { ...draft.seller, name: "Anbieter GmbH", addressLine1: "Straße 1", postalCode: "10115", city: "Berlin", countryCode: "DE", vatId: "DE123456789", contactName: "Erika", phone: "+49 30 123456", email: "rechnung@muster.invalid" },
    buyer: { ...draft.buyer, name: "Kunde AG", addressLine1: "Weg 2", postalCode: "20095", city: "Hamburg", countryCode: "DE" },
    lines: draft.lines.length ? draft.lines : [{ id: "1", description: "Leistung", quantity: "1", unitCode: "C62", netUnitPrice: "100", taxRate: "19", taxCase: "S19", exemptionReason: "", sourceTokenIds: [], allowances: [] }],
  };
}

describe("review document type and allowances", () => {
  afterEach(() => cleanup());
  it("preselects a credit note, shows the original invoice, and keeps Rabatt editable", () => {
    const draft = filled(reviewDraftFromExtraction(extraction));
    expect(draft.invoiceType).toBe("381");
    expect(draft.precedingInvoiceNumber).toBe("RE-100");
    expect(draft.allowances[0]).toMatchObject({ reason: "Rabatt", amount: "10.00" });
    const validation = validateReviewDraft(draft);
    const onDraftChange = (next: ReviewDraft) => { Object.assign(draft, next); };
    render(<ReviewPanel
      extraction={extraction}
      draft={draft}
      validation={validation}
      zugferdValidation={validateReviewDraft(draft, [], "zugferd")}
      calculated={validation.invoice}
      hybridConfirmed={false}
      onHybridConfirmedChange={() => undefined}
      unsupportedCases={[]}
      learningProfiles={[{ id: "default", name: "Standard" }]}
      activeLearningProfileId="default"
      onSelectLearningProfile={() => undefined}
      onDraftChange={onDraftChange}
      onSelectField={() => undefined}
      onSelectTokens={() => undefined}
      onFocusPath={() => undefined}
      sourceSelections={{}}
      onChooseSource={() => undefined}
      onSave={() => undefined}
      onCreateXRechnung={() => undefined}
      onCreateZugferd={() => undefined}
      onDismissFeedback={() => undefined}
    />);
    expect(screen.getByLabelText("Belegart")).toHaveProperty("value", "381");
    expect(screen.getByLabelText("Nummer der Ursprungsrechnung")).toHaveProperty("value", "RE-100");
    expect(screen.getByLabelText("Gutschriftnummer")).toBeTruthy();
    expect(screen.getByLabelText("Betrag Zu- oder Abschlag 1")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Für Behörden speichern" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Als PDF-Rechnung speichern" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Belegart"), { target: { value: "380" } });
    expect(onDraftChange).toBeDefined();
  });

  it("lets the reviewer choose reverse charge instead of typing 0 %", () => {
    const draft = filled({
      ...reviewDraftFromExtraction(extraction),
      invoiceType: "380",
      precedingInvoiceNumber: "",
      lines: [{ id: "1", description: "Beratung", quantity: "2", unitCode: "HUR", netUnitPrice: "100", taxRate: "0", taxCase: "", exemptionReason: "", sourceTokenIds: [], allowances: [] }],
    });
    const validation = validateReviewDraft(draft);
    render(<ReviewPanel
      extraction={extraction}
      draft={draft}
      validation={validation}
      zugferdValidation={validateReviewDraft(draft, [], "zugferd")}
      calculated={validation.invoice}
      hybridConfirmed={false}
      onHybridConfirmedChange={() => undefined}
      unsupportedCases={[]}
      learningProfiles={[{ id: "default", name: "Standard" }]}
      activeLearningProfileId="default"
      onSelectLearningProfile={() => undefined}
      onDraftChange={() => undefined}
      onSelectField={() => undefined}
      onSelectTokens={() => undefined}
      onFocusPath={() => undefined}
      sourceSelections={{}}
      onChooseSource={() => undefined}
      onSave={() => undefined}
      onCreateXRechnung={() => undefined}
      onCreateZugferd={() => undefined}
      onDismissFeedback={() => undefined}
    />);
    expect(screen.getByLabelText("Steuer Position 1")).toHaveProperty("value", "");
    expect(screen.queryByLabelText("Steuersatz Position 1")).toBeNull();
  });

  it("shows preceding invoices and prepaid amounts for a final invoice", () => {
    const source = analyzeDocumentPages([{
      page: 1, width: 600, height: 800,
      tokens: [
        { id: "t1", page: 1, text: "Schlussrechnung", box: { x: 50, y: 40, width: 140, height: 12 }, origin: "text-layer" },
        { id: "t2", page: 1, text: "Abschlag 1: RE-A-1 vom 01.07.2026 100,00", box: { x: 50, y: 60, width: 260, height: 10 }, origin: "text-layer" },
      ],
    }]);
    const draft = filled(reviewDraftFromExtraction(source));
    expect(draft.finalInvoice).toBe(true);
    expect(draft.precedingInvoices[0]).toMatchObject({ invoiceNumber: "RE-A-1", paidAmount: "100.00" });
    const validation = validateReviewDraft(draft);
    render(<ReviewPanel
      extraction={source}
      draft={draft}
      validation={validation}
      zugferdValidation={validateReviewDraft(draft, [], "zugferd")}
      calculated={validation.invoice}
      hybridConfirmed={false}
      onHybridConfirmedChange={() => undefined}
      unsupportedCases={[]}
      learningProfiles={[{ id: "default", name: "Standard" }]}
      activeLearningProfileId="default"
      onSelectLearningProfile={() => undefined}
      onDraftChange={() => undefined}
      onSelectField={() => undefined}
      onSelectTokens={() => undefined}
      onFocusPath={() => undefined}
      sourceSelections={{}}
      onChooseSource={() => undefined}
      onSave={() => undefined}
      onCreateXRechnung={() => undefined}
      onCreateZugferd={() => undefined}
      onDismissFeedback={() => undefined}
    />);
    expect(screen.getByLabelText("Belegart")).toHaveProperty("value", "final");
    expect(screen.getByLabelText("Nummer bisherige Rechnung 1")).toHaveProperty("value", "RE-A-1");
    expect(screen.getByLabelText("Betrag bisherige Rechnung 1")).toBeTruthy();
    expect(screen.getByLabelText("Bereits gezahlt")).toBeTruthy();
  });
});
