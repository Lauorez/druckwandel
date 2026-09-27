import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const portable = process.argv.includes("--portable");
const requireSignatures = process.argv.includes("--require-production-signatures");
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const steps = [];

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    shell: process.platform === "win32",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  });
  return {
    code: result.status ?? 1,
    output: `${result.stdout ?? ""}${result.stderr ?? ""}`.trim(),
  };
}

function record(id, title, status, detail) {
  steps.push({ id, title, status, detail: String(detail ?? "").slice(0, 2000) });
  const mark = status === "passed" ? "PASS" : status === "pending" ? "PENDING" : "FAIL";
  console.log(`${mark} ${title}${detail ? `: ${String(detail).split("\n")[0]}` : ""}`);
}

function required(id, title, command, args) {
  const result = run(command, args);
  if (result.code === 0) record(id, title, "passed", "ok");
  else record(id, title, "failed", result.output || `exit ${result.code}`);
}

function optional(id, title, present, command, args, pendingDetail) {
  if (!present) {
    record(id, title, "pending", pendingDetail);
    return;
  }
  required(id, title, command, args);
}

await import("./inventory-components.mjs");

required("typescript", "TypeScript, UI und Korpus", npmCmd, ["run", "check"]);

if (portable) {
  record("rust", "Rust-Clippy und -Tests", "pending", "Portabler Lauf ohne Cargo.");
  record("dotnet", ".NET-Druckkern", "pending", "Portabler Lauf ohne Windows-Druckerbuild.");
  record("xml", "KoSIT/Mustang-Regressionen", "pending", "Portabler Lauf ohne gebündelte Prüfer.");
  record("pdf", "Mustang/veraPDF-Hybridprüfung", "pending", "Portabler Lauf ohne gebündelte Prüfer.");
  record("installer-lifecycle", "Isolierte Installer-Szenarien", "pending", "Portabler Lauf ohne Windows.");
} else {
  const cargo = existsSync(resolve(root, "apps/desktop/src-tauri/Cargo.toml"));
  optional("rust", "Rust-Clippy und -Tests", cargo, npmCmd, ["run", "check:native"], "Cargo-Projekt fehlt.");
  const csproj = resolve(root, "drucker/tests/PrintCore.Tests/PrintCore.Tests.csproj");
  optional("dotnet", ".NET-Druckkern", existsSync(csproj), "dotnet", ["test", csproj, "--configuration", "Release"], "dotnet test nicht ausführbar.");
  const java = resolve(root, "apps/desktop/src-tauri/resources/validators/jre/bin", process.platform === "win32" ? "java.exe" : "java");
  optional("xml", "KoSIT/Mustang-Regressionen", existsSync(java), npmCmd, ["run", "check:xml"], "Gebündelte JRE fehlt. npm run validators:fetch auf Windows.");
  optional("pdf", "Mustang/veraPDF-Hybridprüfung", existsSync(java), npmCmd, ["run", "check:pdf"], "Gebündelte JRE fehlt. npm run validators:fetch auf Windows.");
  if (process.platform === "win32") {
    required("installer-lifecycle", "Isolierte Update-/Datenprüfung", "powershell.exe", [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      resolve(root, "scripts/test-installer-lifecycle.ps1"),
    ]);
  } else {
    record("installer-lifecycle", "Isolierte Update-/Datenprüfung", "pending", "Nur unter Windows.");
  }
}

