// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsView } from "../apps/desktop/src/SettingsView.js";
import { emptyDatevProfile } from "../apps/desktop/src/datevStore.js";

const native = vi.hoisted(() => ({
  memory: null as string | null,
  datev: null as string | null,
  signing: false,
  failSave: false,
  invoke: vi.fn(),
  destroy: vi.fn(),
  close: undefined as undefined | ((event: { preventDefault: () => void }) => void),
}));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", async () => (await import("./helpers/tauri-events.js")).tauriEvents());
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({
  destroy: native.destroy,
  onCloseRequested: async (handler: typeof native.close) => { native.close = handler; return () => { native.close = undefined; }; },
}) }));

describe("settings window content", () => {
  beforeEach(() => {
    native.memory = null;
    native.datev = null;
    native.signing = false;
    native.failSave = false;
    native.destroy.mockReset();
    native.close = undefined;
    native.invoke.mockReset();
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "read_learning_memory") return native.memory;
      if (command === "write_learning_memory") { native.memory = String(args!.contents); return; }
      if (command === "datev_get_profile") return native.datev;
      if (command === "datev_save_profile") {
        if (native.failSave) throw new Error("Datenträger voll");
        native.datev = String(args!.contents); return;
      }
      if (command === "get_archive_status") return { rootPath: "C:\\archiv", entryCount: 0, signedCount: 0, signingEnabled: native.signing };
      if (command === "backup_status") return { sameVolume: false, reminderDue: true, pendingRestore: false, defaultPath: "C:\\Sicherungen\\E-Rechnung.erechnung" };
      if (command === "diagnostic_report") return { schemaVersion: 1, createdAtMs: 1, appVersion: "0.3.1", os: "windows", arch: "x64", components: [], archiveEntries: 0, signedEntries: 0, signingEnabled: false, workspaceDocuments: 0, datevProfilePresent: false, backupReminderDue: true, pendingRestore: false, notes: ["DATEV_OFFICIAL_CHECK_PENDING"] };
      if (command === "set_archive_signing") {
        native.signing = Boolean(args!.enabled);
        return { rootPath: "C:\\archiv", entryCount: 0, signedCount: 0, signingEnabled: native.signing };
      }
      throw new Error(`Unexpected native call: ${command}`);
    });
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("keeps keyboard focus inside the overlay and restores it on close", async () => {
    const trigger = document.createElement("button");
    document.body.append(trigger);
    trigger.focus();
    const view = render(<SettingsView overlay onClose={() => undefined} />);
    const close = screen.getByRole("button", { name: "Schließen" });
    expect(document.activeElement).toBe(close);
    await waitFor(() => expect(screen.getByRole("button", { name: "Neues Profil" })).toHaveProperty("disabled", false));
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(document.activeElement).not.toBe(close);
    expect(screen.getByRole("dialog").contains(document.activeElement)).toBe(true);
    fireEvent.keyDown(document.activeElement!, { key: "Tab" });
    expect(document.activeElement).toBe(close);
    view.unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });

  it("invalidates the checked restore when another file or password is entered", async () => {
    const checked = { archiveEntries: 1, drafts: 2, originals: 2, datevExports: 0, hasSigningKey: false, chainHead: "h", createdAtMs: 1, issues: [] };
    const implementation = native.invoke.getMockImplementation()!;
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "backup_status") return { ...await implementation(command, args), checkedPreview: checked };
      if (command === "backup_preview") return checked;
      return implementation(command, args);
    });
    render(<SettingsView overlay initialSection="backup" onClose={() => undefined} />);
    await screen.findByRole("button", { name: "Geprüfte Sicherung übernehmen" });
    fireEvent.change(screen.getByLabelText("Sicherungsdatei"), { target: { value: "D:\\andere.erechnung" } });
    expect(screen.queryByRole("button", { name: "Geprüfte Sicherung übernehmen" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Sicherung prüfen" }));
    await screen.findByRole("button", { name: "Geprüfte Sicherung übernehmen" });
    fireEvent.change(screen.getByLabelText("Kennwort der Sicherung"), { target: { value: "anderes Kennwort" } });
    expect(screen.queryByRole("button", { name: "Geprüfte Sicherung übernehmen" })).toBeNull();
  });

  it("removes a previous restore confirmation when a new check fails", async () => {
    const checked = { archiveEntries: 1, drafts: 2, originals: 2, datevExports: 0, hasSigningKey: false, chainHead: "h", createdAtMs: 1, issues: [] };
    const implementation = native.invoke.getMockImplementation()!;
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "backup_status") return { ...await implementation(command, args), checkedPreview: checked };
      if (command === "backup_preview") throw new Error("Sicherungsdatei beschädigt");
      return implementation(command, args);
    });
    render(<SettingsView overlay initialSection="backup" onClose={() => undefined} />);
    await screen.findByRole("button", { name: "Geprüfte Sicherung übernehmen" });
    fireEvent.click(screen.getByRole("button", { name: "Sicherung prüfen" }));
    await screen.findByText("Sicherungsdatei beschädigt");
    expect(screen.queryByRole("button", { name: "Geprüfte Sicherung übernehmen" })).toBeNull();
  });

  it("creates a recognition profile in the settings overlay", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("Kanzlei Nord");
    render(<SettingsView overlay onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: "Neues Profil" }));
    await waitFor(() => expect(JSON.parse(native.memory!).profiles.map((profile: { name: string }) => profile.name)).toContain("Kanzlei Nord"));
    expect((screen.getByLabelText("Aktives Erkennungsprofil") as HTMLSelectElement).selectedOptions[0]?.text).toBe("Kanzlei Nord");
  });

  it("toggles archive signing from the archive settings section", async () => {
    render(<SettingsView overlay initialSection="archive" onClose={() => undefined} />);
    const checkbox = await screen.findByRole("checkbox") as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    fireEvent.click(checkbox);
    await waitFor(() => expect(native.signing).toBe(true));
    expect(checkbox.checked).toBe(true);
  });

  it("stores DATEV office details from the settings form", async () => {
    native.datev = JSON.stringify(emptyDatevProfile());
    render(<SettingsView overlay initialSection="datev" onClose={() => undefined} />);
    const consultant = await screen.findByLabelText("Beraternummer") as HTMLInputElement;
    fireEvent.change(consultant, { target: { value: "12345" } });
    await waitFor(() =>     expect(JSON.parse(native.datev!).consultant).toBe("12345"));
    expect(screen.getByRole("button", { name: "Speichern" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Kanzleiangaben gespeichert" })).toBeNull();
    expect(screen.getAllByRole("heading", { name: "Angaben der Steuerkanzlei" })).toHaveLength(1);
  });

  it.each(["close", "escape", "section", "native"])("flushes office edits before leaving through %s", async route => {
    native.datev = JSON.stringify(emptyDatevProfile());
    const onClose = vi.fn(() => { expect(JSON.parse(native.datev!).consultant).toBe("12345"); });
    render(<SettingsView overlay={route !== "native"} initialSection="datev" onClose={onClose} />);
    await screen.findByText("Gespeichert.");
    fireEvent.change(screen.getByLabelText("Beraternummer"), { target: { value: "12345" } });
    if (route === "section") {
      fireEvent.click(screen.getByRole("button", { name: "Erkennungsprofile" }));
      await screen.findByRole("button", { name: "Neues Profil" });
    } else if (route === "native") {
      await waitFor(() => expect(native.close).toBeTypeOf("function"));
      const preventDefault = vi.fn();
      native.close!({ preventDefault });
      expect(preventDefault).toHaveBeenCalled();
      await waitFor(() => expect(native.destroy).toHaveBeenCalled());
    } else {
      if (route === "escape") fireEvent.keyDown(window, { key: "Escape" });
      else fireEvent.click(screen.getByRole("button", { name: "Schließen" }));
      await waitFor(() => expect(onClose).toHaveBeenCalled());
    }
    expect(JSON.parse(native.datev!).consultant).toBe("12345");
  });

  it("creates a password-protected backup from the settings section", async () => {
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "backup_status") return { sameVolume: false, reminderDue: true, pendingRestore: false, defaultPath: "C:\\Sicherungen\\E-Rechnung.erechnung" };
      if (command === "backup_create") {
        expect(args).toMatchObject({ destination: "D:\\extern\\stand.erechnung", password: "Testkennwort-12" });
        return { path: args!.destination, createdAtMs: 1, sameVolume: false, archiveEntries: 2, drafts: 1 };
      }
      throw new Error(`Unexpected native call: ${command}`);
    });
    render(<SettingsView overlay initialSection="backup" onClose={() => undefined} />);
    const destination = await screen.findByLabelText("Zieldatei") as HTMLInputElement;
    fireEvent.change(destination, { target: { value: "D:\\extern\\stand.erechnung" } });
    fireEvent.change(screen.getByLabelText("Sicherungskennwort"), { target: { value: "Testkennwort-12" } });
    fireEvent.change(screen.getByLabelText("Sicherungskennwort wiederholen"), { target: { value: "Testkennwort-12" } });
    fireEvent.click(screen.getByRole("button", { name: "Sicherung erstellen" }));
    await screen.findByText(/Sicherung gespeichert \(2 Archiveinträge, 1 Entwürfe\)/);
  });

  it("rehydrates a checked restore after restart instead of treating it as interrupted", async () => {
    native.invoke.mockImplementation(async (command: string) => {
      if (command === "backup_status") {
        return {
          sameVolume: false,
          reminderDue: true,
          pendingRestore: false,
          pendingPreview: true,
          checkedPreview: {
            archiveEntries: 3,
            drafts: 1,
            originals: 1,
            datevExports: 0,
            hasSigningKey: true,
            chainHead: "abc",
            createdAtMs: 1,
            issues: [],
          },
          defaultPath: "C:\\Sicherungen\\E-Rechnung.erechnung",
        };
      }
      throw new Error(`Unexpected native call: ${command}`);
    });
    render(<SettingsView overlay initialSection="backup" onClose={() => undefined} />);
    await screen.findByText(/Geprüft: 3 Archiveinträge/);
    expect(screen.queryByRole("button", { name: "Wiederherstellung fortsetzen" })).toBeNull();
    expect(screen.getByRole("button", { name: "Geprüfte Sicherung übernehmen" })).toBeTruthy();
  });

  it("previews a diagnostic report before saving", async () => {
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === "diagnostic_report") {
        return { schemaVersion: 1, createdAtMs: 1, appVersion: "0.3.1", os: "windows", arch: "x64", components: [{ name: "KoSIT", version: "1.6.3", status: "vorhanden" }], archiveEntries: 2, signedEntries: 0, signingEnabled: false, workspaceDocuments: 1, datevProfilePresent: false, backupReminderDue: true, pendingRestore: false, notes: ["DATEV_OFFICIAL_CHECK_PENDING"] };
      }
      if (command === "write_diagnostic_report") {
        expect(args!.contents).toBeUndefined();
        expect(args!.destination).toBe("D:\\extern\\diagnose.json");
        return args!.destination;
      }
      throw new Error(`Unexpected native call: ${command}`);
    });
    render(<SettingsView overlay initialSection="help" onClose={() => undefined} />);
    await screen.findByText(/DATEV_OFFICIAL_CHECK_PENDING/);
    const save = screen.getByRole("button", { name: "Bericht speichern" }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.change(screen.getByLabelText("Zieldatei"), { target: { value: "D:\\extern\\diagnose.json" } });
    fireEvent.click(save);
    await screen.findByText(/Diagnosebericht gespeichert/);
  });

  it("keeps settings open after a failed save and retries on close", async () => {
    native.datev = JSON.stringify(emptyDatevProfile());
    const onClose = vi.fn();
    render(<SettingsView overlay initialSection="datev" onClose={onClose} />);
    await screen.findByText("Gespeichert.");
    native.failSave = true;
    fireEvent.change(screen.getByLabelText("Beraternummer"), { target: { value: "12345" } });
    fireEvent.click(screen.getByRole("button", { name: "Schließen" }));
    await screen.findByText(/Einstellungen nicht gespeichert: Datenträger voll/);
    expect(onClose).not.toHaveBeenCalled();
    native.failSave = false;
    fireEvent.click(screen.getByRole("button", { name: "Schließen" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(JSON.parse(native.datev!).consultant).toBe("12345");
  });
});
