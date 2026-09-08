import { calculateInvoice } from "../domain/calculate.js";
import { assertValidEn16931Invoice } from "../domain/validate.js";
import type { CalculatedInvoice, InvoiceInput } from "../domain/types.js";
import { generateCii } from "./cii.js";
import { prepareHybridPdf } from "./hybrid-pdf.js";
import { generateUbl } from "./ubl.js";

export interface ZugferdPackage {
  pdf: Uint8Array;
  xml: string;
}

export class EInvoiceEngine {
  calculate(input: InvoiceInput): CalculatedInvoice {
    const invoice = calculateInvoice(input);
    assertValidEn16931Invoice(invoice);
    return invoice;
  }

  cii(invoice: CalculatedInvoice): string {
    return generateCii(invoice);
  }

  xrechnung(invoice: CalculatedInvoice): string {
    return generateUbl(invoice);
  }

  async zugferd(sourcePdfA3: Uint8Array, invoice: CalculatedInvoice): Promise<Uint8Array> {
    return prepareHybridPdf(sourcePdfA3, generateCii(invoice));
  }

  async zugferdPackage(sourcePdfA3: Uint8Array, invoice: CalculatedInvoice): Promise<ZugferdPackage> {
    const xml = generateCii(invoice);
    return { pdf: await prepareHybridPdf(sourcePdfA3, xml), xml };
  }
}
