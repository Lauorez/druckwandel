import { decimal } from "../domain/money.js";
import { classifyFields } from "./classify.js";
import { classifyInvoiceTables } from "./invoice-table.js";
import { reconstructLines } from "./layout.js";
import type { DocumentPage, ExtractionResult, ExtractionWarning } from "./types.js";

export function analyzeDocumentPages(
  pages: DocumentPage[],
  warnings: ExtractionWarning[] = [],
  usedOcr = false,
): ExtractionResult {
  const lines = reconstructLines(pages);
  const fields = classifyFields(lines);
  const table = classifyInvoiceTables(lines, pages);
  Object.assign(fields, table.fields);

  const net = fields.lineNet;
  const tax = fields.taxTotal;
  const gross = fields.taxInclusive ?? fields.payable;
  if (net && tax && gross && !decimal(net.value).add(tax.value).eq(gross.value)) {
    net.confidence = Math.min(net.confidence, 0.6);
    tax.confidence = Math.min(tax.confidence, 0.6);
    gross.confidence = Math.min(gross.confidence, 0.6);
    warnings.push({ code: "TOTALS_MISMATCH", message: "Erkanntes Netto plus Steuer entspricht nicht dem erkannten Rechnungsbetrag; manuelle Prüfung erforderlich." });
  }
  if (net && table.lineItems.length > 0) {
    const lineSum = table.lineItems.reduce((sum, line) => sum.add(line.netAmount), decimal(0));
    if (!lineSum.eq(net.value)) {
      net.confidence = Math.min(net.confidence, 0.6);
      for (const line of table.lineItems) line.confidence = Math.min(line.confidence, 0.6);
      warnings.push({ code: "LINE_TOTAL_MISMATCH", message: `Erkannte Positionssumme ${lineSum.toFixed(2)} entspricht nicht dem erkannten Netto ${net.value}; manuelle Prüfung erforderlich.` });
    }
  }
  return { pages, lines, fields, lineItems: table.lineItems, warnings, usedOcr };
}
