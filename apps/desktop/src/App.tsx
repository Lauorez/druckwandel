import { useEffect, useMemo, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ExtractedFieldName, ExtractionResult } from "../../../src/extraction/types.js";
import {
  applyLearnedCorrections,
  learnCorrections,
  normalizeSourceValue,
  selectedSourceLine,
  sourceAssignmentError,
  upgradeLegacyFieldRules,
  type FieldSourceSelections,
  type LearnableFieldName,
} from "../../../src/learning/correction-memory.js";
import {
  activeLearningProfile,
  emptyLearningProfileStore,
  replaceActiveMemory,
  selectLearningProfile,
  type LearningProfileStore,
} from "../../../src/learning/profiles.js";
import { detectUnsupportedCases } from "../../../src/policy/unsupported-cases.js";
import {
  calculateReviewDraft,
  invoiceInputFromReview,
  reviewDraftFromExtraction,
  validateReviewDraft,
  type ReviewDraft,
} from "../../../src/review/draft.js";
import { PdfReview } from "./PdfReview.js";
import { assignSourceValue } from "./pdfSelection.js";
import { ReviewPanel, reviewFieldId, type ActionFeedback } from "./ReviewPanel.js";
import { loadLearningProfiles, saveLearningProfiles } from "./learningMemoryStore.js";
import { listenSettingsChanged, openSettingsWindow, type SettingsSection } from "./settingsWindow.js";
import { SettingsView } from "./SettingsView.js";
import { subscription } from "./subscription.js";
import { ArchiveView } from "./ArchiveView.js";
import { backupStatus, type BackupStatus } from "./backupStore.js";
import { DatevView } from "./DatevView.js";
import { cancelInvoiceValidation, saveAndArchiveInvoice, validatePreparedInvoice, type ArchiveMetadata, type OfficialCheckIssue } from "./archiveStore.js";
import { useWorkspace } from "./useWorkspace.js";
import { InboxView } from "./InboxView.js";
import { parseLegacyDraft, toBase64, type LegacyDraft, type WorkspaceSnapshot } from "./workspaceStore.js";
import { invoiceSnapshot, readInvoiceSnapshot } from "../../../src/export/invoice-snapshot.js";
import { compareInvoiceToSource, assertExportableConsistency } from "../../../src/engine/consistency.js";
import { HybridPdfError } from "../../../src/engine/hybrid-pdf-error.js";
import { germanFieldLabel } from "../../../src/engine/validation-report.js";

function safeFileStem(value: string): string {
  const stem = value.replace(/\.pdf$/i, "").replace(/[^\p{L}\p{N}._-]+/gu, "_").replace(/^\.+/, "");
  return stem.slice(0, 100) || "rechnung";
}

