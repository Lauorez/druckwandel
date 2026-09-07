import { mkdir, writeFile } from "node:fs/promises";
import { calculateInvoice } from "../src/domain/calculate.js";
import { generateCii } from "../src/engine/cii.js";
import { generateUbl } from "../src/engine/ubl.js";
import { standardInvoice } from "../test/fixtures/invoice.js";

const directory = new URL("../test/fixtures/generated/", import.meta.url);
await mkdir(directory, { recursive: true });
const invoice = calculateInvoice(standardInvoice);
await Promise.all([
  writeFile(new URL("zugferd-en16931.xml", directory), generateCii(invoice)),
  writeFile(new URL("xrechnung-ubl.xml", directory), generateUbl(invoice)),
]);
console.log("Generated CII and UBL fixtures in test/fixtures/generated");
