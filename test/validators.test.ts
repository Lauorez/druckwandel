import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import {
  evaluateKositOutcome,
  evaluateMustangOutcome,
  evaluateVeraPdfOutcome,
  germanFieldLabel,
  mapOfficialLocationToPath,
} from "../src/engine/validation-report.js";

const fixture = (name: string) => readFileSync(resolve(import.meta.dirname, "fixtures/validation", name), "utf8");

describe("official XML validation reports", () => {
  it("accepts a KoSIT report only when valid, accepted and a report is present", () => {
    const passed = evaluateKositOutcome({ code: 0, output: "ok", reportXml: fixture("kosit-valid.xml") }, "xrechnung-3.0.2");
    expect(passed).toMatchObject({ status: "passed", valid: true, engine: "kosit" });
  });

  it("rejects KoSIT results without a machine-readable report even when the exit code is 0", () => {
    const missing = evaluateKositOutcome({ code: 0, output: "Acceptance|Error\n| ACCEPT" }, "xrechnung-3.0.2");
    expect(missing.valid).toBe(false);
    expect(missing.status).toBe("missing-report");
  });

  it("treats KoSIT reject assessment as failed despite valid=true and exit 0", () => {
    const rejected = evaluateKositOutcome({ code: 0, output: "", reportXml: fixture("kosit-exit0-rejected.xml") }, "xrechnung-3.0.2");
    expect(rejected.valid).toBe(false);
    expect(rejected.status).toBe("failed");
  });

  it("maps KoSIT failed asserts to German fields", () => {
    const failed = evaluateKositOutcome({ code: 1, output: "", reportXml: fixture("kosit-invalid.xml") }, "xrechnung-3.0.2");
    expect(failed.valid).toBe(false);
    expect(failed.issues.some((issue) => issue.path === "buyerReference" && issue.code === "BR-DE-15")).toBe(true);
    expect(failed.issues.some((issue) => issue.path === "seller.name")).toBe(true);
    expect(germanFieldLabel("buyerReference")).toBe("Bestellnummer oder Leitweg-ID");
  });

  it("fails on timeout, cancellation and unreadable reports", () => {
    expect(evaluateKositOutcome({ code: -1, output: "", timedOut: true }, "xrechnung-3.0.2").status).toBe("timeout");
    expect(evaluateKositOutcome({ code: -1, output: "", cancelled: true }, "xrechnung-3.0.2").status).toBe("cancelled");
    expect(evaluateKositOutcome({ code: 0, output: "", reportXml: "<not-a-report/>" }, "xrechnung-3.0.2").status).toBe("unreadable-report");
  });

  it("requires Mustang summary status valid and a readable report", () => {
    const passed = evaluateMustangOutcome({ code: 0, output: fixture("mustang-valid.xml") }, "factur-x");
    expect(passed).toMatchObject({ status: "passed", valid: true });
    const failed = evaluateMustangOutcome({ code: 1, output: fixture("mustang-invalid.xml") }, "factur-x");
    expect(failed.valid).toBe(false);
    expect(failed.issues[0]?.path).toBe("totals.taxInclusive");
    const missing = evaluateMustangOutcome({ code: 0, output: "" }, "factur-x");
    expect(missing.status).toBe("missing-report");
    const unreadable = evaluateMustangOutcome({ code: 0, output: "ok" }, "factur-x");
    expect(unreadable.status).toBe("unreadable-report");
  });

  it("maps invoice line locations", () => {
    expect(mapOfficialLocationToPath("/Invoice/InvoiceLine[1]/InvoicedQuantity", "BT-129")).toBe("lines.0.quantity");
  });

  it("accepts veraPDF only with a compliant PDF/A-3b report", () => {
    const passed = evaluateVeraPdfOutcome({ code: 0, output: "", reportXml: fixture("verapdf-valid.xml") }, "pdfa-3b");
    expect(passed).toMatchObject({ status: "passed", valid: true, engine: "verapdf" });
    const failed = evaluateVeraPdfOutcome({ code: 1, output: "", reportXml: fixture("verapdf-invalid.xml") }, "pdfa-3b");
    expect(failed.valid).toBe(false);
    const missing = evaluateVeraPdfOutcome({ code: 0, output: "" }, "pdfa-3b");
    expect(missing.status).toBe("missing-report");
  });
});

describe("validator fetch packaging", () => {
  it("entpackt Archive mit tar, damit Windows ohne unzip auskommt", () => {
    const script = readFileSync(resolve(import.meta.dirname, "../scripts/fetch-validators.mjs"), "utf8");
    expect(script).toContain('spawnSync("tar"');
    expect(script).toContain("--force-local");
    expect(script).not.toContain('spawnSync("unzip"');
  });
});
