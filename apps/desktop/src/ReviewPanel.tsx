import type { CalculatedInvoice } from "../../../src/domain/types.js";
import { germanFieldLabel } from "../../../src/engine/validation-report.js";
import type { OfficialCheckIssue } from "./archiveStore.js";
import { decimal, money } from "../../../src/domain/money.js";
import { formatGermanDecimal } from "../../../src/domain/localized-decimal.js";
import type { ExtractedFieldName, ExtractionResult } from "../../../src/extraction/types.js";
import type { UnsupportedCase } from "../../../src/policy/unsupported-cases.js";
import { emptyReviewLine, type ReviewDraft, type ReviewLineDraft, type ReviewValidation } from "../../../src/review/draft.js";
import { LocalizedDecimalInput } from "./LocalizedDecimalInput.js";
import type { ReactNode } from "react";
import type { FieldSourceSelections, LearnableFieldName } from "../../../src/learning/correction-memory.js";

export type ActionFeedback = { kind: "success" | "error"; message: string };

interface ReviewPanelProps {
  extraction: ExtractionResult;
  draft: ReviewDraft;
  validation: ReviewValidation;
  zugferdValidation: ReviewValidation;
  calculated?: CalculatedInvoice;
  unsupportedCases: UnsupportedCase[];
  activeAction?: "draft" | "authority" | "pdf";
  validationPhase?: string;
  officialIssues?: OfficialCheckIssue[];
  feedback?: ActionFeedback;
  onDismissFeedback: () => void;
  learningRuleCount: number;
  onClearLearningMemory: () => void;
  onDraftChange: (draft: ReviewDraft) => void;
  onSelectField: (name: ExtractedFieldName) => void;
  onSelectTokens: (tokenIds: string[]) => void;
  onFocusPath: (path: string) => void;
  sourceSelections: FieldSourceSelections;
  onChooseSource: (name: LearnableFieldName, label: string) => void;
  onSave: () => void;
  onCreateXRechnung: () => void;
  onCreateZugferd: () => void;
}

const UNIT_OPTIONS = [
  ["C62", "Stück"],
  ["HUR", "Stunde"],
  ["DAY", "Tag"],
  ["KGM", "Kilogramm"],
  ["LTR", "Liter"],
  ["MTR", "Meter"],
] as const;
let manualLineSequence = 0;

function nextManualLineId(): string {
  manualLineSequence += 1;
  return `manual-${Date.now()}-${manualLineSequence}`;
}

const SOURCE_PATH: Record<string, string> = {
  invoiceNumber: "invoiceNumber",
  buyerReference: "buyerReference",
  issueDate: "issueDate",
  dueDate: "dueDate",
  serviceDate: "serviceDate",
  currency: "currency",
  sellerName: "seller.name",
  sellerAddressLine1: "seller.address.line1",
  sellerCity: "seller.address.city",
  sellerPostalCode: "seller.address.postalCode",
  sellerCountryCode: "seller.address.countryCode",
  buyerName: "buyer.name",
  buyerAddressLine1: "buyer.address.line1",
  buyerCity: "buyer.address.city",
  buyerPostalCode: "buyer.address.postalCode",
  buyerCountryCode: "buyer.address.countryCode",
};

export function reviewFieldId(path: string): string {
  return `review-field-${path.replace(/\./g, "-")}`;
}

function validationLabel(path: string): string {
  return germanFieldLabel(path);
}

function lineAmount(line: ReviewLineDraft): string | undefined {
  try {
    return money(decimal(line.quantity).mul(line.netUnitPrice));
  } catch {
    return undefined;
  }
}

