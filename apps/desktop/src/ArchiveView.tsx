import { useEffect, useState } from "react";
import { formatGermanDecimal } from "../../../src/domain/localized-decimal.js";
import {
  getArchiveEntry,
  getArchiveStatus,
  listArchiveEntries,
  openArchiveEntryFile,
  openArchiveFolder,
  openArchiveReport,
  verifyArchive,
  type ArchiveEntryDetail,
  type ArchiveEntrySummary,
  type ArchiveFormat,
  type ArchiveSignatureFilter,
  type ArchiveStatus,
  type ArchiveVerificationReport,
} from "./archiveStore.js";
import { listenSettingsChanged } from "./settingsWindow.js";
import { subscription } from "./subscription.js";

interface ArchiveViewProps {
  refreshToken: number;
  onDatev?: () => void;
  onSettings?: () => void;
}

const PAGE_SIZE = 100;

function formatDate(value: string): string {
  const [year, month, day] = value.split("-");
  return year && month && day ? `${day}.${month}.${year}` : value;
}

function formatTimestamp(value: number): string {
  return new Intl.DateTimeFormat("de-DE", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}

function formatAmount(value: string, currency: string): string {
  try {
    return `${formatGermanDecimal(value)} ${currency}`;
  } catch {
    return `${value} ${currency}`;
  }
}

function formatLabel(format: ArchiveFormat): string {
  return format === "xrechnung" ? "Behörden-Datei" : "PDF-Rechnung";
}

export function ArchiveView({ refreshToken, onDatev, onSettings }: ArchiveViewProps) {
  const [entries, setEntries] = useState<ArchiveEntrySummary[]>([]);
  const [filteredTotal, setFilteredTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [selectedId, setSelectedId] = useState<string>();
  const [detail, setDetail] = useState<ArchiveEntryDetail>();
  const [detailLoading, setDetailLoading] = useState(false);
  const [status, setStatus] = useState<ArchiveStatus>();
  const [search, setSearch] = useState("");
  const [format, setFormat] = useState<"" | ArchiveFormat>("");
  const [signature, setSignature] = useState<ArchiveSignatureFilter>("");
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [verification, setVerification] = useState<ArchiveVerificationReport>();
  const [verifying, setVerifying] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(() => {
      setLoading(true);
      setError("");
      void listArchiveEntries({ search, format, signature, dateFrom, dateTo, limit: PAGE_SIZE, offset: page * PAGE_SIZE }).then((result) => {
        if (cancelled) return;
        if (page > 0 && page * PAGE_SIZE >= result.total) {
          setPage(Math.max(0, Math.ceil(result.total / PAGE_SIZE) - 1));
          return;
        }
        const nextEntries = result.entries;
        setEntries(nextEntries);
        setFilteredTotal(result.total);
        setSelectedId((current) => nextEntries.some((entry) => entry.id === current) ? current : nextEntries[0]?.id);
      }).catch((reason) => {
        console.error(reason);
        if (!cancelled) setError("Das Archiv konnte nicht geladen werden.");
      }).finally(() => {
        if (!cancelled) setLoading(false);
      });
    }, 180);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [search, format, signature, dateFrom, dateTo, page, refreshToken]);

  useEffect(() => {
    let cancelled = false;
    const load = () => {
      void getArchiveStatus().then((nextStatus) => {
        if (!cancelled) setStatus(nextStatus);
      }).catch((reason) => {
        console.error(reason);
        if (!cancelled) setError("Der Archivstatus konnte nicht geladen werden.");
      });
    };
    load();
    const stop = subscription(listenSettingsChanged((scope) => {
      if (scope === "archive") load();
    }));
    return () => {
      cancelled = true;
      stop();
    };
  }, [refreshToken]);

  useEffect(() => {
    setPage(0);
  }, [search, format, signature, dateFrom, dateTo]);

  useEffect(() => {
    let cancelled = false;
    setDetail(undefined);
    setDetailLoading(Boolean(selectedId));
    if (!selectedId) {
      return;
    }
    void getArchiveEntry(selectedId).then((entry) => {
      if (!cancelled) setDetail(entry);
    }).catch((reason) => {
      console.error(reason);
      if (!cancelled) setError("Der Archiveintrag konnte nicht geöffnet werden.");
    }).finally(() => {
      if (!cancelled) setDetailLoading(false);
    });
    return () => { cancelled = true; };
  }, [selectedId]);

  async function checkArchive() {
    setVerifying(true);
    setVerification(undefined);
    setError("");
    try {
      setVerification(await verifyArchive());
    } catch (reason) {
      console.error(reason);
      setError("Das Archiv konnte nicht vollständig geprüft werden.");
    } finally {
      setVerifying(false);
    }
  }

  async function openEntryFile(kind: "pdf" | "xml" | "report") {
    if (!detail || detail.id !== selectedId) return;
    try {
      await openArchiveEntryFile(detail.id, kind);
    } catch (reason) {
      console.error(reason);
      setError("Die archivierte Datei konnte nicht geöffnet werden.");
    }
  }

  async function openFolder() {
    try {
      await openArchiveFolder();
    } catch (reason) {
      console.error(reason);
      setError("Der Archivordner konnte nicht geöffnet werden.");
    }
  }

  async function openReport() {
    if (!verification) return;
    try {
      await openArchiveReport(verification.reportPath);
    } catch (reason) {
      console.error(reason);
      setError("Der Prüfbericht konnte nicht geöffnet werden.");
    }
  }

  return <section className="archive-view">
    <div className="archive-heading">
      <div>
        <h2>Rechnungsarchiv</h2>
        <p>{status?.entryCount ?? 0} {(status?.entryCount ?? 0) === 1 ? "Rechnung" : "Rechnungen"} lokal abgelegt</p>
      </div>
      <div className="archive-heading-actions">
        {onDatev && <button type="button" className="secondary" onClick={onDatev}>Für die Steuerkanzlei exportieren</button>}
        <button type="button" className="secondary" onClick={() => void openFolder()}>Archivordner öffnen</button>
        <button type="button" className="primary" disabled={verifying} onClick={() => void checkArchive()}>
          {verifying ? "Archiv wird geprüft …" : "Archiv prüfen"}
        </button>
      </div>
    </div>

    {error && <div className="error-banner archive-error" role="alert">{error}</div>}

    <div className="archive-protection">
      <div>
        <strong>Zusätzlicher Schutz für neue Rechnungen</strong>
        <span>Neue Einträge können mit einem nur auf diesem Computer gespeicherten Schlüssel bestätigt werden. Der Schalter liegt in den Einstellungen. Bereits archivierte Rechnungen bleiben unverändert.</span>
      </div>
      <div className="archive-protection-actions">
        <span className="switch-label">{status?.signingEnabled ? "Eingeschaltet" : "Ausgeschaltet"}</span>
        {onSettings && <button type="button" className="secondary" onClick={onSettings}>Einstellungen öffnen</button>}
      </div>
    </div>

    {verification && <section className={`verification-result ${verification.valid ? "valid" : "invalid"}`} aria-live="polite">
      <div>
        <strong>{verification.valid ? "Archiv vollständig und unverändert" : "Im Archiv wurden Abweichungen gefunden"}</strong>
        <span>{verification.entryCount} Einträge und {verification.fileCount} Dateien geprüft. {verification.signedCount} Einträge haben den zusätzlichen Schutz.</span>
      </div>
      <button className="secondary" onClick={() => void openReport()}>Prüfbericht öffnen</button>
      {verification.issues.length > 0 && <ul>
        {verification.issues.slice(0, 8).map((issue, index) => <li key={`${issue.sequence ?? "archive"}-${index}`}>
          {issue.invoiceNumber ? `${issue.invoiceNumber}: ` : ""}{issue.message}
        </li>)}
      </ul>}
    </section>}

    <div className="archive-filters" aria-label="Archiv durchsuchen und filtern">
      <label className="archive-search"><span>Suche</span><input type="search" value={search} placeholder="Rechnungsnummer, Firma oder Dateiname" onChange={(event) => setSearch(event.target.value)} /></label>
      <label><span>Art</span><select value={format} onChange={(event) => setFormat(event.target.value as "" | ArchiveFormat)}>
        <option value="">Alle Rechnungen</option>
        <option value="xrechnung">Behörden-Datei</option>
        <option value="zugferd">PDF-Rechnung</option>
      </select></label>
      <label><span>Schutz</span><select value={signature} onChange={(event) => setSignature(event.target.value as ArchiveSignatureFilter)}>
        <option value="">Alle</option>
        <option value="signed">Zusätzlich geschützt</option>
        <option value="unsigned">Ohne zusätzlichen Schutz</option>
      </select></label>
      <label><span>Von</span><input type="date" value={dateFrom} onChange={(event) => setDateFrom(event.target.value)} /></label>
      <label><span>Bis</span><input type="date" value={dateTo} onChange={(event) => setDateTo(event.target.value)} /></label>
    </div>

    <div className="archive-content">
      <div className="archive-list" aria-label="Archivierte Rechnungen">
        {loading && <div className="archive-empty">Archiv wird geladen …</div>}
        {!loading && entries.length === 0 && <div className="archive-empty">
          <strong>Keine Rechnungen gefunden</strong>
          <span>{status?.entryCount ? "Ändern Sie die Suche oder die Filter." : "Sobald Sie eine E-Rechnung speichern, erscheint sie automatisch hier."}</span>
        </div>}
        {entries.map((entry) => <button
          type="button"
          className={`archive-list-item${entry.id === selectedId ? " selected" : ""}`}
          aria-current={entry.id === selectedId ? "true" : undefined}
          key={entry.id}
          onClick={() => setSelectedId(entry.id)}
        >
          <span className="archive-list-top"><strong>{entry.invoiceNumber}</strong><small>#{entry.sequence}</small></span>
          <span>{entry.buyerName}</span>
          <span className="archive-list-bottom"><small>{formatDate(entry.issueDate)} · {formatLabel(entry.format)}</small><b>{formatAmount(entry.grossAmount, entry.currency)}</b></span>
          {entry.independentlyChecked && <em>Unabhängig geprüft</em>}
          {entry.signed && <em>Zusätzlich geschützt</em>}
        </button>)}
        {!loading && filteredTotal > 0 && <nav className="archive-pagination" aria-label="Seiten im Archiv">
          <button className="secondary" disabled={page === 0} onClick={() => setPage((value) => Math.max(0, value - 1))}>Zurück</button>
          <span>Seite {page + 1} von {Math.ceil(filteredTotal / PAGE_SIZE)}</span>
          <button className="secondary" disabled={(page + 1) * PAGE_SIZE >= filteredTotal} onClick={() => setPage((value) => value + 1)}>Weiter</button>
        </nav>}
      </div>

      <article className="archive-detail" aria-busy={detailLoading}>
        {!detail || detail.id !== selectedId ? <div className="archive-empty" role="status"><strong>{detailLoading ? "Rechnung wird geladen …" : selectedId ? "Rechnung konnte nicht geladen werden" : "Rechnung auswählen"}</strong><span>{selectedId ? "Die Dateien werden erst nach erfolgreichem Laden bereitgestellt." : "Hier sehen Sie anschließend alle archivierten Angaben und Dateien."}</span></div> : <>
          <div className="archive-detail-title">
            <div><small>Archiveintrag #{detail.sequence}</small><h3>{detail.invoiceNumber}</h3><span>{formatLabel(detail.format)}</span></div>
            <span className={`archive-badge ${detail.independentlyChecked ? "signed" : ""}`}>{detail.independentlyChecked ? `Unabhängig geprüft${detail.ruleVersion ? ` (${detail.ruleVersion})` : ""}` : "Ohne unabhängige Prüfung archiviert"}</span>
          </div>
          <dl className="archive-metadata">
            <div><dt>Rechnungsdatum</dt><dd>{formatDate(detail.issueDate)}</dd></div>
            <div><dt>Rechnungsbetrag</dt><dd>{formatAmount(detail.grossAmount, detail.currency)}</dd></div>
            <div><dt>Absender</dt><dd>{detail.sellerName}</dd></div>
            <div><dt>Empfänger</dt><dd>{detail.buyerName}</dd></div>
            <div><dt>Archiviert am</dt><dd>{formatTimestamp(detail.createdAtMs)}</dd></div>
            <div><dt>Ursprüngliche Datei</dt><dd>{detail.sourceFileName || "—"}</dd></div>
          </dl>
          <div className="archive-file-actions">
            <button className="primary" onClick={() => void openEntryFile("pdf")}>PDF öffnen</button>
            <button className="secondary" onClick={() => void openEntryFile("xml")}>Rechnungsdaten öffnen</button>
            {detail.independentlyChecked && <button className="secondary" onClick={() => void openEntryFile("report")}>Prüfbericht öffnen</button>}
          </div>
        </>}
      </article>
    </div>
  </section>;
}
