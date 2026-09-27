import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { calculateInvoice } from "../src/domain/calculate.js";
import { generateCii } from "../src/engine/cii.js";
import { generateUbl } from "../src/engine/ubl.js";
import { KositValidator, MustangValidator } from "../src/engine/validators.js";
import { exportInvoices } from "../test/fixtures/export-invoices.js";

const bundle = resolve("apps/desktop/src-tauri/resources/validators");
const manifest = JSON.parse(await readFile(resolve(bundle, "manifest.json"), "utf8"));
const kositJar = process.env.KOSIT_VALIDATOR_JAR ?? resolve(bundle, manifest.kosit.jar);
const scenarios = process.env.KOSIT_SCENARIOS ?? resolve(bundle, manifest.kosit.scenarios);
const mustangJar = process.env.MUSTANG_VALIDATOR_JAR ?? resolve(bundle, manifest.mustang.jar);
const java = process.env.VALIDATOR_JAVA ?? resolve(bundle, `${manifest.java.relativePath}${process.platform === "win32" ? ".exe" : ""}`);
const directory = resolve("artifacts/xml-regressions");
await mkdir(directory, { recursive: true });
const validators = [
  { format: "ubl", generate: generateUbl, validator: new KositValidator(resolve(kositJar), resolve(scenarios), undefined, undefined, java) },
  { format: "cii", generate: generateCii, validator: new MustangValidator(resolve(mustangJar), java) },
];
const reports = [];
for (const [name, input] of Object.entries(exportInvoices)) {
  const invoice = calculateInvoice(input);
  for (const { format, generate, validator } of validators) {
    const path = resolve(directory, `${name}-${format}.xml`);
    await writeFile(path, generate(invoice));
    const result = await validator.validate(path);
    reports.push({ name, format, ...result });
    console.log(`${result.valid ? "PASS" : "FAIL"} ${name} ${format}`);
    for (const issue of result.issues) console.log(`  ${issue.code}: ${issue.message}`);
  }
}
await writeFile(resolve(directory, "report.json"), JSON.stringify({ tools: { java, kositJar, scenarios, mustangJar }, reports }, null, 2));
if (reports.some(report => !report.valid)) process.exitCode = 1;
