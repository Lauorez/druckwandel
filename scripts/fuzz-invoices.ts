import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import {
  createCorpusReport,
  evaluateExtraction,
  type CaseEvaluation,
} from "../src/evaluation/corpus.js";
import { generateSyntheticInvoice, type SyntheticLayout } from "../src/evaluation/synthetic-invoice.js";
import { extractInvoicePdf } from "../src/extraction/index.js";

function argument(name: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function integerArgument(name: string, fallback: number, minimum: number, maximum: number): number {
  const raw = argument(name);
  if (raw === undefined) return fallback;
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new Error(`${name} muss eine ganze Zahl zwischen ${minimum} und ${maximum} sein.`);
  }
  return value;
}

const count = integerArgument("--count", 250, 1, 10_000);
const startSeed = integerArgument("--seed", 1_000, 0, Number.MAX_SAFE_INTEGER - count);
const reportPath = resolve(argument("--report") ?? "artifacts/synthetic-fuzz-report.json");
const failureDirectory = resolve(argument("--failures") ?? "artifacts/fuzz-failures");
const evaluations: CaseEvaluation[] = [];
const layoutCounts: Record<SyntheticLayout, number> = {
  "headed-separated": 0,
  "headed-merged": 0,
  "legacy-columns": 0,
};
const failedSeeds: number[] = [];
const startedAt = Date.now();

for (let offset = 0; offset < count; offset += 1) {
  const seed = startSeed + offset;
  const fixture = await generateSyntheticInvoice(seed);
  layoutCounts[fixture.source.layout] += 1;
  let evaluation: CaseEvaluation;
  try {
    const extraction = await extractInvoicePdf(fixture.pdf);
    evaluation = evaluateExtraction(fixture.corpusCase, extraction);
  } catch (error) {
    evaluation = {
      id: fixture.corpusCase.id,
      description: fixture.corpusCase.description,
      passed: false,
      checks: [{
        path: "extraction",
        expected: "successful extraction",
        actual: error instanceof Error ? error.message : String(error),
        passed: false,
      }],
      metrics: {
        expectedFields: Object.keys(fixture.corpusCase.expected.fields ?? {}).length,
        matchedFields: 0,
        expectedLineItems: fixture.corpusCase.expected.lineItems?.length ?? 0,
        matchedLineItems: 0,
      },
    };
  }
  evaluations.push(evaluation);

  if (!evaluation.passed) {
    failedSeeds.push(seed);
    await mkdir(failureDirectory, { recursive: true });
    const prefix = join(failureDirectory, `seed-${seed}`);
    await Promise.all([
      writeFile(`${prefix}.pdf`, fixture.pdf),
      writeFile(`${prefix}.source.json`, `${JSON.stringify(fixture.source, null, 2)}\n`, "utf8"),
      writeFile(`${prefix}.evaluation.json`, `${JSON.stringify(evaluation, null, 2)}\n`, "utf8"),
    ]);
    console.log(`FAIL  Seed ${seed} (${fixture.source.layout})`);
    for (const check of evaluation.checks.filter((candidate) => !candidate.passed)) {
      console.log(`      ${check.path}: erwartet ${JSON.stringify(check.expected)}, erhalten ${JSON.stringify(check.actual)}`);
    }
  } else if ((offset + 1) % 25 === 0 || offset + 1 === count) {
    console.log(`PASS  ${offset + 1}/${count} Seeds geprüft (zuletzt ${seed})`);
  }
}

const report = {
  ...createCorpusReport(`synthetic://seeds/${startSeed}-${startSeed + count - 1}`, evaluations),
  fuzz: {
    startSeed,
    count,
    durationMs: Date.now() - startedAt,
    layoutCounts,
    failedSeeds,
    failureDirectory: relative(process.cwd(), failureDirectory),
  },
};
await mkdir(dirname(reportPath), { recursive: true });
await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");

console.log(`\nFuzz-Test: ${report.totals.passedCases}/${report.totals.cases} Rechnungen, ${report.totals.matchedFields}/${report.totals.expectedFields} Felder, ${report.totals.matchedLineItems}/${report.totals.expectedLineItems} Positionen.`);
console.log(`Layouts: getrennt ${layoutCounts["headed-separated"]}, zusammengeführt ${layoutCounts["headed-merged"]}, Legacy ${layoutCounts["legacy-columns"]}.`);
console.log(`Report: ${reportPath}`);
if (!report.passed) {
  console.log(`Fehlerartefakte: ${failureDirectory}`);
  process.exitCode = 1;
}
