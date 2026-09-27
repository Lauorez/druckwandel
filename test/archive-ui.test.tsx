// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ArchiveView } from "../apps/desktop/src/ArchiveView.js";
import type { ArchiveEntryDetail } from "../apps/desktop/src/archiveStore.js";

const native = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke, isTauri: () => true }));
vi.mock("@tauri-apps/api/event", async () => (await import("./helpers/tauri-events.js")).tauriEvents());
const entry = (id: string): ArchiveEntryDetail => ({
  id, sequence: Number(id), createdAtMs: 1, invoiceNumber: `RE-${id}`, issueDate: "2026-09-01",
  sellerName: "Absender", buyerName: "Empfänger", grossAmount: "119.00", currency: "EUR",
  format: "xrechnung", signed: false, sourceFileName: `${id}.pdf`, pdfPath: "p", xmlPath: "x",
  pdfSha256: "p", xmlSha256: "x", previousChainHash: "", chainHash: "h",
});

describe("archive selection", () => {
  beforeEach(() => {
    native.invoke.mockReset();
    native.invoke.mockImplementation(async (command: string, args?: { id: string }) => {
      if (command === "list_archive_entries") return { entries: [entry("1"), entry("2")], total: 2 };
      if (command === "get_archive_entry") return entry(args!.id);
      if (command === "get_archive_status") return { entryCount: 2, signingEnabled: false };
    });
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("hides the previous invoice and its file actions while the next one loads", async () => {
    render(<ArchiveView refreshToken={0} />);
    await screen.findByRole("heading", { name: "RE-1" });
    let resolve!: (value: ArchiveEntryDetail) => void;
    native.invoke.mockImplementationOnce(() => new Promise<ArchiveEntryDetail>(r => { resolve = r; }));
    fireEvent.click(screen.getByRole("button", { name: /RE-2/ }));
    expect(screen.queryByRole("heading", { name: "RE-1" })).toBeNull();
    expect(screen.queryByRole("button", { name: "PDF öffnen" })).toBeNull();
    await act(async () => resolve(entry("2")));
    await screen.findByRole("heading", { name: "RE-2" });
    fireEvent.click(screen.getByRole("button", { name: "PDF öffnen" }));
    await waitFor(() => expect(native.invoke).toHaveBeenCalledWith("open_archive_entry_file", { id: "2", fileKind: "pdf" }));
  });

  it("does not leave another invoice open after a detail load fails", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    render(<ArchiveView refreshToken={0} />);
    await screen.findByRole("heading", { name: "RE-1" });
    native.invoke.mockRejectedValueOnce(new Error("Datei fehlt"));
    fireEvent.click(screen.getByRole("button", { name: /RE-2/ }));
    await screen.findByRole("alert");
    expect(screen.queryByRole("button", { name: "PDF öffnen" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "RE-1" })).toBeNull();
  });
});
