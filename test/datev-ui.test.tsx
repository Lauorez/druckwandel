// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DatevView } from "../apps/desktop/src/DatevView.js";
import { canonicalJson } from "../src/export/invoice-snapshot.js";
import { profile, source } from "./helpers/datev-source.js";

const native = vi.hoisted(() => ({ invoke: vi.fn(), profile: "" }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke, isTauri: () => true }));
vi.mock("@tauri-apps/api/event", async () => (await import("./helpers/tauri-events.js")).tauriEvents());

describe("DATEV invoice selection and profile edits", () => {
  beforeEach(() => {
    native.profile = canonicalJson(profile());
    native.invoke.mockReset();
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "datev_get_profile") return native.profile;
      if (command === "datev_save_profile") { native.profile = String(args!.contents); return; }
      if (command === "list_archive_entries") return { entries: [{
        id: "a1", invoiceNumber: "RE-100", buyerName: "Kunde", grossAmount: "119.00", currency: "USD",
        issueDate: "2026-09-01", format: "xrechnung", documentId: "doc-1",
      }], total: 1 };
      if (command === "datev_export_status") return [];
      if (command === "datev_list_exports") return { entries: [], total: 0 };
      if (command === "archive_datev_source") return { ...source(), archiveId: "a1" };
      throw new Error(command);
    });
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("shows the actual archive currency before validating export eligibility", async () => {
    render(<DatevView onBack={() => undefined} onBusyChange={() => undefined} />);
    await screen.findByText("119,00 USD");
    expect(screen.queryByText("119,00 EUR")).toBeNull();
  });

  it("keeps newer account edits when an older save emits a settings event", async () => {
    let finishSave!: () => void;
    const saved: string[] = [];
    const implementation = native.invoke.getMockImplementation()!;
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "datev_save_profile") {
        saved.push(String(args!.contents));
        if (saved.length === 1) await new Promise<void>(resolve => { finishSave = resolve; });
        native.profile = String(args!.contents);
        return;
      }
      return implementation(command, args);
    });
    render(<DatevView onBack={() => undefined} onBusyChange={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: /RE-100/ }));
    const account = await screen.findByLabelText(/Kundenkonto für/);
    fireEvent.change(account, { target: { value: "10001" } });
    await waitFor(() => expect(saved).toHaveLength(1));
    fireEvent.change(account, { target: { value: "10002" } });
    await act(async () => finishSave());
    expect(account).toHaveProperty("value", "10002");
    await waitFor(() => expect(JSON.parse(native.profile).debtors[0].account).toBe("10002"));
  });

  it("offers a retry after an automatic profile save fails", async () => {
    let fail = true;
    const implementation = native.invoke.getMockImplementation()!;
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "datev_save_profile" && fail) throw new Error("Datenträger voll");
      return implementation(command, args);
    });
    render(<DatevView onBack={() => undefined} onBusyChange={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: /RE-100/ }));
    fireEvent.change(await screen.findByLabelText(/Kundenkonto für/), { target: { value: "10001" } });
    await screen.findByText(/Kanzleiangaben nicht gespeichert/);
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Erneut speichern" }));
    await waitFor(() => expect(JSON.parse(native.profile).debtors[0].account).toBe("10001"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Zurück zum Archiv" })).toHaveProperty("disabled", false));
  });
});
