import { existsSync, readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");

function readJson(relative) {
  const path = resolve(root, relative);
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

function evidenceStatus(relative) {
  const data = readJson(relative);
  if (!data) return { result: "ungeprüft", note: `Kein Nachweis ${relative}. Nicht aus lokalen Unit-Tests ableiten.` };
  if (data.passed === true || data.result === "passed") {
    return { result: "bestanden", note: `${relative} (${data.generatedAt ?? data.createdAt ?? data.timestamp ?? "ohne Zeitstempel"})` };
  }
  if (data.passed === false || data.result === "failed") {
    return { result: "fehlgeschlagen", note: relative };
  }
  return { result: "ungeprüft", note: `${relative} vorhanden, enthält aber kein bestandenes Ergebnis.` };
}

const rows = [
  {
    id: "vitest",
    flow: "TypeScript, UI, Korpus, DATEV-Adapter, Sicherung, Diagnose",
    data: "Referenzkorpus und synthetische Fixtures",
    os: "Entwicklung (dieser Arbeitsbaum)",
    app: "0.3.1",
    standard: "EN 16931 / XRechnung 3.0.2",
    result: "bestanden",
    note: "npm test / npm run check. Kein Zielsystemtest.",
  },
  {
    id: "rust",
    flow: "Rust-Archiv, Workspace, DATEV-Journal, Backup, Diagnosegrenzen",
    data: "isolierte Temp-Profile",
    os: "Entwicklung (dieser Arbeitsbaum)",
    app: "0.3.1",
    standard: "—",
    result: "bestanden",
    note: "cargo test --lib. Kein Zielsystemtest.",
  },
  {
    id: "xml-pdf",
    flow: "Unabhängige XML- und Hybrid-PDF/A-Prüfung",
    data: "test/fixtures und Musterrechnung",
    os: "Windows mit gebündelter JRE",
    app: "0.3.1",
    standard: "XRechnung 3.0.2, ZUGFeRD EN 16931, PDF/A-3b",
    result: existsSync(resolve(root, "apps/desktop/src-tauri/resources/validators/jre/bin/java.exe"))
      ? "lokal ausführbar"
      : "ungeprüft",
    note: "npm run check:xml / check:pdf. Nicht aus Vitest ableiten, wenn die JRE fehlt.",
  },
  {
    id: "native-window",
    flow: "Echtes Tauri-Fenster: Posteingang, Archiv, DATEV, Diagnose, Tastatur, 100/150/200 %",
    data: "isoliertes erechnung-wp*-Profil",
    os: "Windows 11 Debug-Build",
    app: "0.3.1",
    standard: "—",
    ...evidenceStatus("artifacts/native-window-smoke.json"),
  },
  {
    id: "print-cold",
    flow: "Drucken bei geschlossener App",
    data: "WP7 Drucktest <ID>",
    os: "Windows 11 mit Drucker E-Rechnung",
    app: "0.3.1",
    standard: "—",
    ...evidenceStatus("artifacts/wp7-print-smoke.json"),
  },
  {
    id: "installer-isolated",
    flow: "Isolierte Update-/Datenprüfung, keine Abwärtsinstallation",
    data: "temporäre Dummy-SQLite",
    os: "Windows",
    app: "0.3.1",
    standard: "—",
    result: "bestanden",
    note: "scripts/test-installer-lifecycle.ps1. Kein Update vom echten 0.2.2-Teststand.",
  },
  {
    id: "update-022",
    flow: "Update vom gesicherten 0.2.2-Teststand, Deinstallation, Neuinstallation",
    data: "gesicherter Nutzerbestand",
    os: "bereitgestelltes Windows 11",
    app: "0.3.1",
    standard: "—",
    result: "ungeprüft",
    note: "Hardwareabnahme. Nicht aus dem isolierten String-/Snapshot-Test ableiten.",
  },
  {
    id: "datev-import",
    flow: "DATEV-Testimport des beworbenen EXTF-Umfangs",
    data: "Kanzleiprofil der Pilotkanzlei",
    os: "DATEV Rechnungswesen",
    app: "0.3.1",
    standard: "EXTF 700/21/13, Belegtransfer v6.0",
    result: "ungeprüft",
    note: "Offizielles Prüfprogramm und Kanzleiimport bleiben extern.",
  },
  {
    id: "signatures",
    flow: "Produktionssignaturen Anwendung, Helfer, Drucker, Installer",
    data: "—",
    os: "Windows",
    app: "0.3.1",
    standard: "Authenticode",
    result: "ungeprüft",
    note: "Testzertifikate sind kein Release. --require-production-signatures.",
  },
  {
    id: "pilot",
    flow: "Pilotbetrieb und ausgewertetes Feedback",
    data: "echte Belege der Pilotbetriebe",
    os: "Zielsysteme der Pilotbetriebe",
    app: "0.3.1",
    standard: "vereinbarter Rechnungsumfang",
    result: "ungeprüft",
    note: "Betriebe, Einverständnisse und Rückmeldungen sind nicht Teil dieser lokalen Abnahme.",
  },
];

const claimedWithoutEvidence = rows.filter(
  (row) => row.result === "bestanden" && row.note?.includes("Kein Nachweis"),
);
if (claimedWithoutEvidence.length) {
  throw new Error(`Matrix behauptet Bestanden ohne Nachweis: ${claimedWithoutEvidence.map((row) => row.id).join(", ")}`);
}

const matrix = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  appVersion: "0.3.1",
  release: "kein 1.0",
  rows,
};

const markdown = `# Abnahmematrix 0.3.1

Stand: ${new Date().toISOString().slice(0, 10)}. Keine 1.0- oder Kundenfreigabe.

Nicht verfügbare Kombinationen stehen als **ungeprüft**. Ein bestandener Unit-Test auf dem Entwicklungsrechner gilt nicht als Nachweis für Druck, Installer-Update, DATEV-Import oder Pilotbetrieb.

| Ablauf | Testdaten | Betriebssystem | App | Standard | Ergebnis | Nachweis |
| --- | --- | --- | --- | --- | --- | --- |
${rows
  .map(
    (row) =>
      `| ${row.flow} | ${row.data} | ${row.os} | ${row.app} | ${row.standard} | ${row.result} | ${row.note} |`,
  )
  .join("\n")}

Maschinenlesbar: \`artifacts/acceptance-matrix.json\` (gitignoriert). Erzeugen: \`npm run release:matrix\`.
`;

await mkdir(resolve(root, "artifacts"), { recursive: true });
await writeFile(resolve(root, "artifacts/acceptance-matrix.json"), `${JSON.stringify(matrix, null, 2)}\n`);
await writeFile(resolve(root, "docs/acceptance-matrix.md"), markdown);
console.log(`acceptance-matrix: ${rows.length} Zeilen, ungeprüft: ${rows.filter((r) => r.result === "ungeprüft").length}`);
