import { mkdir, readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const pkg = JSON.parse(await readFile(resolve(root, "package.json"), "utf8"));
const tauri = JSON.parse(await readFile(resolve(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"));
const cargo = await readFile(resolve(root, "apps/desktop/src-tauri/Cargo.toml"), "utf8");
const validators = JSON.parse(
  await readFile(resolve(root, "apps/desktop/src-tauri/resources/validators/manifest.json"), "utf8"),
);

function cargoVersion(name) {
  const match = cargo.match(new RegExp(`^${name}\\s*=\\s*(?:\\{[^}]*version\\s*=\\s*"([^"]+)"|"([^"]+)")`, "m"));
  return match?.[1] ?? match?.[2] ?? "unbekannt";
}

const components = [
  { name: "E-Rechnungs-Assistent", version: tauri.version, license: "proprietär / Projekt", source: "dieses Repository", role: "Anwendung" },
  { name: "Tauri", version: cargoVersion("tauri"), license: "MIT OR Apache-2.0", source: "https://github.com/tauri-apps/tauri", role: "Desktop-Laufzeit" },
  { name: "rusqlite", version: cargoVersion("rusqlite"), license: "MIT", source: "https://github.com/rusqlite/rusqlite", role: "Archiv/Entwürfe" },
  { name: "age", version: cargoVersion("age"), license: "MIT OR Apache-2.0", source: "https://github.com/str4d/rage", role: "Sicherung" },
  { name: "ed25519-dalek", version: cargoVersion("ed25519-dalek"), license: "BSD-3-Clause", source: "https://github.com/dalek-cryptography/curve25519-dalek", role: "Archivbestätigung" },
  { name: validators.java.vendor, version: `Temurin ${validators.java.version}`, license: validators.java.license, source: validators.java.source, role: "Prüflaufzeit" },
  { name: "KoSIT Validator", version: validators.kosit.engineVersion, license: validators.kosit.license, source: validators.kosit.source, role: "XRechnung" },
  { name: "XRechnung-Konfiguration", version: validators.kosit.ruleVersion, license: validators.kosit.license, source: validators.kosit.configurationSource, role: "XRechnung-Regeln" },
  { name: "Mustang-CLI", version: validators.mustang.engineVersion, license: validators.mustang.license, source: validators.mustang.source, role: "ZUGFeRD-XML" },
  { name: "veraPDF Greenfield", version: validators.verapdf.engineVersion, license: validators.verapdf.license, source: validators.verapdf.source, role: "PDF/A-3b" },
  { name: "E-Rechnungsdrucker", version: "MSIX CompanionApp", license: "Projekt", source: "drucker/", role: "Druckannahme" },
];

const npm = Object.entries({ ...pkg.dependencies, ...pkg.devDependencies }).map(([name, version]) => ({
  name,
  version: String(version).replace(/^[^0-9]*/, ""),
  role: pkg.dependencies[name] ? "Laufzeit" : "Entwicklung",
}));

const inventory = {
  schemaVersion: 1,
  appVersion: tauri.version,
  generatedAt: new Date().toISOString(),
  bundled: components,
  npm,
  notes: [
    "JAR-Dateien und die JRE werden nicht mit dem Git-Stand ausgeliefert; Herkunft steht in resources/validators/manifest.json.",
    "Produktionsbuilds dürfen Testzertifikate nicht enthalten.",
  ],
};

const directory = resolve(root, "artifacts");
await mkdir(directory, { recursive: true });
await writeFile(resolve(directory, "components.json"), `${JSON.stringify(inventory, null, 2)}\n`);

const checksumsPath = resolve(root, "apps/desktop/src-tauri/resources/validators/checksums.json");
const checksumNote = existsSync(checksumsPath)
  ? "SHA-256 der heruntergeladenen Prüfer: `apps/desktop/src-tauri/resources/validators/checksums.json`."
  : "Checksummen entstehen beim nächsten `npm run validators:fetch`.";

const markdown = `# Gebündelte Komponenten

Stand: Anwendung ${tauri.version}. Keine 1.0-Freigabe. Diese Liste beschreibt die mitgelieferten oder fest eingebundenen Bestandteile, nicht den gesamten transitiven Abhängigkeitsbaum.

${checksumNote}

| Komponente | Version | Rolle | Lizenz | Herkunft |
| --- | --- | --- | --- | --- |
${components.map((item) => `| ${item.name} | ${item.version} | ${item.role} | ${item.license} | ${item.source} |`).join("\n")}

npm-Laufzeitabhängigkeiten: ${Object.keys(pkg.dependencies).join(", ")}.

Eine maschinenlesbare Fassung schreibt \`node scripts/inventory-components.mjs\` nach \`artifacts/components.json\`. Schwachstellenprüfungen gehören zum Release-Gate; blockierende Befunde verhindern die Auslieferung.
`;
await writeFile(resolve(root, "docs/components.md"), markdown);
console.log(`components: ${components.length} bundled, app ${tauri.version}`);
