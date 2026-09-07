import type { WorkDetail, WorkDocument, InboxCandidate } from "../../apps/desktop/src/workspaceStore.js";

/** Native storage contract double; real file/SQLite behavior is covered in Rust. */
export function workspaceNative() {
  const docs = new Map<string,WorkDetail>();
  let lastOpenedId: string | null = null;
  const state = { docs,candidates: [] as InboxCandidate[],failSave: false };
  return { ...state, async invoke(command: string,args: Record<string, unknown> = {}): Promise<unknown> {
    switch (command) {
      case "workspace_list": return { entries: [...docs.values()].map(v => v.document),total: docs.size,lastOpenedId };
      case "workspace_scan_inbox": return this.candidates.filter(c => ![...docs.values()].some(v => v.document.sourceKey === c.key));
      case "workspace_import":
      case "workspace_import_print": {
        const candidate = this.candidates.find(c => c.job.path === args.path);
        const existing = candidate && [...docs.values()].find(v => v.document.sourceKey === candidate.key);
        if (existing) return existing.document;
        const document: WorkDocument = { id: `doc-${docs.size+1}`,name: String(candidate?.job.name ?? args.name),
          sourceKey: candidate?.key ?? null,jobId: candidate?.job.jobId ?? null,originalSha256: "test",revision: 0,status: "new",updatedAtMs: Date.now(),error: null };
        docs.set(document.id,{ document,pdfBase64: String(args.pdfBase64 ?? "JVBERi0="),snapshot: null });
        return document;
      }
      case "workspace_read": return docs.get(String(args.id));
      case "workspace_save": {
        if (this.failSave) throw new Error("Test: Speicherung nicht möglich");
        const row = docs.get(String(args.id))!;
        if (row.document.revision !== args.expectedRevision) throw new Error("Revision conflict");
        row.snapshot = String(args.contents);
        row.document = { ...row.document,revision: row.document.revision+1,status: JSON.parse(row.snapshot).completed ? "done" : "draft" };
        return row.document;
      }
      case "workspace_activate": lastOpenedId = String(args.id); return;
      case "workspace_error": { const row = docs.get(String(args.id)); if (row) row.document.error = String(args.message); return; }
      default: throw new Error(`Unexpected workspace call: ${command}`);
    }
  } };
}
