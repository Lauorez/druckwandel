// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../apps/desktop/src/App.js";
import { canonicalJson } from "../src/export/invoice-snapshot.js";
import { workspaceNative } from "./helpers/workspace-native.js";
import { profile, source } from "./helpers/datev-source.js";

const native = vi.hoisted(() => ({
  invoke: vi.fn(),
  reminder: false,
  search: "",
}));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", async () => (await import("./helpers/tauri-events.js")).tauriEvents());
vi.mock("@tauri-apps/plugin-deep-link", () => ({ getCurrent: async () => [], onOpenUrl: async () => () => undefined }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({
  destroy: vi.fn(),
  onCloseRequested: async () => () => undefined,
}) }));
vi.mock("../src/extraction/browser.js", () => ({ extractInvoicePdfInBrowser: async () => ({ pages: [], fields: {} }) }));
vi.mock("../src/extraction/browser-pdf.js", () => ({
  loadBrowserPdfJs: async () => ({ getDocument: () => ({ promise: Promise.resolve({ getPage: async () => ({
    getViewport: () => ({ width: 600, height: 800 }), render: () => ({ promise: Promise.resolve() }),
  }) }), destroy: async () => undefined }) }),
}));

const archiveEntry = {
  id: "a1",
  sequence: 1,
  createdAtMs: 1,
  invoiceNumber: "RE-100",
  issueDate: "2026-08-01",
  sellerName: "Anbieter GmbH",
  buyerName: "Kunde AG",
  grossAmount: "119.00",
  currency: "EUR",
  format: "xrechnung" as const,
  signed: false,
  documentId: "document-1",
  independentlyChecked: true,
};

describe("release-candidate flows in the desktop UI", () => {
  let storage: ReturnType<typeof workspaceNative>;
  beforeEach(() => {
    storage = workspaceNative();
    native.reminder = false;
    native.search = "";
    native.invoke.mockReset();
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command.startsWith("workspace_")) return storage.invoke(command, args);
      if (command === "read_learning_memory") return null;
      if (command === "acknowledge_print_job" || command === "write_review_draft") return;
      if (command === "list_print_jobs") return [];
      if (command === "backup_status") {
        return { sameVolume: false, reminderDue: native.reminder, pendingRestore: false, defaultPath: "D:\\extern\\stand.erechnung" };
      }
      if (command === "diagnostic_report") {
        return { schemaVersion: 1, createdAtMs: 1, appVersion: "0.3.1", os: "windows", arch: "x64", components: [], archiveEntries: 1, signedEntries: 0, signingEnabled: false, workspaceDocuments: 0, datevProfilePresent: true, backupReminderDue: native.reminder, pendingRestore: false, notes: ["DATEV_OFFICIAL_CHECK_PENDING"] };
      }
      if (command === "list_archive_entries") {
        const query = (args?.query ?? {}) as { search?: string };
        native.search = String(query.search ?? "");
        const match = !native.search || archiveEntry.invoiceNumber.includes(native.search);
        return { entries: match ? [archiveEntry] : [], total: match ? 1 : 0 };
      }
      if (command === "get_archive_status") return { rootPath: "C:\\archiv", entryCount: 1, signedCount: 0, signingEnabled: false };
      if (command === "get_archive_entry") {
        return { ...archiveEntry, sourceFileName: "rechnung.pdf", pdfPath: "p", xmlPath: "x", pdfSha256: "1", xmlSha256: "2", previousChainHash: "", chainHash: "3" };
      }
      if (command === "verify_archive") {
        return { valid: true, checkedAtMs: 1, checkedAtDisplay: "heute", entryCount: 1, fileCount: 2, signedCount: 0, reportPath: "report.json", issues: [] };
      }
      if (command === "datev_get_profile") return canonicalJson(profile());
      if (command === "datev_save_profile") return;
      if (command === "datev_export_status") return [];
      if (command === "datev_list_exports") return { entries: [], total: 0 };
      if (command === "datev_check_duplicates") return ["a1"];
      if (command === "archive_datev_source") return { ...source(), archiveId: "a1" };
      throw new Error(`Unexpected native call: ${command}`);
    });
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("searches and verifies the archive, then blocks a DATEV repeat without reason", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /Archiv/ }));
    await screen.findByRole("heading", { name: "Rechnungsarchiv" });
    expect(screen.getByRole("button", { name: "Archiv" }).getAttribute("aria-current")).toBe("page");
    fireEvent.change(screen.getByLabelText("Suche"), { target: { value: "RE-999" } });
    await waitFor(() => expect(screen.getByText("Keine Rechnungen gefunden")).toBeTruthy());
    fireEvent.change(screen.getByLabelText("Suche"), { target: { value: "RE-100" } });
    await screen.findByRole("button", { name: /RE-100/ });
    fireEvent.click(screen.getByRole("button", { name: "Archiv prüfen" }));
    await screen.findByText("Archiv vollständig und unverändert");

    fireEvent.click(screen.getByRole("button", { name: "Für die Steuerkanzlei exportieren" }));
    fireEvent.click(await screen.findByLabelText("Rechnung RE-100 auswählen"));
    fireEvent.click(screen.getByRole("button", { name: "Vorschau erstellen" }));
    await screen.findByText(/Bereits ausgegebene Rechnungen oder mögliche Kopien/);
    const create = screen.getByRole("button", { name: "Paket für die Steuerkanzlei erstellen" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    fireEvent.click(screen.getByRole("checkbox", { name: /bewusst eine neue Ausgabe/ }));
    expect(create.disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Begründung für erneute Ausgabe"), { target: { value: "Kanzlei bittet um Wiederholung" } });
    await waitFor(() => expect(create.disabled).toBe(false));
  });

  it("shows a backup reminder and closes settings with Escape", async () => {
    native.reminder = true;
    render(<App />);
    await screen.findByText("Noch keine Sicherung");
    fireEvent.click(screen.getByRole("button", { name: "Sicherung öffnen" }));
    await screen.findByRole("heading", { name: "Einstellungen" });
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("heading", { name: "Einstellungen" })).toBeNull());
  });

  it("clears the backup reminder after a successful backup", async () => {
    native.reminder = true;
    render(<App />);
    await screen.findByText("Noch keine Sicherung");
    native.reminder = false;
    const { emit } = await import("@tauri-apps/api/event");
    await emit("app-settings-changed", { scope: "backup" });
    await waitFor(() => expect(screen.queryByText("Noch keine Sicherung")).toBeNull());
  });
});
