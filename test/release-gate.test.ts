import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const root = resolve(import.meta.dirname, "..");
const read = (relative: string) => readFileSync(resolve(root, relative), "utf8");

describe("release delivery", () => {
  it("treats the official DATEV checker as pending unless it actually ran", () => {
    const gate = read("scripts/release-gate.mjs").replaceAll("\r\n", "\n");
    expect(gate).toContain('"datev-official"');
    expect(gate).toContain("Adaptertests sind kein Ersatz");
    expect(gate).toContain('record(\n    "datev-official"');
    expect(gate).toContain("Kein offizielles DATEV-Prüfprogramm");
    expect(gate).toContain("blocking: failed.length > 0");
  });

  it("rejects skipped production gates, test certificates and downgrades", () => {
    expect(read("scripts/build-windows-installer.ps1")).toContain(
      "Ein Produktionsbuild darf das Release-Gate nicht überspringen.",
    );
    expect(read("scripts/verify-release-signatures.ps1")).toContain(
      "Test- oder selbstsigniertes Zertifikat",
    );
    expect(read("apps/desktop/src-tauri/installer/windows/UpdateGuard.ps1")).toContain(
      "Eine Abwärtsinstallation ist nicht zulässig.",
    );
    expect(read("apps/desktop/src-tauri/nsis/installer-hooks.nsh")).toContain("PrepareUpdate");
  });

  it("documents bundled components and prepares CI without claiming a Windows runner", () => {
    expect(read("scripts/inventory-components.mjs")).toContain("docs/components.md");
    const workflow = read(".github/workflows/release-gate.yml");
    expect(workflow).toContain("ubuntu-latest");
    expect(workflow).toContain("release:gate -- --portable");
    expect(workflow).not.toMatch(/windows-latest|self-hosted/);
    expect(read("scripts/release-gate.mjs")).toContain("native-window-smoke.json");
  });

  it("keeps validator processes windowless and cancelable", () => {
    const validator = read("apps/desktop/src-tauri/src/validator.rs");
    expect(validator).toContain("CREATE_NO_WINDOW");
    expect(validator).toContain("cancel_invoice_validation");
    expect(validator).toContain("--disable-file-logging");
  });
});