export function ReviewPanel({
  extraction,
  draft,
  validation,
  zugferdValidation,
  calculated,
  unsupportedCases,
  activeAction,
  validationPhase,
  officialIssues,
  feedback,
  onDismissFeedback,
  learningRuleCount,
  onClearLearningMemory,
  onDraftChange,
  onSelectField,
  onSelectTokens,
  sourceSelections,
  onChooseSource,
  onFocusPath,
  onSave,
  onCreateXRechnung,
  onCreateZugferd,
}: ReviewPanelProps) {
  const updateLine = (index: number, changes: Partial<ReviewLineDraft>) => {
    onDraftChange({
      ...draft,
      lines: draft.lines.map((line, lineIndex) => lineIndex === index ? { ...line, ...changes } : line),
    });
  };
  const sourceField = (name: LearnableFieldName, label: string, control: ReactNode) => {
    const source = extraction.fields[name];
    const remembered = source?.transformations.some((step) => step.operation === "learned-layout");
    const assigned = sourceSelections[name] !== undefined;
    const accessibleLabel = name.startsWith("seller") ? `Absender: ${label}` : name.startsWith("buyer") && name !== "buyerReference" ? `Empfänger: ${label}` : label;
    const path = SOURCE_PATH[name] ?? name;
    return <div className="source-field" id={reviewFieldId(path)}>
      <label className={source || assigned ? "" : "missing"} onClick={() => onSelectField(name)}>
        <span>{label} {(source || assigned) && <small>{assigned ? "selbst zugeordnet" : remembered ? "aus ähnlicher Rechnung" : "übernommen"}</small>}</span>
        {control}
      </label>
      <button type="button" className="choose-source" aria-label={`${accessibleLabel}: Im PDF markieren`}
        disabled={Boolean(activeAction)} onClick={() => onChooseSource(name, accessibleLabel)}>Im PDF markieren</button>
    </div>;
  };
  const rememberedFieldCount = Object.values(extraction.fields)
    .filter((field) => field?.transformations.some((step) => step.operation === "learned-layout"))
    .length;
  const rememberedTable = extraction.warnings.some((warning) => warning.code === "LEARNED_TABLE");
  const rememberedAreaCount = rememberedFieldCount + (rememberedTable ? 1 : 0);

  return <aside className="review-panel">
    <ol className="workflow-steps" aria-label="Drei Schritte bis zur fertigen E-Rechnung">
      <li className="done"><b>1</b><span>Rechnung geöffnet</span></li>
      <li className="active"><b>2</b><span>Angaben prüfen</span></li>
      <li><b>3</b><span>Datei speichern</span></li>
    </ol>
    <p className="form-intro">Prüfen Sie die übernommenen Angaben. Fehlende Pflichtangaben sind hell markiert. Klicken Sie auf ein Feld, um die passende Stelle in der Rechnung zu sehen.</p>
    <div className={`learning-note${rememberedAreaCount > 0 ? " applied" : ""}`}>
      <div>
        <strong>{rememberedAreaCount > 0 ? `${rememberedAreaCount} ${rememberedAreaCount === 1 ? "Bereich wurde" : "Bereiche wurden"} aus ähnlichen Rechnungen ergänzt` : "Wird mit jeder Rechnung besser"}</strong>
        <span>Ergänzen Sie Angaben von Hand oder wählen Sie „Im PDF markieren“. Beim Speichern merkt sich der Assistent die passende Stelle für gleich aufgebaute Rechnungen. Alles bleibt auf diesem Computer.</span>
      </div>
      {learningRuleCount > 0 && <button type="button" onClick={onClearLearningMemory}>Gemerkte Ergänzungen löschen</button>}
    </div>
    {extraction.warnings.map((warning) => <div className="warning" key={warning.code}><strong>Bitte beachten</strong>{warning.message}</div>)}
    {unsupportedCases.map((unsupportedCase) => <button
      className="unsupported-case"
      key={unsupportedCase.code}
      onClick={() => onSelectTokens(unsupportedCase.sourceTokenIds)}
    >
      <strong>Diese Rechnung kann noch nicht sicher erstellt werden</strong>
      <span>{unsupportedCase.message}</span>
      {unsupportedCase.sourceTokenIds.length > 0 && <small>Stelle in der Rechnung zeigen</small>}
    </button>)}

    <section>
      <h2>Angaben zur Rechnung</h2>
      <div className="field-grid">
        {sourceField("invoiceNumber", "Rechnungsnummer", <input required value={draft.invoiceNumber} placeholder="Bitte eintragen" onChange={(event) => onDraftChange({ ...draft, invoiceNumber: event.target.value })} />)}
        {sourceField("buyerReference", "Bestellnummer oder Leitweg-ID", <input value={draft.buyerReference} placeholder="Für Rechnungen an Behörden" onChange={(event) => onDraftChange({ ...draft, buyerReference: event.target.value })} />)}
        {sourceField("issueDate", "Rechnungsdatum", <input required type="date" value={draft.issueDate} onChange={(event) => onDraftChange({ ...draft, issueDate: event.target.value })} />)}
        {sourceField("dueDate", "Fälligkeitsdatum", <input type="date" value={draft.dueDate} onChange={(event) => onDraftChange({ ...draft, dueDate: event.target.value })} />)}
        {sourceField("serviceDate", "Leistungsdatum", <input type="date" value={draft.serviceDate} onChange={(event) => onDraftChange({ ...draft, serviceDate: event.target.value })} />)}
        {sourceField("currency", "Währung (zum Beispiel EUR)", <input required maxLength={3} value={draft.currency} onChange={(event) => onDraftChange({ ...draft, currency: event.target.value.toUpperCase() })} />)}
      </div>
    </section>

    <section>
      <h2>Absender und Empfänger</h2>
      <div className="party-columns">
        <div><h3>Absender</h3><div className="field-grid single">
          {sourceField("sellerName", "Name", <input required value={draft.seller.name} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, name: event.target.value } })} />)}
          {sourceField("sellerAddressLine1", "Straße und Hausnummer", <input required value={draft.seller.addressLine1} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, addressLine1: event.target.value } })} />)}
          {sourceField("sellerPostalCode", "Postleitzahl", <input required value={draft.seller.postalCode} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, postalCode: event.target.value } })} />)}
          {sourceField("sellerCity", "Ort", <input required value={draft.seller.city} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, city: event.target.value } })} />)}
          {sourceField("sellerCountryCode", "Land (zum Beispiel DE)", <input required maxLength={2} value={draft.seller.countryCode} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, countryCode: event.target.value.toUpperCase() } })} />)}
          {sourceField("sellerVatId", "Umsatzsteuer-ID", <input value={draft.seller.vatId} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, vatId: event.target.value } })} />)}
        </div></div>
        <div><h3>Empfänger</h3><div className="field-grid single">
          {sourceField("buyerName", "Name", <input required value={draft.buyer.name} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, name: event.target.value } })} />)}
          {sourceField("buyerAddressLine1", "Straße und Hausnummer", <input required value={draft.buyer.addressLine1} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, addressLine1: event.target.value } })} />)}
          {sourceField("buyerPostalCode", "Postleitzahl", <input required value={draft.buyer.postalCode} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, postalCode: event.target.value } })} />)}
          {sourceField("buyerCity", "Ort", <input required value={draft.buyer.city} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, city: event.target.value } })} />)}
          {sourceField("buyerCountryCode", "Land (zum Beispiel DE)", <input required maxLength={2} value={draft.buyer.countryCode} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, countryCode: event.target.value.toUpperCase() } })} />)}
          {sourceField("buyerVatId", "Umsatzsteuer-ID (freiwillig)", <input value={draft.buyer.vatId} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, vatId: event.target.value } })} />)}
        </div></div>
      </div>
    </section>

    <section>
      <div className="section-title">
        <h2>Leistungen und Artikel <small>{draft.lines.length}</small></h2>
        <button className="compact-button" onClick={() => onDraftChange({ ...draft, lines: [...draft.lines, emptyReviewLine(nextManualLineId())] })}>+ Hinzufügen</button>
      </div>
      <div className="line-table">
        <div className="line-header"><span>Bezeichnung</span><span>Anzahl</span><span>Einheit</span><span>Preis</span><span>Steuer</span><span>Betrag</span><span /></div>
        {draft.lines.length === 0 && <div className="empty-lines">Noch keine Leistungen oder Artikel vorhanden.</div>}
        {draft.lines.map((line, index) => <div className="line-row" key={line.id} id={reviewFieldId(`lines.${index}.name`)}>
          <input aria-label={`Beschreibung Position ${index + 1}`} value={line.description} onFocus={() => onSelectTokens(line.sourceTokenIds)} onChange={(event) => updateLine(index, { description: event.target.value })} />
          <LocalizedDecimalInput aria-label={`Menge Position ${index + 1}`} value={line.quantity} minimumFractionDigits={0} maximumFractionDigits={3} onChange={(value) => updateLine(index, { quantity: value })} />
          <select aria-label={`Einheit Position ${index + 1}`} value={line.unitCode} onChange={(event) => updateLine(index, { unitCode: event.target.value as ReviewLineDraft["unitCode"] })}>
            {UNIT_OPTIONS.map(([code, label]) => <option key={code} value={code}>{label}</option>)}
          </select>
          <LocalizedDecimalInput aria-label={`Einzelpreis Position ${index + 1}`} value={line.netUnitPrice} onChange={(value) => updateLine(index, { netUnitPrice: value })} />
          <LocalizedDecimalInput aria-label={`Steuersatz Position ${index + 1}`} value={line.taxRate} minimumFractionDigits={0} onChange={(value) => updateLine(index, { taxRate: value })} />
          <output>{lineAmount(line) ? formatGermanDecimal(lineAmount(line)!) : "—"}</output>
          <button className="remove-line" aria-label={`Position ${index + 1} entfernen`} title="Position entfernen" onClick={() => onDraftChange({ ...draft, lines: draft.lines.filter((_, lineIndex) => lineIndex !== index) })}>Entfernen</button>
        </div>)}
      </div>
    </section>

    <section>
      <h2>Zahlung und Gesamtbetrag</h2>
      <div className="field-grid">
        {sourceField("iban", "IBAN (Kontonummer)", <input value={draft.payment.iban} onChange={(event) => onDraftChange({ ...draft, payment: { ...draft.payment, iban: event.target.value } })} />)}
        {sourceField("bic", "BIC (Bankkennung)", <input value={draft.payment.bic} onChange={(event) => onDraftChange({ ...draft, payment: { ...draft.payment, bic: event.target.value } })} />)}
      </div>
      <div className="field-grid single wide-field">{sourceField("paymentTerms", "Zahlungsbedingungen", <textarea value={draft.payment.terms} onChange={(event) => onDraftChange({ ...draft, payment: { ...draft.payment, terms: event.target.value } })} />)}</div>
      <div className="totals">
        <span>Nettobetrag <b>{calculated ? formatGermanDecimal(calculated.totals.lineNet) : "—"}</b></span>
        <span>Umsatzsteuer <b>{calculated ? formatGermanDecimal(calculated.totals.taxTotal) : "—"}</b></span>
        <span>Rechnungsbetrag <b>{calculated ? formatGermanDecimal(calculated.totals.payable) : "—"} {draft.currency}</b></span>
      </div>
    </section>

    {!validation.valid && <section className="validation-panel" aria-live="polite">
      <h2>{zugferdValidation.valid ? "Für eine Rechnung an Behörden fehlt noch" : "Bitte ergänzen Sie diese Angaben"}</h2>
      <ul>{validation.issues.slice(0, 8).map((issue) => <li key={`${issue.code}-${issue.path}`}><button type="button" className="issue-link" onClick={() => onFocusPath(issue.path)}><strong>{validationLabel(issue.path)}:</strong> {issue.message}</button></li>)}</ul>
    </section>}
    {officialIssues && officialIssues.length > 0 && <section className="validation-panel official" aria-live="polite">
      <h2>Die unabhängige Prüfung hat die Datei nicht angenommen</h2>
      <p>Der Entwurf bleibt gespeichert. Es wurde keine fertige E-Rechnung ausgegeben.</p>
      <ul>{officialIssues.slice(0, 10).map((issue) => <li key={`${issue.code}-${issue.path}-${issue.message}`}><button type="button" className="issue-link" onClick={() => onFocusPath(issue.path)}><strong>{validationLabel(issue.path)}:</strong> {issue.message}</button></li>)}</ul>
    </section>}
    {feedback && <div className={`feedback ${feedback.kind}`} role={feedback.kind === "error" ? "alert" : "status"}>
      <span>{feedback.message}</span>
      <button type="button" aria-label="Meldung schließen" onClick={onDismissFeedback}>Schließen</button>
    </div>}
    <div className="export-note"><strong>Wie möchten Sie die Rechnung weitergeben?</strong><span>Behörden benötigen meist die reine E-Rechnungsdatei. Für andere Kunden können Sie die Rechnung als PDF speichern.</span></div>

    <footer>
      <button className="secondary" disabled={Boolean(activeAction)} onClick={onSave}>
        {activeAction === "draft" ? "Entwurf wird gespeichert …" : "Entwurf speichern"}
      </button>
      <button className="secondary" disabled={Boolean(activeAction) || !validation.valid} title={validation.valid ? "E-Rechnungsdatei für Behörden speichern" : "Bitte ergänzen Sie zuerst die angezeigten Angaben"} onClick={onCreateXRechnung}>
        {activeAction === "authority" ? "Wird für Behörden gespeichert …" : "Für Behörden speichern"}
      </button>
      <button className="primary" disabled={Boolean(activeAction) || !zugferdValidation.valid} title={zugferdValidation.valid ? "Lesbare PDF-Rechnung speichern" : "Bitte ergänzen Sie zuerst die angezeigten Angaben"} onClick={onCreateZugferd}>
        {activeAction === "pdf" ? "PDF-Rechnung wird gespeichert …" : "Als PDF-Rechnung speichern"}
      </button>
    </footer>
  </aside>;
}
