import type { ContentConsistency } from "../../../src/engine/consistency.js";
import type { CalculatedInvoice } from "../../../src/domain/types.js";
import { germanFieldLabel } from "../../../src/engine/validation-report.js";
import type { OfficialCheckIssue } from "./archiveStore.js";
import { documentAdjustmentTotals, lineNetAmount, prepaidAmountOf } from "../../../src/domain/calculate.js";
import { isCreditOrCorrection, isPartialInvoice } from "../../../src/domain/types.js";
import { money } from "../../../src/domain/money.js";
import { formatGermanDecimal } from "../../../src/domain/localized-decimal.js";
import type { ExtractedFieldName, ExtractionResult } from "../../../src/extraction/types.js";
import type { UnsupportedCase } from "../../../src/policy/unsupported-cases.js";
import { TAX_CASES, applyReviewTaxCase, taxCaseById, taxTreatmentFromCase } from "../../../src/policy/tax-cases.js";
import { applyReviewDocumentKind, draftRequiresVatIds, emptyPrecedingInvoice, emptyReviewAllowance, emptyReviewLine, reviewKindValue, type ReviewAllowanceDraft, type ReviewDraft, type ReviewLineDraft, type ReviewPrecedingInvoiceDraft, type ReviewValidation } from "../../../src/review/draft.js";
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
  consistency?: ContentConsistency;
  hybridConfirmed: boolean;
  onHybridConfirmedChange: (confirmed: boolean) => void;
  unsupportedCases: UnsupportedCase[];
  activeAction?: "draft" | "authority" | "pdf";
  validationPhase?: string;
  officialIssues?: OfficialCheckIssue[];
  feedback?: ActionFeedback;
  onDismissFeedback: () => void;
  learningProfiles: Array<{ id: string; name: string }>;
  activeLearningProfileId: string;
  onSelectLearningProfile: (profileId: string) => void;
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
let manualAllowanceSequence = 0;

function nextManualLineId(): string {
  manualLineSequence += 1;
  return `manual-${Date.now()}-${manualLineSequence}`;
}

function nextAllowanceId(prefix: string): string {
  manualAllowanceSequence += 1;
  return `${prefix}-${Date.now()}-${manualAllowanceSequence}`;
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
    return money(lineNetAmount({
      id: line.id,
      name: line.description,
      quantity: line.quantity,
      unitCode: line.unitCode,
      netUnitPrice: line.netUnitPrice,
      tax: taxTreatmentFromCase(line.taxCase, line.taxRate, line.exemptionReason),
      allowances: line.allowances.filter((item) => item.amount.trim()).map((item) => ({
        charge: item.charge,
        amount: item.amount,
        tax: taxTreatmentFromCase(item.taxCase || line.taxCase, item.taxRate || line.taxRate, item.exemptionReason || line.exemptionReason),
      })),
    }));
  } catch {
    return undefined;
  }
}

function lineDiscount(line: ReviewLineDraft): string {
  return line.allowances.find((item) => !item.charge)?.amount ?? "";
}

function showSettlement(draft: ReviewDraft): boolean {
  if (isCreditOrCorrection(draft.invoiceType)) return false;
  return draft.finalInvoice || draft.prepaymentInvoice || isPartialInvoice(draft.invoiceType) || draft.precedingInvoices.length > 0 || Boolean(draft.prepaidAmount.trim());
}

function originalDocumentLabel(draft: ReviewDraft): string {
  if (draft.invoiceType === "381") return "die Originalgutschrift";
  if (draft.invoiceType === "384") return "die Originalkorrektur";
  if (draft.finalInvoice) return "die Originalschlussrechnung";
  if (draft.invoiceType === "326") return "die Originalabschlagsrechnung";
  if (draft.prepaymentInvoice) return "die Originalanzahlungsrechnung";
  return "die Originalrechnung";
}

