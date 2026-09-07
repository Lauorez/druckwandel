import { describe, expect, it } from "vitest";
import { evaluateExtraction } from "../src/evaluation/corpus.js";
import { generateSyntheticInvoice, SeededRandom } from "../src/evaluation/synthetic-invoice.js";
import { extractInvoicePdf } from "../src/extraction/index.js";

describe("synthetic invoice generator", () => {
  it("generates a deterministic source model for a seed", async () => {
    const first = await generateSyntheticInvoice(4_242);
    const second = await generateSyntheticInvoice(4_242);

    expect(second.source).toEqual(first.source);
    expect(second.corpusCase).toEqual(first.corpusCase);
  });

  it("uses a reproducible pseudo-random sequence", () => {
    const first = new SeededRandom(17);
    const second = new SeededRandom(17);
    expect(Array.from({ length: 10 }, () => first.next())).toEqual(Array.from({ length: 10 }, () => second.next()));
  });

  it("round-trips representative PDFs against their independent source models", async () => {
    for (const seed of [1_000, 1_001, 1_015]) {
      const fixture = await generateSyntheticInvoice(seed);
      const extraction = await extractInvoicePdf(fixture.pdf);
      const evaluation = evaluateExtraction(fixture.corpusCase, extraction);
      expect(evaluation.checks.filter((check) => !check.passed), `Seed ${seed}`).toEqual([]);
    }
  });
});
