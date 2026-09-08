// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { analyzeDocumentPages } from "../src/extraction/analyze.js";
import { App } from "../apps/desktop/src/App.js";
import { workspaceNative } from "./helpers/workspace-native.js";

const native = vi.hoisted(() => ({ memory: null as string | null, extract: vi.fn(), invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: native.invoke }));
vi.mock("@tauri-apps/plugin-deep-link", () => ({ getCurrent: async () => [], onOpenUrl: async () => () => undefined }));
const windowMock = vi.hoisted(() => ({ close: undefined as undefined | ((event: { preventDefault: () => void }) => Promise<void>),destroy: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({
  onCloseRequested: async (listener: typeof windowMock.close) => { windowMock.close = listener; return () => { windowMock.close = undefined; }; },
  destroy: windowMock.destroy,
}) }));
vi.mock("../src/extraction/browser.js", () => ({ extractInvoicePdfInBrowser: native.extract }));
vi.mock("../src/extraction/browser-pdf.js", () => ({
  loadBrowserPdfJs: async () => ({ getDocument: () => ({
    promise: Promise.resolve({ getPage: async () => ({
      getViewport: () => ({ width: 600, height: 800 }), render: () => ({ promise: Promise.resolve() }),
    }) }), destroy: async () => undefined,
  }) }),
}));

function invoice(company: string, street: string, vat: string) {
  const values = [
    ["Rechnung", 50, 40], ["USt-ID:", 50, 80], [vat, 170, 80],
    ["Unternehmen", 50, 710], ["Bankverbindung", 240, 710], ["Steuerangaben", 430, 710],
    [company, 50, 730], ["IBAN: DE89 3704 0044 0532 0130 00", 240, 730], ["Steuernr.: 12/345/67890", 430, 730],
    [street, 50, 745], ["BIC: COBADEFFXXX", 240, 745], [`USt-IdNr.: ${vat}`, 430, 745],
  ] as const;
  return analyzeDocumentPages([{ page: 1, width: 600, height: 800, tokens: values.map(([text, x, y], index) => ({
    id: `t${index}`, page: 1, text, box: { x, y, width: 160, height: 10 }, origin: "text-layer" as const,
  })) }]);
}

function storedRules() {
  const parsed = JSON.parse(native.memory!) as { rules?: unknown[]; profiles?: Array<{ memory?: { rules?: unknown[] } }> };
  return parsed.profiles?.[0]?.memory?.rules ?? parsed.rules ?? [];
}

function openFile(name: string) {
  const file = new File(["synthetic"], name, { type: "application/pdf" });
  Object.defineProperty(file, "arrayBuffer", { value: async () => new ArrayBuffer(1) });
  fireEvent.change(screen.getByLabelText("Rechnung öffnen"), { target: { files: [file] } });
}

