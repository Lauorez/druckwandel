import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const root = resolve(import.meta.dirname, "..");
const read = (relative: string) => readFileSync(resolve(root, relative), "utf8");

describe("release-candidate evidence", () => {
  it("does not treat missing hardware evidence as passed", () => {
    const matrix = read("docs/acceptance-matrix.md");
    expect(matrix).toContain("ungeprüft");
    expect(matrix).toContain("DATEV-Testimport");
    expect(matrix).toContain("Produktionssignaturen");
    expect(matrix).toContain("Pilotbetrieb");
    expect(matrix).toContain("gilt nicht als Nachweis");
    expect(matrix).toContain("Hardwareabnahme");
    const script = read("scripts/acceptance-matrix.mjs");
    expect(script).toContain('result: "ungeprüft"');
    expect(script).toContain("native-window-smoke.json");
    expect(script).toContain("behauptet Bestanden ohne Nachweis");
  });

  it("keeps native window checks behind an isolated smoke script", () => {
    const smoke = read("scripts/smoke-release.mjs");
    expect(smoke).toContain("erechnung-wp");
    expect(smoke).toContain("deviceScaleFactor");
    expect(smoke).toContain("Diagnose");
    expect(smoke).toContain("Escape");
    expect(smoke).not.toContain("?.click() !== undefined");
    expect(smoke).toContain("button.click(); return true");
    expect(read("scripts/smoke-native-window.ps1")).toContain("Keinen echten Nutzerbestand");
    expect(read("scripts/release-gate.mjs")).toContain("native-window-smoke.json");
    expect(read("docs/pilot-guide.md")).toContain("Diagnosebericht");
  });
});