const datevTool = [
  resolve(root, "artifacts/datev/reference/Datev_Format_Pruefprogramm_2_2_3_0_76439824cb.zip"),
  process.env.ERECHNUNG_DATEV_CHECKER,
].find((path) => path && existsSync(path));
if (process.env.ERECHNUNG_DATEV_CHECKER && existsSync(process.env.ERECHNUNG_DATEV_CHECKER)) {
  const result = run(process.env.ERECHNUNG_DATEV_CHECKER, process.env.ERECHNUNG_DATEV_CHECKER_ARGS?.split("\u0001") ?? []);
  record("datev-official", "Offizielles DATEV-Prüfprogramm", result.code === 0 ? "passed" : "failed", result.output || "ok");
} else {
  record(
    "datev-official",
    "Offizielles DATEV-Prüfprogramm",
    "pending",
    datevTool
      ? "DATEV-Prüfprogramm ist lokal vorhanden, wurde aber nicht unbeaufsichtigt ausgeführt."
      : "Kein offizielles DATEV-Prüfprogramm. Adaptertests sind kein Ersatz. node scripts/fetch-datev-reference.mjs --tools lädt die angebotenen Werkzeuge.",
  );
}

const audit = run(npmCmd, ["audit", "--omit=dev", "--audit-level=high"]);
if (audit.code === 0) record("npm-audit", "npm-Schwachstellen (high/critical, Produktion)", "passed", "keine blockierenden Befunde");
else if ((audit.output || "").includes("ENOTFOUND") || (audit.output || "").includes("network")) {
  record("npm-audit", "npm-Schwachstellen (high/critical, Produktion)", "pending", "Registry nicht erreichbar.");
} else record("npm-audit", "npm-Schwachstellen (high/critical, Produktion)", "failed", audit.output);

const cargoAudit = run("cargo", ["audit", "--version"]);
if (cargoAudit.code === 0) {
  const result = run("cargo", ["audit", "--manifest-path", "apps/desktop/src-tauri/Cargo.toml"]);
  record("cargo-audit", "Cargo-Schwachstellen", result.code === 0 ? "passed" : "failed", result.output);
} else {
  record("cargo-audit", "Cargo-Schwachstellen", "pending", "cargo-audit ist nicht installiert.");
}

if (requireSignatures) {
  if (process.platform !== "win32") {
    record("signatures", "Produktionssignaturen", "failed", "Signaturprüfung nur unter Windows.");
  } else {
    required("signatures", "Produktionssignaturen", "powershell.exe", [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      resolve(root, "scripts/verify-release-signatures.ps1"),
    ]);
  }
} else {
  record("signatures", "Produktionssignaturen", "pending", "Nur mit --require-production-signatures. Testzertifikate sind kein Release.");
}

await import("./acceptance-matrix.mjs");
record("acceptance-matrix", "Abnahmematrix", "passed", "docs/acceptance-matrix.md");

const nativeSmoke = resolve(root, "artifacts/wp14-native-smoke.json");
if (existsSync(nativeSmoke)) {
  const report = JSON.parse(readFileSync(nativeSmoke, "utf8"));
  if (report.passed === true) record("native-window", "Echtes Tauri-Fenster", "passed", nativeSmoke);
  else record("native-window", "Echtes Tauri-Fenster", "failed", "artifacts/wp14-native-smoke.json meldet kein Bestanden.");
} else {
  record(
    "native-window",
    "Echtes Tauri-Fenster",
    "pending",
    "Kein artifacts/wp14-native-smoke.json. Isoliert: scripts/run-wp14-smoke.ps1. Nicht aus Unit-Tests ableiten.",
  );
}

const failed = steps.filter((step) => step.status === "failed");
const pending = steps.filter((step) => step.status === "pending");
const report = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  portable,
  requireSignatures,
  passed: steps.filter((step) => step.status === "passed").length,
  pending: pending.length,
  failed: failed.length,
  blocking: failed.length > 0,
  steps,
};
await mkdir(resolve(root, "artifacts"), { recursive: true });
await writeFile(resolve(root, "artifacts/release-gate.json"), `${JSON.stringify(report, null, 2)}\n`);
if (failed.length) {
  console.error(`Release-Gate blockiert: ${failed.map((step) => step.id).join(", ")}`);
  process.exitCode = 1;
} else {
  console.log(`Release-Gate ohne Blocker. Ausstehend: ${pending.map((step) => step.id).join(", ") || "nichts"}`);
}
