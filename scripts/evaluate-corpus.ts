import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { createCorpusReport, evaluateExtraction, type CorpusManifest } from "../src/evaluation/corpus.js";
import { createQualityReport } from "../src/evaluation/quality-report.js";
import { extractInvoicePdf } from "../src/extraction/index.js";

function argument(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function validateManifest(value: unknown): asserts value is CorpusManifest {
  if (!value || typeof value !== "object") throw new Error("Korpusmanifest muss ein JSON-Objekt sein.");
  const manifest = value as Partial<CorpusManifest>;
  if (manifest.schemaVersion !== 1 || !Array.isArray(manifest.cases)) throw new Error("Nicht unterstützte Korpusmanifest-Version.");
  const ids = new Set<string>();
  for (const corpusCase of manifest.cases) {
    if (!corpusCase?.id || !corpusCase.description || !corpusCase.pdf || !corpusCase.expected) throw new Error("Jeder Korpusfall benötigt id, description, pdf und expected.");
    if (ids.has(corpusCase.id)) throw new Error(`Doppelte Korpus-ID: ${corpusCase.id}`);
    ids.add(corpusCase.id);
  }
}

const manifestPath = resolve(argument("--manifest") ?? "test/corpus/manifest.json");
const reportPath = resolve(argument("--report") ?? "artifacts/corpus-report.json");
const manifestDirectory = dirname(manifestPath);
const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as unknown;
validateManifest(manifest);

const evaluations = [];
for (const corpusCase of manifest.cases) {
  const pdfPath = resolve(manifestDirectory, corpusCase.pdf);
  const relativePdfPath = relative(manifestDirectory, pdfPath);
  if (relativePdfPath.startsWith("..") || resolve(pdfPath) === manifestDirectory) {
    throw new Error(`PDF-Pfad liegt außerhalb des Korpus: ${corpusCase.pdf}`);
  }
  const bytes = await readFile(pdfPath);
  if (bytes.length > 25 * 1024 * 1024) throw new Error(`Korpus-PDF ist größer als 25 MB: ${corpusCase.pdf}`);
  const extraction = await extractInvoicePdf(bytes);
  const evaluation = evaluateExtraction(corpusCase, extraction);
  evaluations.push(evaluation);
  const status = evaluation.passed ? "PASS" : "FAIL";
  console.log(`${status.padEnd(4)}  ${corpusCase.id.padEnd(28)} Felder ${evaluation.metrics.matchedFields}/${evaluation.metrics.expectedFields}, Positionen ${evaluation.metrics.matchedLineItems}/${evaluation.metrics.expectedLineItems}`);
  for (const check of evaluation.checks.filter((candidate) => !candidate.passed)) {
    console.log(`      ${check.path}: erwartet ${JSON.stringify(check.expected)}, erhalten ${JSON.stringify(check.actual)}`);
  }
}

const report = createCorpusReport(relative(process.cwd(), manifestPath), evaluations);
const qualityPath = resolve(argument("--quality-report") ?? "artifacts/quality-report.json");
const quality = createQualityReport(evaluations, report.generatedAt);
await mkdir(dirname(reportPath), { recursive: true });
await writeFile(reportPath, JSON.stringify(report, null, 2) + "\n", "utf8");
await writeFile(qualityPath, JSON.stringify(quality, null, 2) + "\n", "utf8");
console.log(`\nKorpus: ${report.totals.passedCases}/${report.totals.cases} Fälle, ${report.totals.matchedFields}/${report.totals.expectedFields} Felder, ${report.totals.matchedLineItems}/${report.totals.expectedLineItems} Positionen.`);
console.log(`Qualität: Felder ${(quality.fieldAccuracy.rate * 100).toFixed(1)} %, Positionen ${(quality.lineItemAccuracy.rate * 100).toFixed(1)} %, ${quality.neededCorrections.length} Korrekturen, ${quality.blocks.length} Blockierungen.`);
console.log(quality.disclaimer);
console.log(`Report: ${reportPath}`);
console.log(`Qualitätsbericht: ${qualityPath}`);
if (!report.passed) process.exitCode = 1;
