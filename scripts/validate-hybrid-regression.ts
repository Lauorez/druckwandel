import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { calculateInvoice } from "../src/domain/calculate.js";
import { generateCii } from "../src/engine/cii.js";
import { extractFacturXXmlFromPdf, prepareHybridPdf } from "../src/engine/hybrid-pdf.js";
import { MustangValidator, VeraPdfValidator } from "../src/engine/validators.js";
import { extractInvoicePdf } from "../src/extraction/index.js";
import { invoiceInputFromReview, reviewDraftFromExtraction } from "../src/review/draft.js";

const bundle = resolve("apps/desktop/src-tauri/resources/validators");
const manifest = JSON.parse(await readFile(resolve(bundle, "manifest.json"), "utf8"));
const java = resolve(bundle, `${manifest.java.relativePath}${process.platform === "win32" ? ".exe" : ""}`);
const source = await readFile(resolve("artifacts/demo/muster-rechnung.pdf"));
const draft = reviewDraftFromExtraction(await extractInvoicePdf(source));
const xml = generateCii(calculateInvoice(invoiceInputFromReview(draft)));
const prepared = await prepareHybridPdf(source, xml);
const directory = resolve("artifacts/hybrid-regression");
await mkdir(directory, { recursive: true });
const path = resolve(directory, "invoice.pdf");
await writeFile(path, prepared);
const extracted = await extractFacturXXmlFromPdf(prepared);
assert.equal(extracted, xml, "Die eingebetteten XML-Bytes müssen unverändert bleiben.");
const extractedPath = resolve(directory, "extracted.xml");
await rm(extractedPath, { force: true });
const extraction = spawnSync(java, [
  "-Djava.awt.headless=true", "-jar", resolve(bundle, manifest.mustang.jar),
  "--action", "extract", "--source", path, "--out", extractedPath, "--disable-file-logging",
], { windowsHide: true, timeout: 60_000, encoding: "utf8", env: { ...process.env, JAVA_TOOL_OPTIONS: undefined } });
assert.equal(extraction.status, 0, extraction.error?.message ?? extraction.stderr);
assert.deepEqual(await readFile(extractedPath), Buffer.from(xml), "Auch Mustang muss dieselben XML-Bytes extrahieren.");
console.log("PASS bytegleicher Mustang-XML-Extrakt");
const validators = [
  new VeraPdfValidator("verapdf", java, resolve(bundle, manifest.verapdf.jar)),
  new MustangValidator(resolve(bundle, manifest.mustang.jar), java),
];
const reports = [];
for (const validator of validators) {
  const result = await validator.validate(path);
  reports.push({ engine: validator.engine, ...result });
  console.log(`${result.valid ? "PASS" : "FAIL"} ${validator.name}`);
  for (const issue of result.issues) console.log(`  ${issue.code}: ${issue.message}`);
}
await writeFile(resolve(directory, "report.json"), JSON.stringify(reports, null, 2));
if (reports.some(report => !report.valid)) process.exitCode = 1;