export function ReviewPanel({
  extraction,
  draft,
  validation,
  zugferdValidation,
  calculated,
  consistency,
  hybridConfirmed,
  onHybridConfirmedChange,
  unsupportedCases,
  activeAction,
  validationPhase,
  officialIssues,
  feedback,
  onDismissFeedback,
  learningProfiles,
  activeLearningProfileId,
  onSelectLearningProfile,
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
  const applyLineTaxCase = (index: number, taxCaseId: string) => {
    const line = draft.lines[index]!;
    const tax = applyReviewTaxCase(taxCaseId, line.exemptionReason);
    updateLine(index, {
      ...tax,
      allowances: line.allowances.map((item) => ({ ...item, ...applyReviewTaxCase(taxCaseId, item.exemptionReason) })),
    });
  };
  const setLineDiscount = (index: number, amount: string) => {
    const line = draft.lines[index]!;
    const charges = line.allowances.filter((item) => item.charge);
    const existing = line.allowances.find((item) => !item.charge);
    const allowances = amount.trim()
      ? [{ id: existing?.id ?? nextAllowanceId(`line-${line.id}`), charge: false, reason: existing?.reason || "Rabatt", amount, taxRate: line.taxRate, taxCase: line.taxCase, exemptionReason: line.exemptionReason }, ...charges]
      : charges;
    updateLine(index, { allowances });
  };
  const updateAllowance = (index: number, changes: Partial<ReviewAllowanceDraft>) => {
    onDraftChange({
      ...draft,
      allowances: draft.allowances.map((item, itemIndex) => itemIndex === index ? { ...item, ...changes } : item),
    });
  };
  const updatePreceding = (index: number, changes: Partial<ReviewPrecedingInvoiceDraft>) => {
    onDraftChange({
      ...draft,
      precedingInvoices: draft.precedingInvoices.map((item, itemIndex) => itemIndex === index ? { ...item, ...changes } : item),
    });
  };
  const defaultTaxCase = draft.lines.find((line) => line.taxCase)?.taxCase || "S19";
  const vatRequired = draftRequiresVatIds(draft);
  const adjustments = calculated ? documentAdjustmentTotals(calculated.allowances) : undefined;
  const prepaid = calculated ? prepaidAmountOf(calculated) : undefined;
  const sourceField = (name: LearnableFieldName, label: string, control: ReactNode) => {
    const source = extraction.fields[name];
    const remembered = source?.transformations.some((step) => step.operation === "learned-layout");
    const assigned = sourceSelections[name] !== undefined;
    const accessibleLabel = name.startsWith("seller") ? `Absender: ${label}` : name.startsWith("buyer") && name !== "buyerReference" ? `Empfänger: ${label}` : label;
    const path = SOURCE_PATH[name] ?? name;
    return <div className="source-field" id={reviewFieldId(path)}>
      <label className={source || assigned ? "" : "missing"} onClick={() => onSelectField(name)}>
        <span>{label} {(source || assigned) && <small>{assigned ? "selbst zugeordnet" : remembered ? (source && source.confidence < 0.7 ? "unsicher, bitte prüfen" : "aus ähnlicher Rechnung") : "übernommen"}</small>}</span>
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
      {rememberedAreaCount > 0 && <strong>{rememberedAreaCount} {rememberedAreaCount === 1 ? "Bereich wurde" : "Bereiche wurden"} aus ähnlichen Rechnungen ergänzt</strong>}
      <div className="learning-profiles">
        <label>Erkennungsprofil
          <select aria-label="Erkennungsprofil" value={activeLearningProfileId} onChange={(event) => onSelectLearningProfile(event.target.value)}>
            {learningProfiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
          </select>
        </label>
      </div>
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
        <label id={reviewFieldId("invoiceType")}><span>Belegart</span>
          <select aria-label="Belegart" value={reviewKindValue(draft)} onChange={(event) => onDraftChange(applyReviewDocumentKind(draft, event.target.value))}>
            <option value="380">Rechnung</option>
            <option value="381">Gutschrift</option>
            <option value="384">Rechnungskorrektur</option>
            <option value="326">Abschlagsrechnung</option>
            <option value="prepayment">Anzahlungsrechnung</option>
            <option value="final">Schlussrechnung</option>
          </select>
        </label>
        {sourceField("invoiceNumber", draft.invoiceType === "381" ? "Gutschriftnummer" : "Rechnungsnummer", <input required value={draft.invoiceNumber} placeholder="Bitte eintragen" onChange={(event) => onDraftChange({ ...draft, invoiceNumber: event.target.value })} />)}
        {sourceField("buyerReference", "Bestellnummer oder Leitweg-ID", <input value={draft.buyerReference} placeholder="Für Rechnungen an Behörden" onChange={(event) => onDraftChange({ ...draft, buyerReference: event.target.value })} />)}
        {sourceField("issueDate", "Rechnungsdatum", <input required type="date" value={draft.issueDate} onChange={(event) => onDraftChange({ ...draft, issueDate: event.target.value })} />)}
        {sourceField("dueDate", "Fälligkeitsdatum", <input type="date" value={draft.dueDate} onChange={(event) => onDraftChange({ ...draft, dueDate: event.target.value })} />)}
        {sourceField("serviceDate", "Leistungsdatum", <input type="date" value={draft.serviceDate} onChange={(event) => onDraftChange({ ...draft, serviceDate: event.target.value })} />)}
        {sourceField("currency", "Währung (zum Beispiel EUR)", <input required maxLength={3} value={draft.currency} onChange={(event) => onDraftChange({ ...draft, currency: event.target.value.toUpperCase() })} />)}
      </div>
      {(draft.lines.some(line => line.taxCase === "K") || draft.allowances.some(item => item.taxCase === "K") || draft.deliveryAddress) && <div className="field-grid">
        <p className="document-hint">Bitte die tatsächliche Lieferanschrift eintragen. Für Behörden sind Ort und Postleitzahl erforderlich.</p>
        {([ ["line1", "Lieferanschrift: Straße"], ["postalCode", "Lieferanschrift: Postleitzahl"], ["city", "Lieferanschrift: Ort"], ["countryCode", "Lieferland (Ländercode)"] ] as const).map(([key, label]) => <label key={key} id={reviewFieldId(`deliveryAddress.${key}`)}>
          <span>{label}</span>
          <input value={draft.deliveryAddress?.[key] ?? ""} maxLength={key === "countryCode" ? 2 : undefined} onChange={event => onDraftChange({ ...draft, deliveryAddress: {
            line1: "", postalCode: "", city: "", countryCode: "", ...draft.deliveryAddress,
            [key]: key === "countryCode" ? event.target.value.toUpperCase() : event.target.value,
          } })} />
        </label>)}
      </div>}
      {(draft.invoiceType === "381" || draft.invoiceType === "384") && <div className="field-grid preceding-invoice">
        <p className="document-hint">Beträge bleiben positiv. Die Belegart sagt, dass dieser Beleg die Ursprungsrechnung berichtigt.</p>
        <label id={reviewFieldId("precedingInvoice.invoiceNumber")} className={draft.precedingInvoiceNumber ? "" : "missing"}><span>Nummer der Ursprungsrechnung</span>
          <input required value={draft.precedingInvoiceNumber} placeholder="z. B. RE-2026-001" onChange={(event) => onDraftChange({ ...draft, precedingInvoiceNumber: event.target.value })} /></label>
        <label id={reviewFieldId("precedingInvoice.issueDate")}><span>Datum der Ursprungsrechnung</span>
          <input type="date" value={draft.precedingInvoiceDate} onChange={(event) => onDraftChange({ ...draft, precedingInvoiceDate: event.target.value })} /></label>
      </div>}
      {showSettlement(draft) && <div className="preceding-invoice">
        <div className="section-title">
          <h2>Bisherige Rechnungen und Zahlungen <small>{draft.precedingInvoices.length}</small></h2>
          <button className="compact-button" onClick={() => onDraftChange({ ...draft, precedingInvoices: [...draft.precedingInvoices, emptyPrecedingInvoice()] })}>+ Hinzufügen</button>
        </div>
        <p className="document-hint">{draft.finalInvoice
          ? "Die Schlussrechnung enthält den Restbetrag. Tragen Sie die bisherigen Abschläge oder Anzahlungen mit ihren bereits berechneten Beträgen ein, nicht als Rabatt."
          : "Bereits gezahlte oder berechnete Beträge gehören zu den bisherigen Rechnungen. Das ist kein Nachlass auf diese Rechnung."}</p>
        <div className="allowance-table settlement-table">
          <div className="settlement-header"><span>Rechnungsnummer</span><span>Datum</span><span>Bereits berechnet</span><span /></div>
          {draft.precedingInvoices.length === 0 && <div className="empty-lines">Keine bisherigen Abschläge oder Anzahlungen.</div>}
          {draft.precedingInvoices.map((item, index) => <div className="settlement-row" key={`preceding-${index}`} id={reviewFieldId(`precedingInvoices.${index}.invoiceNumber`)}>
            <input aria-label={`Nummer bisherige Rechnung ${index + 1}`} value={item.invoiceNumber} placeholder="z. B. RE-A-1" onChange={(event) => updatePreceding(index, { invoiceNumber: event.target.value })} />
            <input aria-label={`Datum bisherige Rechnung ${index + 1}`} type="date" value={item.issueDate} onChange={(event) => updatePreceding(index, { issueDate: event.target.value })} />
            <LocalizedDecimalInput aria-label={`Betrag bisherige Rechnung ${index + 1}`} value={item.paidAmount} onChange={(value) => updatePreceding(index, { paidAmount: value })} />
            <button className="remove-line" aria-label={`Bisherige Rechnung ${index + 1} entfernen`} onClick={() => onDraftChange({ ...draft, precedingInvoices: draft.precedingInvoices.filter((_, itemIndex) => itemIndex !== index) })}>Entfernen</button>
          </div>)}
        </div>
        <label id={reviewFieldId("prepaidAmount")} className="prepaid-total"><span>Bereits gezahlt insgesamt</span>
          <LocalizedDecimalInput aria-label="Bereits gezahlt" value={draft.prepaidAmount} onChange={(value) => onDraftChange({ ...draft, prepaidAmount: value })} />
        </label>
      </div>}
    </section>

    <section>
      <h2>Absender und Empfänger</h2>
      {vatRequired && <p className="document-hint">Reverse Charge und innergemeinschaftliche Lieferungen brauchen die Umsatzsteuer-ID von Absender und Empfänger.</p>}
      <div className="party-columns">
        <div><h3>Absender</h3><div className="field-grid single">
          {sourceField("sellerName", "Name", <input required value={draft.seller.name} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, name: event.target.value } })} />)}
          {sourceField("sellerAddressLine1", "Straße und Hausnummer", <input required value={draft.seller.addressLine1} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, addressLine1: event.target.value } })} />)}
          {sourceField("sellerPostalCode", "Postleitzahl", <input required value={draft.seller.postalCode} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, postalCode: event.target.value } })} />)}
          {sourceField("sellerCity", "Ort", <input required value={draft.seller.city} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, city: event.target.value } })} />)}
          {sourceField("sellerCountryCode", "Land (zum Beispiel DE)", <input required maxLength={2} value={draft.seller.countryCode} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, countryCode: event.target.value.toUpperCase() } })} />)}
          {sourceField("sellerVatId", "Umsatzsteuer-ID", <input value={draft.seller.vatId} onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, vatId: event.target.value } })} />)}
          <label id={reviewFieldId("seller.contact.name")}><span>Ansprechpartner</span>
            <input value={draft.seller.contactName ?? ""} placeholder="Für Rechnungen an Behörden" onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, contactName: event.target.value } })} /></label>
          <label id={reviewFieldId("seller.contact.phone")}><span>Telefon</span>
            <input value={draft.seller.phone ?? ""} placeholder="+49 …" onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, phone: event.target.value } })} /></label>
          <label id={reviewFieldId("seller.contact.email")}><span>E-Mail</span>
            <input value={draft.seller.email ?? ""} placeholder="rechnung@firma.example" onChange={(event) => onDraftChange({ ...draft, seller: { ...draft.seller, email: event.target.value } })} /></label>
        </div></div>
        <div><h3>Empfänger</h3><div className="field-grid single">
          {sourceField("buyerName", "Name", <input required value={draft.buyer.name} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, name: event.target.value } })} />)}
          {sourceField("buyerAddressLine1", "Straße und Hausnummer", <input required value={draft.buyer.addressLine1} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, addressLine1: event.target.value } })} />)}
          {sourceField("buyerPostalCode", "Postleitzahl", <input required value={draft.buyer.postalCode} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, postalCode: event.target.value } })} />)}
          {sourceField("buyerCity", "Ort", <input required value={draft.buyer.city} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, city: event.target.value } })} />)}
          {sourceField("buyerCountryCode", "Land (zum Beispiel DE)", <input required maxLength={2} value={draft.buyer.countryCode} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, countryCode: event.target.value.toUpperCase() } })} />)}
          {sourceField("buyerVatId", vatRequired ? "Umsatzsteuer-ID" : "Umsatzsteuer-ID (freiwillig)", <input value={draft.buyer.vatId} onChange={(event) => onDraftChange({ ...draft, buyer: { ...draft.buyer, vatId: event.target.value } })} />)}
        </div></div>
      </div>
    </section>

    <section>
      <div className="section-title">
        <h2>Leistungen und Artikel <small>{draft.lines.length}</small></h2>
        <button className="compact-button" onClick={() => onDraftChange({ ...draft, lines: [...draft.lines, emptyReviewLine(nextManualLineId())] })}>+ Hinzufügen</button>
      </div>
      <div className="line-table">
        <div className="line-header"><span>Bezeichnung</span><span>Anzahl</span><span>Einheit</span><span>Preis</span><span>Nachlass</span><span>Steuer</span><span>Betrag</span><span /></div>
        {draft.lines.length === 0 && <div className="empty-lines">Noch keine Leistungen oder Artikel vorhanden.</div>}
        {draft.lines.map((line, index) => {
          const tax = taxCaseById(line.taxCase);
          return <div className="line-block" key={line.id}>
            <div className="line-row" id={reviewFieldId(`lines.${index}.name`)}>
              <input aria-label={`Beschreibung Position ${index + 1}`} value={line.description} onFocus={() => onSelectTokens(line.sourceTokenIds)} onChange={(event) => updateLine(index, { description: event.target.value })} />
              <LocalizedDecimalInput aria-label={`Menge Position ${index + 1}`} value={line.quantity} minimumFractionDigits={0} maximumFractionDigits={3} onChange={(value) => updateLine(index, { quantity: value })} />
              <select aria-label={`Einheit Position ${index + 1}`} value={line.unitCode} onChange={(event) => updateLine(index, { unitCode: event.target.value as ReviewLineDraft["unitCode"] })}>
                {UNIT_OPTIONS.map(([code, label]) => <option key={code} value={code}>{label}</option>)}
              </select>
              <LocalizedDecimalInput aria-label={`Einzelpreis Position ${index + 1}`} value={line.netUnitPrice} onChange={(value) => updateLine(index, { netUnitPrice: value })} />
              <LocalizedDecimalInput aria-label={`Nachlass Position ${index + 1}`} value={lineDiscount(line)} onChange={(value) => setLineDiscount(index, value)} />
              <select aria-label={`Steuer Position ${index + 1}`} value={line.taxCase} onChange={(event) => applyLineTaxCase(index, event.target.value)}>
                <option value="">Bitte wählen</option>
                {TAX_CASES.map((item) => <option key={item.id} value={item.id}>{item.shortLabel}</option>)}
              </select>
              <output>{lineAmount(line) ? formatGermanDecimal(lineAmount(line)!) : "—"}</output>
              <button className="remove-line" aria-label={`Position ${index + 1} entfernen`} title="Position entfernen" onClick={() => onDraftChange({ ...draft, lines: draft.lines.filter((_, lineIndex) => lineIndex !== index) })}>Entfernen</button>
            </div>
            {tax?.exemptionReasonRequired && <div className="line-subrow" id={reviewFieldId(`lines.${index}.tax.exemptionReason`)}>
              <label className={line.exemptionReason.trim() ? "" : "missing"}>
                <span>Grund der Steuerbefreiung</span>
                <input aria-label={`Grund der Steuerbefreiung Position ${index + 1}`} value={line.exemptionReason} placeholder={tax.id === "E" ? "z. B. § 4 Nr. … UStG" : undefined} onChange={(event) => updateLine(index, { exemptionReason: event.target.value })} />
              </label>
            </div>}
          </div>;
        })}
      </div>
    </section>

    <section>
      <div className="section-title">
        <h2>Zu- und Abschläge <small>{draft.allowances.length}</small></h2>
        <button className="compact-button" onClick={() => onDraftChange({ ...draft, allowances: [...draft.allowances, emptyReviewAllowance(nextAllowanceId("doc"), false, defaultTaxCase)] })}>+ Hinzufügen</button>
      </div>
      <p className="section-note">Nachlässe auf die ganze Rechnung, zum Beispiel ein ausgewiesener Rabatt. Das Wort „Rabatt“ allein sperrt die Erstellung nicht.</p>
      <div className="allowance-table">
        <div className="allowance-header"><span>Art</span><span>Grund</span><span>Betrag</span><span>Steuer</span><span /></div>
        {draft.allowances.length === 0 && <div className="empty-lines">Keine belegweiten Zu- oder Abschläge.</div>}
        {draft.allowances.map((item, index) => {
          const tax = taxCaseById(item.taxCase);
          return <div className="allowance-block" key={item.id}>
            <div className="allowance-row" id={reviewFieldId(`allowances.${index}.amount`)}>
              <select aria-label={`Art Zu- oder Abschlag ${index + 1}`} value={item.charge ? "charge" : "allowance"} onChange={(event) => updateAllowance(index, { charge: event.target.value === "charge", reason: event.target.value === "charge" ? (item.reason || "Zuschlag") : (item.reason || "Rabatt") })}>
                <option value="allowance">Nachlass</option>
                <option value="charge">Zuschlag</option>
              </select>
              <input aria-label={`Grund Zu- oder Abschlag ${index + 1}`} value={item.reason} onChange={(event) => updateAllowance(index, { reason: event.target.value })} />
              <LocalizedDecimalInput aria-label={`Betrag Zu- oder Abschlag ${index + 1}`} value={item.amount} onChange={(value) => updateAllowance(index, { amount: value })} />
              <select aria-label={`Steuer Zu- oder Abschlag ${index + 1}`} value={item.taxCase} onChange={(event) => updateAllowance(index, applyReviewTaxCase(event.target.value, item.exemptionReason))}>
                <option value="">Bitte wählen</option>
                {TAX_CASES.map((option) => <option key={option.id} value={option.id}>{option.shortLabel}</option>)}
              </select>
              <button className="remove-line" aria-label={`Zu- oder Abschlag ${index + 1} entfernen`} onClick={() => onDraftChange({ ...draft, allowances: draft.allowances.filter((_, itemIndex) => itemIndex !== index) })}>Entfernen</button>
            </div>
            {tax?.exemptionReasonRequired && <div className="line-subrow" id={reviewFieldId(`allowances.${index}.tax.exemptionReason`)}>
              <label className={item.exemptionReason.trim() ? "" : "missing"}>
                <span>Grund der Steuerbefreiung</span>
                <input aria-label={`Grund der Steuerbefreiung Zu- oder Abschlag ${index + 1}`} value={item.exemptionReason} onChange={(event) => updateAllowance(index, { exemptionReason: event.target.value })} />
              </label>
            </div>}
          </div>;
        })}
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
        {adjustments && (adjustments.allowanceTotal.gt(0) || adjustments.chargeTotal.gt(0)) ? <>
          <span>Positionssumme <b>{formatGermanDecimal(calculated!.totals.lineNet)}</b></span>
          {adjustments.allowanceTotal.gt(0) && <span>Nachlässe <b>−{formatGermanDecimal(money(adjustments.allowanceTotal))}</b></span>}
          {adjustments.chargeTotal.gt(0) && <span>Zuschläge <b>{formatGermanDecimal(money(adjustments.chargeTotal))}</b></span>}
        </> : null}
        <span>Nettobetrag <b>{calculated ? formatGermanDecimal(calculated.totals.taxExclusive) : "—"}</b><small>Original: {extraction.fields.lineNet?.value || "nicht gelesen"}</small></span>
        <span>Umsatzsteuer <b>{calculated ? formatGermanDecimal(calculated.totals.taxTotal) : "—"}</b><small>Original: {extraction.fields.taxTotal?.value || "nicht gelesen"}</small></span>
        {prepaid?.gt(0) ? <>
          <span>Rechnungsbetrag <b>{formatGermanDecimal(calculated!.totals.taxInclusive)}</b><small>Original: {extraction.fields.taxInclusive?.value || "nicht gelesen"}</small></span>
          <span>Bereits gezahlt <b>−{formatGermanDecimal(money(prepaid))}</b></span>
          <span>Zahlbetrag <b>{formatGermanDecimal(calculated!.totals.payable)} {draft.currency}</b><small>Original: {extraction.fields.payable?.value || "nicht gelesen"}</small></span>
        </> : <span>Rechnungsbetrag <b>{calculated ? formatGermanDecimal(calculated.totals.payable) : "—"} {draft.currency}</b><small>Original: {extraction.fields.payable?.value || extraction.fields.taxInclusive?.value || "nicht gelesen"}</small></span>}
      </div>
    </section>

    {consistency && consistency.mismatches.length > 0 && <section className="validation-panel official" aria-live="polite">
      <h2>Angaben weichen von der Originalrechnung ab</h2>
      <p>Bitte die Widersprüche auflösen oder die Rechnung im Ursprungsprogramm korrigieren. Eine fertige E-Rechnung wird nicht erzeugt.</p>
      <ul>{consistency.mismatches.slice(0, 8).map((item) => <li key={item.path}><button type="button" className="issue-link" onClick={() => onFocusPath(item.path)}><strong>{item.label}:</strong> Original „{item.sourceValue}“, Ausgabe „{item.outputValue}“</button></li>)}</ul>
    </section>}
    {consistency && consistency.supplemented.length > 0 && consistency.mismatches.length === 0 && <section className="validation-panel" aria-live="polite">
      <h2>Diese Angaben stehen so nicht in der Originalrechnung</h2>
      <p>Bitte prüfen Sie, ob die Ausgabe die ausgestellte Rechnung korrekt wiedergibt. Inhaltliche Änderungen gehören ins Ursprungsprogramm.</p>
      <ul>{consistency.supplemented.slice(0, 8).map((item) => <li key={item.path}><button type="button" className="issue-link" onClick={() => onFocusPath(item.path)}><strong>{item.label}:</strong> {item.outputValue}</button></li>)}</ul>
    </section>}

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
    <div className="export-note"><strong>Wie möchten Sie die Rechnung weitergeben?</strong><span>Behörden benötigen meist die reine E-Rechnungsdatei. Für andere Kunden können Sie die Rechnung als PDF speichern. Die Original-PDF bleibt unverändert; für die PDF-Rechnung wird eine neue PDF/A-3-Datei erzeugt.</span></div>
    <label className="confirm-original">
      <input type="checkbox" checked={hybridConfirmed} disabled={Boolean(activeAction) || Boolean(consistency?.blocked)} onChange={(event) => onHybridConfirmedChange(event.target.checked)} />
      <span>Ich bestätige, dass die Angaben {originalDocumentLabel(draft)} korrekt wiedergeben. Änderungen am Rechnungsinhalt gehören ins Ursprungsprogramm.</span>
    </label>

    <footer>
      {validationPhase && activeAction && <p role="status">{validationPhase}</p>}
      <button className="secondary" disabled={Boolean(activeAction)} onClick={onSave}>
        {activeAction === "draft" ? "Entwurf wird gespeichert …" : "Entwurf speichern"}
      </button>
      <button className="secondary" disabled={Boolean(activeAction) || !validation.valid || Boolean(consistency?.blocked) || !hybridConfirmed} title={!hybridConfirmed ? "Bitte zuerst die Übereinstimmung mit der Originalrechnung bestätigen" : validation.valid ? "E-Rechnungsdatei für Behörden speichern" : "Bitte ergänzen Sie zuerst die angezeigten Angaben"} onClick={onCreateXRechnung}>
        {activeAction === "authority" ? "Wird für Behörden gespeichert …" : "Für Behörden speichern"}
      </button>
      <button className="primary" disabled={Boolean(activeAction) || !zugferdValidation.valid || Boolean(consistency?.blocked) || !hybridConfirmed} title={!hybridConfirmed ? "Bitte zuerst die Übereinstimmung mit der Originalrechnung bestätigen" : zugferdValidation.valid ? "Lesbare PDF-Rechnung speichern" : "Bitte ergänzen Sie zuerst die angezeigten Angaben"} onClick={onCreateZugferd}>
        {activeAction === "pdf" ? "PDF-Rechnung wird gespeichert …" : "Als PDF-Rechnung speichern"}
      </button>
    </footer>
  </aside>;
}
