import { access } from "node:fs/promises";
import { resolve } from "node:path";
import { MustangValidator } from "../src/engine/validators.js";

const jar = process.env.MUSTANG_VALIDATOR_JAR;
const documents = process.argv.slice(2).map((path) => resolve(path));
if (!jar || documents.length === 0) {
  console.error("Usage: set MUSTANG_VALIDATOR_JAR, then npm run validate:zugferd -- invoice.xml-or-pdf [...]");
  process.exit(2);
}
await Promise.all([access(jar), ...documents.map(access)]);
const validator = new MustangValidator(jar);
let failed = false;
for (const document of documents) {
  const result = await validator.validate(document);
  console.log(`${result.valid ? "PASS" : "FAIL"} ${document}`);
  for (const issue of result.issues) console.error(`  ${issue.code}: ${issue.message}`);
  failed ||= !result.valid;
}
process.exitCode = failed ? 1 : 0;
