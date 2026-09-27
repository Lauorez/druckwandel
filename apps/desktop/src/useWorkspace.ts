import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { printJobIdFromDeepLink, type PrintJob } from "./printInbox.js";
import { DraftWriter, fromBase64, parseWorkspaceSnapshot, workStore, type InboxCandidate, type LegacyDraft, type WorkDocument, type WorkPage, type WorkspaceSnapshot } from "./workspaceStore.js";

interface Options {
  snapshot: WorkspaceSnapshot | undefined;
  build: (pdf: Uint8Array) => Promise<WorkspaceSnapshot>;
  load: (pdf: Uint8Array, name: string, snapshot: WorkspaceSnapshot) => void;
  clear: () => void;
  error: (message: string) => void;
  isActionActive: boolean;
  isImportActive: boolean;
}
const message = (e: unknown) => e instanceof Error ? e.message : String(e);

export function useWorkspace(options: Options) {
  const latest = useRef(options); latest.current = options;
  const writer = useRef<DraftWriter | undefined>(undefined);
  const alive = useRef(true);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const queuedCount = useRef(0);
  const refreshSequence = useRef(0);
  const [busy,setBusy] = useState(false);
  const [activeId,setActiveId] = useState<string>();
  const [saveStatus,setSaveStatus] = useState<"saving" | "saved" | "error">("saved");
  const [saveError,setSaveError] = useState("");
  const [page,setPage] = useState<WorkPage>({ entries: [],total: 0,lastOpenedId: null });
  const [offset,setOffset] = useState(0);
  const offsetRef = useRef(0);
  const [legacy,setLegacy] = useState<InboxCandidate[]>([]);
  const [printStatus,setPrintStatus] = useState("Die Druckfunktion wird vorbereitet …");
  const report = (e: unknown) => { if (alive.current) latest.current.error(message(e)); };

  function enqueue<T>(action: () => Promise<T>): Promise<T> {
    queuedCount.current += 1;
    const next = queue.current.catch(() => undefined).then(action).finally(() => { queuedCount.current -= 1; });
    queue.current = next.catch(() => undefined);
    return next;
  }
  async function refresh(nextOffset = offsetRef.current) {
    const sequence = ++refreshSequence.current;
    const result = await workStore.list(nextOffset);
    if (alive.current && sequence === refreshSequence.current && nextOffset === offsetRef.current) setPage(result);
    return result;
  }
  async function flush() {
    if (writer.current && latest.current.snapshot) writer.current.update(latest.current.snapshot);
    await writer.current?.flush();
  }
  async function activate(doc: WorkDocument, legacyDraft?: LegacyDraft) {
    if (!alive.current) return;
    if (latest.current.isActionActive) throw new Error("Bitte warten Sie, bis das Speichern abgeschlossen ist.");
    setBusy(true);
    try {
      await flush();
      const detail = await workStore.read(doc.id);
      const pdf = fromBase64(detail.pdfBase64);
      let snapshot = detail.snapshot ? parseWorkspaceSnapshot(detail.snapshot) : await latest.current.build(pdf);
      if (legacyDraft) {
        // Old JSON has no original hash or extraction version. Preserve values,
        // but never attach old token IDs to a newly reconstructed text layer.
        const draft = { ...legacyDraft.draft, lines: legacyDraft.draft.lines.map(l => ({ ...l,sourceTokenIds: [] })) };
        snapshot = { ...snapshot,draft,initialDraft: draft,sourceSelections: {},pendingSourceFields: [],completed: false,hybridConfirmed: false };
      }
      let currentDoc = detail.document;
      if (!detail.snapshot || legacyDraft) currentDoc = await workStore.save(doc.id,currentDoc.revision,JSON.stringify(snapshot));
      await workStore.activate(doc.id);
      if (!alive.current) return;
      writer.current?.dispose();
      const nextWriter = new DraftWriter(currentDoc,JSON.stringify(snapshot),workStore.save,(status,error) => {
        if (!alive.current || writer.current !== nextWriter) return;
        setSaveStatus(status); setSaveError(error ?? "");
        if (status === "saved") void refresh().catch(report);
      });
      writer.current = nextWriter;
      latest.current = { ...latest.current, snapshot };
      setActiveId(doc.id); setSaveStatus("saved"); setSaveError("");
      latest.current.load(pdf,doc.name,snapshot);
      if (doc.jobId) await invoke("acknowledge_print_job", { jobId: doc.jobId,status: "opened",message: null })
        .catch(() => setPrintStatus("Rechnung geöffnet. Die Bestätigung an den Druckeingang konnte nicht geschrieben werden."));
      await refresh();
    } catch (e) {
      // Failed flush belongs to the current document, not the new invoice.
      if (!writer.current?.dirty) {
        await workStore.error(doc.id,message(e)).catch(() => undefined);
        if (doc.jobId) await invoke("acknowledge_print_job", { jobId: doc.jobId,status: "failed",message: message(e) }).catch(() => undefined);
      }
      throw e;
    } finally { if (alive.current) { setBusy(false); void refresh().catch(report); } }
  }
  async function receive(job: PrintJob) {
    const doc = await workStore.importPrint(job.path);
    if (!writer.current && !latest.current.isActionActive) await activate(doc);
    else {
      await refresh();
      if (alive.current) setPrintStatus(`Im Posteingang bereit: ${doc.name}`);
    }
  }

  useEffect(() => {
    if (!isTauri()) return;
    if (options.snapshot) writer.current?.update(options.snapshot);
  }, [options.snapshot]);

  useEffect(() => {
    if (!isTauri()) { setPrintStatus("Der automatische Druck ist nur in der installierten Anwendung verfügbar."); return; }
    alive.current = true;
    let cancelled = false;
    let stopLinks: (() => void) | undefined;
    let stopClose: (() => void) | undefined;
    let timer: ReturnType<typeof setInterval> | undefined;
    let polling = false;
    async function links(urls: readonly string[]) {
      for (const url of urls) {
        const id = printJobIdFromDeepLink(url);
        if (id && !cancelled) await receive(await invoke<PrintJob>("get_print_job",{ jobId: id }));
      }
    }
    async function poll() {
      if (polling || cancelled) return;
      polling = true;
      try {
        const candidates = await workStore.scan();
        if (cancelled) return;
        setLegacy(candidates.filter(c => c.legacy));
        for (const candidate of [...candidates].reverse().filter(c => !c.legacy)) {
          if (cancelled) break;
          try { await receive(candidate.job); } catch (e) { report(e); }
        }
      } finally { polling = false; }
    }
    void enqueue(async () => {
      if (cancelled) return;
      stopLinks = await onOpenUrl(urls => { void enqueue(() => links(urls)).catch(report); });
      if (cancelled) { stopLinks(); return; }
      const startUrls = await getCurrent();
      const initialPage = await refresh();
      // Establish durable history before handling the explicit startup handoff.
      const initialCandidates = await workStore.scan().catch(e => { report(e); return []; });
      if (cancelled) return;
      setLegacy(initialCandidates.filter(c => c.legacy));
      if (startUrls?.length) await links(startUrls).catch(report);
      if (!writer.current && initialPage.lastOpenedId) {
        const detail = await workStore.read(initialPage.lastOpenedId).catch(e => { report(e); return undefined; });
        if (detail) await activate(detail.document).catch(report);
      }
      await poll().catch(report);
      if (!cancelled) {
        setPrintStatus("Bereit – neue Rechnungen erscheinen im Posteingang");
        timer = setInterval(() => { if (!polling) void enqueue(poll).catch(report); },1500);
      }
    }).catch(report);
    // Close handler is independent of inbox availability: saving must still work
    // when Documents/OneDrive or the printer is temporarily unavailable.
    void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      const win = getCurrentWindow();
      stopClose = await win.onCloseRequested(async event => {
        event.preventDefault();
        if (latest.current.isActionActive || latest.current.isImportActive || queuedCount.current > 0) { report("Bitte warten Sie, bis die laufende Verarbeitung abgeschlossen ist."); return; }
        try { await flush(); await win.destroy(); } catch (e) { report(`Das Fenster bleibt geöffnet, damit keine Eingaben verloren gehen. ${message(e)}`); }
      });
      if (cancelled) stopClose();
    }).catch(report);
    return () => {
      cancelled = true; alive.current = false;
      clearInterval(timer); stopLinks?.(); stopClose?.(); writer.current?.dispose();
    };
  }, []);

  return {
    busy,activeId,page,offset,legacy,saveStatus,saveError,printStatus,flush,
    exportReference: async () => {
      await flush();
      if (!writer.current) throw new Error("Bitte zuerst eine Rechnung öffnen und speichern.");
      return { documentId: writer.current.document.id, sourceRevision: writer.current.document.revision };
    },
    open: (doc: WorkDocument) => enqueue(() => activate(doc)).catch(report),
    importFile: (name: string, pdf: Uint8Array, draft?: LegacyDraft) => enqueue(async () => {
      await flush();
      const doc = await workStore.import(name,pdf);
      await activate(doc,draft);
    }).catch(report),
    importLegacy: (candidate: InboxCandidate) => enqueue(async () => {
      const doc = await workStore.importPrint(candidate.job.path);
      await activate(doc);
      setLegacy(current => current.filter(c => c.key !== candidate.key));
    }).catch(report),
    remove: (doc: WorkDocument) => enqueue(async () => {
      await flush();
      const clearing = writer.current?.document.id === doc.id;
      await workStore.remove(doc.id);
      if (clearing) {
        writer.current?.dispose();
        writer.current = undefined;
        setActiveId(undefined);
        latest.current.clear();
      }
      const result = await refresh();
      if (result.entries.length === 0 && offsetRef.current > 0) {
        const next = Math.max(0, offsetRef.current - 100);
        offsetRef.current = next;
        setOffset(next);
        await refresh(next);
      }
    }).catch(report),
    dismissLegacy: (candidate: InboxCandidate) => enqueue(async () => {
      await workStore.dismiss(candidate.key);
      setLegacy(current => current.filter(c => c.key !== candidate.key));
    }).catch(report),
    changePage: (next: number) => { offsetRef.current = next; setOffset(next); void refresh(next).catch(report); },
  };
}