describe("mark and remember a source in the application", () => {
  let storage: ReturnType<typeof workspaceNative>;
  beforeEach(() => {
    storage = workspaceNative();
    windowMock.destroy.mockReset();
    native.memory = null;
    native.extract.mockReset();
    native.invoke.mockReset();
    native.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command.startsWith("workspace_")) return storage.invoke(command,args);
      if (command === "read_learning_memory") return native.memory;
      if (command === "write_learning_memory") { native.memory = String(args!.contents); return; }
      if (command === "acknowledge_print_job") return;
      if (command === "write_review_draft") return "test-entwurf.json";
      if (command === "list_print_jobs") return [];
      throw new Error(`Unexpected native call: ${command}`);
    });
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it("persists a clicked source and a typed missing field, then reads new values after restart", async () => {
    native.extract.mockResolvedValueOnce(invoice("Erste Firma GmbH", "Hauptstraße 12", "DE123456789"));
    const first = render(<App />);
    openFile("erste.pdf");
    fireEvent.click(await screen.findByRole("button", { name: "Absender: Name: Im PDF markieren" }));
    fireEvent.click(screen.getByRole("button", { name: "Erste Firma GmbH" }));
    expect((screen.getByLabelText("Wert aus der Markierung") as HTMLInputElement).value).toBe("Erste Firma GmbH");
    fireEvent.click(screen.getByRole("button", { name: "Übernehmen" }));
    fireEvent.change(screen.getAllByLabelText("Straße und Hausnummer")[0]!, { target: { value: "Hauptstraße 12" } });
    fireEvent.click(screen.getByRole("button", { name: "Entwurf speichern" }));
    await waitFor(() => expect(storedRules()).toHaveLength(2));
    await screen.findByText(/Entwurf gespeichert\. Sie können/);
    expect(native.memory).not.toContain("Erste Firma GmbH");
    expect(native.memory).not.toContain("Hauptstraße 12");
    expect(JSON.parse(native.memory!).schemaVersion).toBe(2);
    first.unmount();

    native.extract.mockResolvedValueOnce(invoice("Zweite Firma GmbH", "Bauhofstraße 18", "DE000000001"));
    render(<App />);
    openFile("zweite.pdf");
    await screen.findByDisplayValue("Zweite Firma GmbH");
    expect(screen.getByDisplayValue("Bauhofstraße 18")).toBeTruthy();
    expect(screen.queryByDisplayValue(/Zweite Firma GmbH IBAN/)).toBeNull();
  });

  it("refuses an invalid PDF assignment for a date field", async () => {
    native.extract.mockResolvedValueOnce(invoice("Erste Firma GmbH", "Hauptstraße 12", "DE123456789"));
    render(<App />);
    openFile("erste.pdf");
    fireEvent.click(await screen.findByRole("button", { name: "Fälligkeitsdatum: Im PDF markieren" }));
    fireEvent.click(screen.getByRole("button", { name: "Erste Firma GmbH" }));
    fireEvent.click(screen.getByRole("button", { name: "Übernehmen" }));
    expect((await screen.findByRole("alert")).textContent).toMatch(/gültiges Datum/);
    expect((screen.getByLabelText("Fälligkeitsdatum") as HTMLInputElement).value).toBe("");
  });

  it("autosaves without learning, and restores the exact draft without re-extraction", async () => {
    native.extract.mockResolvedValue(invoice("Firma A","Weg 1","DE123456789"));
    const first = render(<App />);
    openFile("a.pdf");
    await screen.findByRole("button", { name: "Absender: Name: Im PDF markieren" });
    fireEvent.change(screen.getAllByLabelText("Straße und Hausnummer")[0]!, { target: { value: "Manuell 42" } });
    await waitFor(() => expect(JSON.parse(storage.docs.get("doc-1")!.snapshot!).draft.seller.addressLine1).toBe("Manuell 42"));
    expect(native.memory).toBeNull();
    first.unmount();
    render(<App />);
    await screen.findByDisplayValue("Manuell 42");
    expect(native.extract).toHaveBeenCalledTimes(1);
    expect(native.memory).toBeNull();
  });

  it("queues a printed invoice without replacing the active draft", async () => {
    native.extract.mockResolvedValue(invoice("Firma A","Weg 1","DE123456789"));
    render(<App />); openFile("a.pdf");
    await screen.findByRole("button",{ name: "Absender: Name: Im PDF markieren" });
    fireEvent.change(screen.getAllByLabelText("Straße und Hausnummer")[0]!,{ target: { value: "Bleibt erhalten" } });
    storage.candidates.push({ key: "print:b",legacy: false,job: { jobId: "b",path: "b.pdf",name: "b.pdf",modifiedMs: 1,size: 10 } });
    await screen.findByText("Posteingang (2)",{}, { timeout: 3500 });
    expect(screen.getByDisplayValue("Bleibt erhalten")).toBeTruthy();
    expect(native.extract).toHaveBeenCalledTimes(1);
    expect(native.invoke.mock.calls.some(([command,args]) => command === "acknowledge_print_job" && args.jobId === "b")).toBe(false);
    fireEvent.click(screen.getByText("Posteingang (2)"));
    fireEvent.click(screen.getByRole("button",{ name: /b\.pdf/ }));
    await waitFor(() => expect(native.extract).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(native.invoke.mock.calls.some(([command,args]) => command === "acknowledge_print_job" && args.jobId === "b" && args.status === "opened")).toBe(true));
    expect(JSON.parse(storage.docs.get("doc-1")!.snapshot!).draft.seller.addressLine1).toBe("Bleibt erhalten");
  });

  it("refuses closing or switching when the latest draft cannot be saved", async () => {
    native.extract.mockResolvedValue(invoice("Firma A","Weg 1","DE123456789"));
    render(<App />); openFile("a.pdf");
    await screen.findByRole("button",{ name: "Absender: Name: Im PDF markieren" });
    storage.failSave = true;
    fireEvent.change(screen.getAllByLabelText("Straße und Hausnummer")[0]!,{ target: { value: "Noch nicht gesichert" } });
    await screen.findByText("Nicht gespeichert");
    await windowMock.close!({ preventDefault: vi.fn() });
    expect(windowMock.destroy).not.toHaveBeenCalled();
    openFile("b.pdf");
    await screen.findByText("Test: Speicherung nicht möglich");
    expect(storage.docs.size).toBe(1);
    expect(screen.getByDisplayValue("Noch nicht gesichert")).toBeTruthy();
  });

  it("imports an older JSON draft only after the user supplies its original PDF", async () => {
    native.extract.mockResolvedValue(invoice("Firma A","Weg 1","DE123456789"));
    render(<App />); openFile("a.pdf");
    await screen.findByRole("button",{ name: "Absender: Name: Im PDF markieren" });
    const original = JSON.parse(storage.docs.get("doc-1")!.snapshot!);
    const legacy = { schemaVersion:1,source:"alte-rechnung.pdf",draft:{ ...original.draft,invoiceNumber:"ALTER-ENTWURF" } };
    const json = new File([JSON.stringify(legacy)],"entwurf.json",{ type:"application/json" });
    Object.defineProperty(json,"text",{ value:async()=>JSON.stringify(legacy) });
    fireEvent.change(screen.getByLabelText("Entwurf öffnen"),{target:{files:[json]}});
    await screen.findByRole("region",{name:"Originaldatei zuordnen"});
    expect(storage.docs.size).toBe(1);
    const pdf = new File(["%PDF-test"],"alte-rechnung.pdf",{type:"application/pdf"});
    Object.defineProperty(pdf,"arrayBuffer",{value:async()=>new ArrayBuffer(1)});
    fireEvent.change(screen.getByLabelText("Zugehörige PDF auswählen"),{target:{files:[pdf]}});
    await screen.findByDisplayValue("ALTER-ENTWURF");
    expect(storage.docs.size).toBe(2);
    expect(JSON.parse(storage.docs.get("doc-2")!.snapshot!).draft.invoiceNumber).toBe("ALTER-ENTWURF");
    expect(native.memory).toBeNull();
  });
});