function downloadText(fileName: string, contents: string, mimeType: string): string {
  const blob = new Blob([contents], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
  return `Die Datei „${fileName}“ wird gespeichert.`;
}

function downloadBytes(fileName: string, contents: Uint8Array, mimeType: string): string {
  const blob = new Blob([contents as BlobPart], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
  return `Die Datei „${fileName}“ wird gespeichert.`;
}

async function saveDraft(fileName: string, contents: string): Promise<string> {
  if (isTauri()) return invoke<string>("write_review_draft", { fileName, contents });
  return downloadText(fileName, contents, "application/json");
}

export function App() {
  const [view, setView] = useState<"editor" | "archive" | "inbox" | "datev">(isTauri() ? "inbox" : "editor");
  const [datevBusy, setDatevBusy] = useState(false);
  const [archiveRevision, setArchiveRevision] = useState(0);
  const [fileName, setFileName] = useState("");
  const [pdfBytes, setPdfBytes] = useState<Uint8Array>();
  const [extraction, setExtraction] = useState<ExtractionResult>();
  const [sourceExtraction, setSourceExtraction] = useState<ExtractionResult>();
  const [draft, setDraft] = useState<ReviewDraft>();
  const [initialDraft, setInitialDraft] = useState<ReviewDraft>();
  const [learningProfiles, setLearningProfiles] = useState<LearningProfileStore>(emptyLearningProfileStore);
  const [selectedTokenIds, setSelectedTokenIds] = useState<string[]>([]);
  const [sourceSelections, setSourceSelections] = useState<FieldSourceSelections>({});
  const [pendingSourceFields, setPendingSourceFields] = useState<LearnableFieldName[]>([]);
  const [sourceTarget, setSourceTarget] = useState<{ field: LearnableFieldName; label: string }>();
  const [sourceValue, setSourceValue] = useState("");
  const [sourceError, setSourceError] = useState("");
  const sourcePickerRef = useRef<HTMLDivElement>(null);
  const [pageNumber, setPageNumber] = useState(1);
  const [analyzing, setAnalyzing] = useState(false);
  const importing = useRef(false);
  const [activeAction, setActiveAction] = useState<"draft" | "authority" | "pdf">();
  const [validationPhase, setValidationPhase] = useState<string>();
  const [officialIssues, setOfficialIssues] = useState<OfficialCheckIssue[]>([]);
  const [error, setError] = useState("");
  const [feedback, setFeedback] = useState<ActionFeedback>();
  const [completed, setCompleted] = useState(false);
  const [hybridConfirmed, setHybridConfirmed] = useState(false);
  const [legacyDraft, setLegacyDraft] = useState<LegacyDraft>();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsSection, setSettingsSection] = useState<SettingsSection>("profiles");
  const [backupReminder, setBackupReminder] = useState<BackupStatus>();
  const learningStore = useRef<LearningProfileStore>(emptyLearningProfileStore());
  const learningStorePromise = useRef<Promise<LearningProfileStore> | null>(null);
  const selectedTokenSet = useMemo(() => new Set(selectedTokenIds), [selectedTokenIds]);
  const unsupportedCases = useMemo(() => extraction ? detectUnsupportedCases(extraction) : [], [extraction]);
  const validation = useMemo(() => draft ? validateReviewDraft(draft, unsupportedCases) : undefined, [draft, unsupportedCases]);
  const zugferdValidation = useMemo(() => draft ? validateReviewDraft(draft, unsupportedCases, "zugferd") : undefined, [draft, unsupportedCases]);
  const calculated = useMemo(() => draft ? calculateReviewDraft(draft) : undefined, [draft]);
  const consistency = useMemo(() => extraction && draft ? compareInvoiceToSource(extraction, draft, calculated) : undefined, [extraction, draft, calculated]);
  const snapshot = useMemo<WorkspaceSnapshot | undefined>(() => sourceExtraction && extraction && draft && initialDraft ? {
    schemaVersion: 1, extractionVersion: "text-layout-v1", sourceExtraction, extraction, draft, initialDraft,
    sourceSelections, pendingSourceFields, completed, hybridConfirmed,
  } : undefined, [sourceExtraction,extraction,draft,initialDraft,sourceSelections,pendingSourceFields,completed,hybridConfirmed]);
  const work = useWorkspace({ snapshot, build: buildSnapshot, load: restoreDocument, clear: clearEditor, error: setError, isActionActive: Boolean(activeAction) || datevBusy, isImportActive: analyzing });
  const printStatus = work.printStatus;
  const processing = datevBusy || work.busy || analyzing || Boolean(activeAction);


  function archiveMetadata(invoice: NonNullable<typeof calculated>): ArchiveMetadata {
    return {
      invoiceNumber: invoice.invoiceNumber,
      issueDate: invoice.issueDate,
      sellerName: invoice.seller.name,
      buyerName: invoice.buyer.name,
      grossAmount: invoice.totals.payable,
      currency: invoice.currency,
      sourceFileName: fileName,
    };
  }

  function rememberStore(store: LearningProfileStore) {
    learningStore.current = store;
    learningStorePromise.current = Promise.resolve(store);
    setLearningProfiles(store);
  }

  function ensureLearningStore(): Promise<LearningProfileStore> {
    learningStorePromise.current ??= loadLearningProfiles().then((store) => {
      rememberStore(store);
      return store;
    }).catch((reason) => {
      learningStorePromise.current = null;
      throw reason;
    });
    return learningStorePromise.current;
  }

  useEffect(() => {
    void ensureLearningStore().catch(reason => setError(reason instanceof Error ? reason.message : String(reason)));
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    const refreshReminder = () => backupStatus().then((status) => {
      setBackupReminder(status.reminderDue ? status : undefined);
    }).catch(() => undefined);
    void refreshReminder();
    return subscription(listenSettingsChanged((scope) => {
      if (scope === "backup") void refreshReminder();
    }));
  }, []);

  useEffect(() => {
    let cancelled = false;
    const stop = subscription(listenSettingsChanged((scope) => {
      if (cancelled || (scope !== "learning" && scope !== "profiles")) return;
      learningStorePromise.current = null;
      void loadLearningProfiles().then((store) => { if (!cancelled) rememberStore(store); }).catch(console.error);
    }));
    return () => { cancelled = true; stop(); };
  }, []);

  async function openSettings(section: SettingsSection = "profiles") {
    if (work.busy || analyzing || activeAction) return;
    setSettingsSection(section);
    const mode = await openSettingsWindow(section);
    if (mode === "fallback") setSettingsOpen(true);
  }

  useEffect(() => {
    if (sourceTarget) sourcePickerRef.current?.focus();
  }, [sourceTarget]);

  useEffect(() => {
    if (!isTauri()) return;
    return subscription(listen<{ phase: string }>("invoice-validation-progress", (event) => {
      setValidationPhase(event.payload.phase);
    }));
  }, []);

  async function buildSnapshot(data: Uint8Array): Promise<WorkspaceSnapshot> {
    const { extractInvoicePdfInBrowser } = await import("../../../src/extraction/browser.js");
    const nextSourceExtraction = await extractInvoicePdfInBrowser(data);
    const storedStore = await ensureLearningStore();
    const storedMemory = activeLearningProfile(storedStore).memory;
    const memory = upgradeLegacyFieldRules(nextSourceExtraction, storedMemory);
    if (memory !== storedMemory) {
      try {
        const nextStore = replaceActiveMemory(storedStore, memory);
        await saveLearningProfiles(nextStore);
        rememberStore(nextStore);
      } catch {
        setFeedback({ kind: "error", message: "Die bisherigen Zuordnungen konnten nicht dauerhaft verbessert werden." });
      }
    }
    const remembered = applyLearnedCorrections(nextSourceExtraction,memory);
    const nextDraft = reviewDraftFromExtraction(remembered.extraction);
    return { schemaVersion: 1,extractionVersion: "text-layout-v1",sourceExtraction: nextSourceExtraction,
      extraction: remembered.extraction,draft: nextDraft,initialDraft: nextDraft,
      sourceSelections: {},pendingSourceFields: [],completed: false,hybridConfirmed: false };
  }

  function clearEditor() {
    setPdfBytes(undefined); setFileName("");
    setSourceExtraction(undefined); setExtraction(undefined);
    setDraft(undefined); setInitialDraft(undefined);
    setSourceSelections({}); setPendingSourceFields([]);
    setCompleted(false); setHybridConfirmed(false); setSelectedTokenIds([]); setSourceTarget(undefined);
    setSourceValue(""); setSourceError(""); setPageNumber(1); setFeedback(undefined); setOfficialIssues([]);
    document.title = "Druckwandel";
  }

  function restoreDocument(data: Uint8Array, name: string, next: WorkspaceSnapshot) {
    setPdfBytes(data); setFileName(name);
    setSourceExtraction(next.sourceExtraction); setExtraction(next.extraction);
    setDraft(next.draft); setInitialDraft(next.initialDraft);
    setSourceSelections(next.sourceSelections); setPendingSourceFields(next.pendingSourceFields);
    setCompleted(next.completed); setHybridConfirmed(next.hybridConfirmed); setSelectedTokenIds([]); setSourceTarget(undefined);
    setSourceValue(""); setSourceError(""); setPageNumber(1); setError(""); setFeedback(undefined); setOfficialIssues([]);
    setView("editor");
    document.title = `${name} – Druckwandel`;
  }

  async function openPdfFile(file: File, importedDraft?: LegacyDraft) {
    if (importing.current || work.busy || activeAction || datevBusy) { setError("Bitte warten Sie, bis die laufende Verarbeitung abgeschlossen ist."); return; }
    if (file.type !== "application/pdf" && !file.name.toLowerCase().endsWith(".pdf")) {
      setError("Bitte wählen Sie eine Rechnung im PDF-Format aus."); return;
    }
    if (file.size > 100 * 1024 * 1024) { setError("Bitte wählen Sie eine PDF-Datei bis 100 MB."); return; }
    importing.current = true;
    setAnalyzing(true);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      if (isTauri()) await work.importFile(file.name,bytes,importedDraft);
      else restoreDocument(bytes,file.name,await buildSnapshot(bytes));
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Die Rechnung konnte nicht gelesen werden."); }
    finally { importing.current = false; setAnalyzing(false); }
  }

  async function openLegacyFile(file: File) {
    try {
      await work.flush();
      setLegacyDraft(parseLegacyDraft(await file.text()));
      setView("inbox"); setError("");
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Der Entwurf konnte nicht gelesen werden."); }
  }

  function selectTokens(tokenIds: string[]) {
    if (!extraction || tokenIds.length === 0) return;
    setSelectedTokenIds(tokenIds);
    const token = extraction.pages.flatMap((page) => page.tokens).find((candidate) => tokenIds.includes(candidate.id));
    if (token) setPageNumber(token.page);
  }

  function selectSource(name: ExtractedFieldName) {
    setSourceTarget(undefined);
    const assigned = sourceSelections[name as LearnableFieldName];
    if (assigned) { selectTokens(assigned); return; }
    const field = extraction?.fields[name];
    if (field) selectTokens(field.sourceTokenIds);
    else setSelectedTokenIds([]);
  }

  function chooseSource(field: LearnableFieldName, label: string) {
    setSourceTarget({ field, label });
    setSelectedTokenIds([]);
    setSourceValue("");
    setSourceError("");
  }

  function markSource(tokenIds: string[]) {
    if (!extraction || !sourceTarget) return;
    const source = selectedSourceLine(extraction, tokenIds);
    setSelectedTokenIds(tokenIds);
    setSourceValue(source?.text ?? "");
    setSourceError(source ? "" : "Hier wurde kein Text gefunden. Bitte markieren Sie die geschriebene Angabe.");
  }

  function acceptSource() {
    if (!draft || !sourceTarget || selectedTokenIds.length === 0) return;
    const value = normalizeSourceValue(sourceTarget.field, sourceValue, { issueDate: draft.issueDate });
    const assignmentError = sourceAssignmentError(sourceTarget.field, sourceValue, { issueDate: draft.issueDate });
    if (!value || assignmentError) {
      setSourceError(assignmentError ?? "Bitte prüfen Sie den markierten Wert. Entfernen Sie zum Beispiel eine mitmarkierte Beschriftung.");
      return;
    }
    setCompleted(false);
    setHybridConfirmed(false);
    setOfficialIssues([]);
    setDraft(assignSourceValue(draft, sourceTarget.field, value));
    setSourceSelections((current) => ({ ...current, [sourceTarget.field]: [...selectedTokenIds] }));
    setPendingSourceFields((current) => [...new Set([...current, sourceTarget.field])]);
    setFeedback({ kind: "success", message: `„${sourceTarget.label}“ wurde übernommen. Speichern Sie den Entwurf oder die fertige Rechnung, um sich diese Stelle zu merken.` });
    setSourceTarget(undefined);
  }

  async function runAction(actionName: "draft" | "authority" | "pdf", action: () => Promise<string>) {
    if (activeAction || work.busy) return;
    setActiveAction(actionName);
    setFeedback(undefined);
    try {
      if (isTauri()) await work.flush();
      const message = await action();
      if (actionName !== "draft") setCompleted(true);
      setFeedback({ kind: "success", message });
    } catch (reason) {
      console.error(reason);
      const message = typeof reason === "string"
        ? reason
        : reason instanceof Error
          ? reason.message
          : reason && typeof reason === "object" && "message" in reason
            ? String((reason as { message: unknown }).message)
            : "Die Datei konnte nicht gespeichert werden. Bitte versuchen Sie es erneut.";
      setFeedback({ kind: "error", message: message || "Die Datei konnte nicht gespeichert werden. Bitte versuchen Sie es erneut." });
    } finally {
      setActiveAction(undefined);
      setValidationPhase(undefined);
    }
  }

  async function rememberCorrections(): Promise<string> {
    if (!sourceExtraction || !initialDraft || !draft) return "";
    try {
      const pendingSelections = Object.fromEntries(pendingSourceFields.map((field) => [field, sourceSelections[field]!])) as FieldSourceSelections;
      const store = await ensureLearningStore();
      const result = learnCorrections(activeLearningProfile(store).memory, sourceExtraction, initialDraft, draft, undefined, pendingSelections);
      if (result.changedFields.length === 0 && !result.tableChanged) return "";
      if (result.learnedFields.length > 0 || result.tableLearned) {
        const nextStore = replaceActiveMemory(store, result.memory);
        await saveLearningProfiles(nextStore);
        rememberStore(nextStore);
      }
      setInitialDraft(draft);
      setPendingSourceFields((current) => current.filter((field) => !result.learnedFields.includes(field)));
      const learnedMessage = result.learnedFields.length === 1
        ? " Eine Ergänzung wurde für ähnlich aufgebaute Rechnungen gemerkt."
        : result.learnedFields.length > 1
          ? ` ${result.learnedFields.length} Ergänzungen wurden für ähnlich aufgebaute Rechnungen gemerkt.`
          : "";
      const skippedMessage = result.skippedFields.length > 0
        ? ` ${result.skippedFields.length} ${result.skippedFields.length === 1 ? "Angabe konnte" : "Angaben konnten"} keiner eindeutigen Stelle zugeordnet werden. Wählen Sie beim jeweiligen Feld „Im PDF markieren“ und speichern Sie erneut.`
        : "";
      const learnedTableMessage = result.tableLearned
        ? " Die Anordnung der Leistungen und Artikel wurde ebenfalls gemerkt."
        : "";
      const skippedTableMessage = result.tableSkipped
        ? " Die Anordnung der Leistungen und Artikel konnte diesmal nicht eindeutig zugeordnet werden."
        : "";
      return `${learnedMessage}${learnedTableMessage}${skippedMessage}${skippedTableMessage}`;
    } catch (reason) {
      console.error(reason);
      return " Die Datei wurde gespeichert, aber die Ergänzungen konnten diesmal nicht gemerkt werden.";
    }
  }

  async function changeLearningProfile(profileId: string) {
    try {
      const next = selectLearningProfile(await ensureLearningStore(), profileId);
      await saveLearningProfiles(next);
      rememberStore(next);
    } catch (reason) {
      console.error(reason);
      setFeedback({ kind: "error", message: "Das Erkennungsprofil konnte nicht gewechselt werden." });
    }
  }

  function saveReview() {
    if (!draft || !extraction) return;
    void runAction("draft", async () => {
      if (isTauri()) return `Entwurf gespeichert. Sie können ihn im Posteingang jederzeit fortsetzen.${await rememberCorrections()}`;
      const contents = JSON.stringify({
        schemaVersion: 1,
        savedAt: new Date().toISOString(),
        source: fileName,
        draft,
        sourceSelections,
        extractedTotals: {
          lineNet: extraction.fields.lineNet?.value,
          taxTotal: extraction.fields.taxTotal?.value,
          taxInclusive: extraction.fields.taxInclusive?.value,
          payable: extraction.fields.payable?.value,
        },
        calculatedTotals: calculated?.totals,
        validation: { valid: validation?.valid ?? false, issues: validation?.issues ?? [] },
        zugferdValidation: { valid: zugferdValidation?.valid ?? false, issues: zugferdValidation?.issues ?? [] },
        extractionWarnings: extraction.warnings,
        unsupportedCases,
      }, null, 2);
      const result = await saveDraft(`${safeFileStem(fileName)}-entwurf.json`, contents);
      const message = isTauri() ? "Entwurf gespeichert. Sie finden ihn im Ordner „Dokumente > E-Rechnung Entwürfe“." : result;
      return `${message}${await rememberCorrections()}`;
    });
  }

  function createXRechnung() {
    if (!draft || !pdfBytes || !validation?.valid || !consistency) return;
    void runAction("authority", async () => {
      assertExportableConsistency(consistency, hybridConfirmed);
      const { EInvoiceEngine } = await import("../../../src/engine/e-invoice-engine.js");
      const engine = new EInvoiceEngine();
      const invoice = engine.calculate(invoiceInputFromReview(draft));
      const xml = engine.xrechnung(invoice);
      const file = `${safeFileStem(draft.invoiceNumber || fileName)}-e-rechnung.xml`;
      let message: string;
      if (isTauri()) {
        message = await finalizeNativeExport("xrechnung", file, xml, pdfBytes, invoice);
      } else {
        message = downloadText(file, xml, "application/xml");
      }
      return `${message}${await rememberCorrections()}`;
    });
  }

  function createZugferd() {
    if (!draft || !pdfBytes || !zugferdValidation?.valid || !consistency) return;
    void runAction("pdf", async () => {
      assertExportableConsistency(consistency, hybridConfirmed);
      const { EInvoiceEngine } = await import("../../../src/engine/e-invoice-engine.js");
      const engine = new EInvoiceEngine();
      const invoice = engine.calculate(invoiceInputFromReview(draft));
      let hybridPdf: Uint8Array;
      let xml: string;
      try {
        ({ pdf: hybridPdf, xml } = await engine.zugferdPackage(pdfBytes, invoice));
      } catch (error) {
        throw error instanceof HybridPdfError ? error : new Error(error instanceof Error ? error.message : String(error));
      }
      const file = `${safeFileStem(draft.invoiceNumber || fileName)}-e-rechnung.pdf`;
      let message: string;
      if (isTauri()) {
        message = await finalizeNativeExport("zugferd", file, xml, hybridPdf, invoice);
      } else {
        const result = downloadBytes(file, hybridPdf, "application/pdf");
        message = `${result} Ohne die lokale Prüfanwendung wird keine fertige, unabhängig geprüfte E-Rechnung erzeugt.`;
      }
      return `${message}${await rememberCorrections()}`;
    });
  }

  async function finalizeNativeExport(
    format: "xrechnung" | "zugferd",
    file: string,
    xml: string,
    pdf: Uint8Array,
    invoice: NonNullable<typeof calculated>,
  ): Promise<string> {
    const snapshot = invoiceSnapshot(invoice);
    readInvoiceSnapshot(snapshot, format, xml);
    const reference = await work.exportReference();
    setOfficialIssues([]);
    setValidationPhase("Unabhängige Prüfung");
    const check = await validatePreparedInvoice({
      format,
      xmlContents: xml,
      pdfContentsBase64: toBase64(pdf),
      documentId: reference.documentId,
      sourceRevision: reference.sourceRevision,
      snapshot,
    });
    if (!check.valid || !check.ticketId) {
      setOfficialIssues(check.issues);
      const first = check.issues[0];
      throw new Error(first ? `${germanFieldLabel(first.path)}: ${first.message}` : "Die unabhängige Prüfung hat die Rechnung nicht angenommen. Es wurde keine fertige Datei gespeichert.");
    }
    setValidationPhase("Geprüfte Datei wird archiviert");
    const result = await saveAndArchiveInvoice({
      format,
      outputFileName: file,
      pdfContentsBase64: toBase64(pdf),
      xmlContents: xml,
      metadata: archiveMetadata(invoice),
      evidence: { schemaVersion: 1, ...reference, snapshot, hybridConfirmed: true },
      ticketId: check.ticketId,
    });
    setArchiveRevision((revision) => revision + 1);
    const kind = format === "xrechnung" ? "E-Rechnungsdatei" : "PDF-Rechnung";
    return `${kind} geprüft, gespeichert und archiviert (Eintrag #${result.archiveEntry.sequence}, ${check.ruleVersion}).`;
  }

  function focusReviewPath(path: string) {
    const target = document.getElementById(reviewFieldId(path));
    const focusable = target?.querySelector<HTMLElement>("input, textarea, select, button");
    target?.scrollIntoView({ block: "center" });
    (focusable ?? target)?.focus?.();
  }

  return <main className={view === "archive" || view === "datev" ? "archive-main" : view === "inbox" ? "inbox-main" : "editor-main"}
    onDragOver={(event) => event.preventDefault()}
    onDrop={(event) => {
      event.preventDefault();
      if (datevBusy || work.busy || activeAction || analyzing || settingsOpen) return;
      const file = event.dataTransfer.files[0];
      if (file) {
        setView("editor");
        void openPdfFile(file);
      }
    }}
  >
    <header>
      <div><h1>Druckwandel</h1><p>{view === "datev" ? "Rechnungen für die Steuerkanzlei vorbereiten" : view === "archive" ? "Gespeicherte Rechnungen finden und prüfen" : view === "inbox" ? "Rechnungen und angefangene Entwürfe" : fileName || "Rechnung öffnen, Angaben prüfen und speichern"}</p></div>
      <div className="header-actions">
        {isTauri() && <nav className="app-navigation" aria-label="Bereich wählen">
          <button type="button" disabled={processing} aria-current={view === "inbox" ? "page" : undefined} className={view === "inbox" ? "active" : ""} onClick={() => setView("inbox")}>Posteingang ({work.page.total})</button>
          <button type="button" disabled={processing} aria-current={view === "editor" ? "page" : undefined} className={view === "editor" ? "active" : ""} onClick={() => setView("editor")}>Rechnung</button>
          <button type="button" disabled={processing} aria-current={view === "archive" || view === "datev" ? "page" : undefined} className={view === "archive" || view === "datev" ? "active" : ""} onClick={() => setView("archive")}>Archiv</button>
        </nav>}
        <label className="open-button">Rechnung öffnen<input disabled={processing} type="file" accept="application/pdf,.pdf" onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = "";
          if (file) {
            setView("editor");
            void openPdfFile(file);
          }
        }} /></label>
        <button type="button" className="secondary" disabled={processing} onClick={() => void openSettings()}>Einstellungen</button>
        {isTauri() && <label className="legacy-open">Entwurf öffnen<input type="file" accept=".json,application/json" disabled={processing} onChange={event => {
          const file = event.target.files?.[0]; event.target.value = "";
          if (file) void openLegacyFile(file);
        }} /></label>}
      </div>
    </header>
    {error && <div className="error-banner" role="alert">{error}<button onClick={() => setError("")} aria-label="Meldung schließen">×</button></div>}
    {backupReminder && <div className="datev-notice backup-reminder" role="status">
      <strong>{backupReminder.lastBackupAtMs ? "Sicherung ist älter als 14 Tage" : "Noch keine Sicherung"}</strong>
      <p>Bitte Archiv und Entwürfe auf ein anderes Laufwerk oder einen anderen Datenträger sichern. Eine Kopie auf derselben Festplatte schützt nicht vor einem Plattenausfall.</p>
      <button type="button" className="secondary" disabled={processing} onClick={() => void openSettings("backup")}>Sicherung öffnen</button>
    </div>}
    {legacyDraft && <section className="legacy-pair" role="region" aria-label="Originaldatei zuordnen">
      <strong>Original-PDF zum Entwurf auswählen</strong>
      <p>Der ältere Entwurf verweist auf „{legacyDraft.source}“, enthält aber keine PDF. Bitte wählen Sie bewusst die zugehörige Originaldatei. Alte Markierungen werden nicht ungeprüft übernommen.</p>
      <label className="open-button">Zugehörige PDF auswählen<input type="file" accept=".pdf,application/pdf" disabled={work.busy} onChange={event => {
        const file = event.target.files?.[0];
        if (file) { const selected = legacyDraft; setLegacyDraft(undefined); void openPdfFile(file,selected); }
      }} /></label><button className="secondary" onClick={() => setLegacyDraft(undefined)}>Abbrechen</button>
    </section>}
    {isTauri() && view !== "archive" && view !== "datev" && <div className={`work-status ${work.saveStatus === "error" ? "failed" : ""}`} role="status">
      <span>{work.busy || analyzing ? "Rechnung wird vorbereitet …" : work.activeId ? work.saveStatus === "saving" ? "Wird gespeichert …" : work.saveStatus === "error" ? "Nicht gespeichert" : "Gespeichert" : printStatus}</span>
      {work.activeId && <small>{printStatus}</small>}
      {work.saveStatus === "error" && <><span>{work.saveError}</span><button onClick={() => void work.flush().catch(reason => setError(String(reason)))}>Erneut speichern</button></>}
    </div>}
    {validationPhase && <div className="validation-progress overlay" role="status">
      <strong>{validationPhase} …</strong>
      <span>Die Datei wird intern vorbereitet und erst nach erfolgreicher Prüfung gespeichert.</span>
      <button type="button" className="secondary" onClick={() => { void cancelInvoiceValidation(); }}>Prüfung abbrechen</button>
    </div>}
    {view === "datev" ? <DatevView onBack={() => setView("archive")} onBusyChange={setDatevBusy} onSettings={() => void openSettings("datev")} /> : view === "archive" ? <ArchiveView refreshToken={archiveRevision} onDatev={() => setView("datev")} onSettings={() => void openSettings("archive")} /> : view === "inbox" ? <InboxView page={work.page} offset={work.offset} legacy={work.legacy}
      activeId={work.activeId} busy={processing} onOpen={doc => void work.open(doc)} onLegacy={candidate => void work.importLegacy(candidate)}
      onDelete={doc => void work.remove(doc)} onDismissLegacy={candidate => void work.dismissLegacy(candidate)} onPage={work.changePage} /> : <>
    {!extraction || !pdfBytes || !draft || !validation || !zugferdValidation ? <section className="drop-zone">
      <strong>{analyzing ? "Rechnung wird gelesen …" : "Rechnung hier ablegen"}</strong>
      <span>Oder klicken Sie oben auf „Rechnung öffnen“. Ihre Daten bleiben auf diesem Computer.</span>
      {isTauri() && <div className="printer-test">
        <b>Direkt aus Ihrem Rechnungsprogramm drucken</b>
        <span>Wählen Sie dort den Drucker „E-Rechnung“. Die Rechnung öffnet sich anschließend automatisch hier.</span>
        <button onClick={() => void invoke("open_print_inbox")}>Ordner mit gedruckten Rechnungen öffnen</button>
        <small>{printStatus}</small>
      </div>}
    </section> : <fieldset disabled={work.busy || analyzing || Boolean(activeAction)} className="workspace">
      <section className="document-panel">
        {sourceTarget && <div ref={sourcePickerRef} tabIndex={-1} className="source-picker" role="region" aria-label="Angabe aus der Rechnung übernehmen"
          onKeyDown={(event) => { if (event.key === "Escape") setSourceTarget(undefined); }}>
          <strong>{sourceTarget.label}: Stelle in der Rechnung wählen</strong>
          <p>Klicken Sie auf den passenden Text oder ziehen Sie mit gedrückter Maustaste einen Rahmen darum.</p>
          {selectedTokenIds.length > 0 && <label>Markierter Wert – bei Bedarf kürzen
            <input aria-label="Wert aus der Markierung" value={sourceValue} onChange={(event) => setSourceValue(event.target.value)} />
          </label>}
          {sourceError && <span role="alert">{sourceError}</span>}
          {extraction.pages[pageNumber - 1]?.tokens.length === 0 && <span>Diese Seite enthält keinen lesbaren Text. Bitte tragen Sie die Angabe von Hand ein.</span>}
          <div><button type="button" className="primary" disabled={selectedTokenIds.length === 0 || !sourceValue.trim()} onClick={acceptSource}>Übernehmen</button>
            <button type="button" className="secondary" onClick={() => setSourceTarget(undefined)}>Abbrechen</button></div>
        </div>}
        <div className="page-toolbar">
          <button aria-label="Vorherige Seite" title="Vorherige Seite" disabled={pageNumber <= 1} onClick={() => { setPageNumber((page) => page - 1); setSelectedTokenIds([]); setSourceValue(""); }}>‹</button>
          <span>Seite {pageNumber} von {extraction.pages.length}</span>
          <button aria-label="Nächste Seite" title="Nächste Seite" disabled={pageNumber >= extraction.pages.length} onClick={() => { setPageNumber((page) => page + 1); setSelectedTokenIds([]); setSourceValue(""); }}>›</button>
        </div>
        {extraction.pages[pageNumber - 1] && <PdfReview pdfBytes={pdfBytes} page={extraction.pages[pageNumber - 1]!} selectedTokenIds={selectedTokenSet} selectionEnabled={Boolean(sourceTarget)} onSelectSource={markSource} />}
      </section>
      <ReviewPanel
        extraction={extraction}
        draft={draft}
        validation={validation}
        zugferdValidation={zugferdValidation}
        calculated={calculated}
        consistency={consistency}
        hybridConfirmed={hybridConfirmed}
        onHybridConfirmedChange={setHybridConfirmed}
        unsupportedCases={unsupportedCases}
        activeAction={activeAction}
        validationPhase={validationPhase}
        officialIssues={officialIssues}
        feedback={feedback}
        onDismissFeedback={() => setFeedback(undefined)}
        learningProfiles={learningProfiles.profiles.map((profile) => ({ id: profile.id, name: profile.name }))}
        activeLearningProfileId={learningProfiles.activeProfileId}
        onSelectLearningProfile={(profileId) => void changeLearningProfile(profileId)}
        onDraftChange={next => { setCompleted(false); setHybridConfirmed(false); setOfficialIssues([]); setDraft(next); }}
        onSelectField={selectSource}
        onSelectTokens={selectTokens}
        sourceSelections={sourceSelections}
        onChooseSource={chooseSource}
        onFocusPath={focusReviewPath}
        onSave={saveReview}
        onCreateXRechnung={createXRechnung}
        onCreateZugferd={createZugferd}
      />
    </fieldset>}
    </>}
    {settingsOpen && <SettingsView overlay initialSection={settingsSection} onClose={() => setSettingsOpen(false)} />}
  </main>;
}
