import { access } from "node:fs/promises";
import { resolve } from "node:path";
import { KositValidator } from "../src/engine/validators.js";

const jar = process.env.KOSIT_VALIDATOR_JAR;
const scenarios = process.env.KOSIT_SCENARIOS;
const documents = process.argv.slice(2).map((path) => resolve(path));
if (!jar || !scenarios || documents.length === 0) {
  console.error("Usage: set KOSIT_VALIDATOR_JAR and KOSIT_SCENARIOS, then npm run validate:external -- invoice.xml [...]");
  process.exit(2);
}
await Promise.all([access(jar), access(scenarios), ...documents.map(access)]);
const validator = new KositValidator(jar, scenarios, undefined, process.env.KOSIT_REPORT_DIR);
let failed = false;
for (const document of documents) {
  const result = await validator.validate(document);
  console.log(`${result.valid ? "PASS" : "FAIL"} ${document}`);
  for (const issue of result.issues) console.error(`  ${issue.code}: ${issue.message}`);
  failed ||= !result.valid;
}
process.exitCode = failed ? 1 : 0;
