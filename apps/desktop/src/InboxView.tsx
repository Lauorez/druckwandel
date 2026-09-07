import type { InboxCandidate, WorkDocument, WorkPage } from "./workspaceStore.js";

const labels = { new: "Noch zu prüfen",draft: "In Bearbeitung",done: "Fertig",error: "Bitte prüfen" };
interface Props {
  page: WorkPage; offset: number; legacy: InboxCandidate[]; busy: boolean; activeId?: string;
  onOpen: (doc: WorkDocument) => void; onLegacy: (candidate: InboxCandidate) => void; onPage: (offset: number) => void;
}
export function InboxView({ page,offset,legacy,busy,activeId,onOpen,onLegacy,onPage }: Props) {
  return <section className="inbox-view">
    <div className="inbox-heading"><h2>Posteingang und Entwürfe</h2><p>Ihre Rechnungen bleiben hier erhalten. Neue Druckaufträge unterbrechen keine laufende Bearbeitung.</p></div>
    <div className="inbox-scroll">
      {page.entries.length === 0 && <p>Noch keine Rechnungen vorhanden. Öffnen Sie eine PDF oder drucken Sie auf „E-Rechnung“.</p>}
      {page.entries.map(doc => <button type="button" className={`inbox-entry ${doc.id === activeId ? "selected" : ""}`} key={doc.id} disabled={busy} onClick={() => onOpen(doc)}>
        <span><strong>{doc.name}</strong><small>{new Date(doc.updatedAtMs).toLocaleString("de-DE")}</small>{doc.error && <em>{doc.error}</em>}</span>
        <span>{labels[doc.status]}{doc.id === activeId ? " · Geöffnet" : ""}</span>
      </button>)}
      {legacy.length > 0 && <details className="legacy-inbox"><summary>{legacy.length} ältere Dateien im Druckeingang</summary>
        <p>Diese Dateien waren bereits vor der Einrichtung des Posteingangs vorhanden. Sie werden nur auf Wunsch übernommen.</p>
        {legacy.map(c => <button type="button" className="inbox-entry" key={c.key} disabled={busy} onClick={() => onLegacy(c)}><strong>{c.job.name}</strong><span>Übernehmen</span></button>)}
      </details>}
    </div>
    <div className="inbox-pagination"><button className="secondary" disabled={busy || offset === 0} onClick={() => onPage(Math.max(0,offset-100))}>Zurück</button>
      <span>{page.total === 0 ? "0 Rechnungen" : `${offset+1}–${Math.min(offset+100,page.total)} von ${page.total} Rechnungen`}</span>
      <button className="secondary" disabled={busy || offset+100 >= page.total} onClick={() => onPage(offset+100)}>Weiter</button></div>
  </section>;
}
