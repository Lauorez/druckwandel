import { prepaidAmountOf } from "../domain/calculate.js";
import type { CalculatedInvoice } from "../domain/types.js";
import { decimal, money } from "../domain/money.js";
import type { ExtractionResult } from "../extraction/types.js";
import type { ReviewDraft } from "../review/draft.js";

export type ConsistencyKind = "match" | "supplemented" | "mismatch";

export interface ConsistencyItem {
  path: string;
  label: string;
  kind: ConsistencyKind;
  sourceValue: string;
  outputValue: string;
}

export interface ContentConsistency {
  items: ConsistencyItem[];
  matches: ConsistencyItem[];
  supplemented: ConsistencyItem[];
  mismatches: ConsistencyItem[];
  blocked: boolean;
  needsConfirmation: boolean;
}

function collapse(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

function asMoney(value: string): string | undefined {
  const trimmed = collapse(value);
  if (!trimmed) return undefined;
  try {
    if (/^-?\d{1,3}(\.\d{3})*,\d{1,2}$/.test(trimmed) || /^-?\d+,\d{1,2}$/.test(trimmed)) {
      return money(trimmed.replace(/\./g, "").replace(",", "."));
    }
    return money(trimmed);
  } catch {
    return undefined;
  }
}

function compareText(source: string, output: string): ConsistencyKind | undefined {
  const left = collapse(source);
  const right = collapse(output);
  if (!right) return undefined;
  if (!left) return "supplemented";
  return left.localeCompare(right, "de-DE", { sensitivity: "accent" }) === 0 ? "match" : "mismatch";
}

function compareMoney(source: string, output: string): ConsistencyKind | undefined {
  const right = asMoney(output) ?? collapse(output);
  if (!right) return undefined;
  const left = asMoney(source);
  if (!source.trim()) return "supplemented";
  if (!left) return collapse(source) === collapse(output) ? "match" : "mismatch";
  try {
    return decimal(left).eq(right) ? "match" : "mismatch";
  } catch {
    return "mismatch";
  }
}

function item(path: string, label: string, kind: ConsistencyKind | undefined, sourceValue: string, outputValue: string): ConsistencyItem | undefined {
  if (!kind) return undefined;
  return { path, label, kind, sourceValue, outputValue };
}

function netAmountForSource(source: string, calculated?: CalculatedInvoice): string {
  const taxExclusive = calculated?.totals.taxExclusive ?? "";
  const lineNet = calculated?.totals.lineNet ?? "";
  if (compareMoney(source, lineNet) === "match") return lineNet;
  return taxExclusive || lineNet;
}

export function compareInvoiceToSource(extraction: ExtractionResult, draft: ReviewDraft, calculated?: CalculatedInvoice): ContentConsistency {
  const field = (name: keyof ExtractionResult["fields"]) => extraction.fields[name]?.value ?? "";
  const prepaid = calculated ? prepaidAmountOf(calculated) : undefined;
  const hasPrepaid = Boolean(prepaid?.gt(0));
  const rows: Array<ConsistencyItem | undefined> = [
    item("invoiceNumber", "Rechnungsnummer", compareText(field("invoiceNumber"), draft.invoiceNumber), field("invoiceNumber"), draft.invoiceNumber),
    item("issueDate", "Rechnungsdatum", compareText(field("issueDate"), draft.issueDate), field("issueDate"), draft.issueDate),
    item("seller.name", "Absender", compareText(field("sellerName"), draft.seller.name), field("sellerName"), draft.seller.name),
    item("buyer.name", "Empfänger", compareText(field("buyerName"), draft.buyer.name), field("buyerName"), draft.buyer.name),
    item("totals.lineNet", "Nettobetrag", compareMoney(field("lineNet"), netAmountForSource(field("lineNet"), calculated)), field("lineNet"), netAmountForSource(field("lineNet"), calculated)),
    item("totals.taxTotal", "Umsatzsteuer", compareMoney(field("taxTotal"), calculated?.totals.taxTotal ?? ""), field("taxTotal"), calculated?.totals.taxTotal ?? ""),
    hasPrepaid ? item("totals.taxInclusive", "Rechnungsbetrag", compareMoney(field("taxInclusive"), calculated?.totals.taxInclusive ?? ""), field("taxInclusive"), calculated?.totals.taxInclusive ?? "") : undefined,
    item("totals.payable", hasPrepaid ? "Zahlbetrag" : "Rechnungsbetrag", compareMoney(field("payable") || field("taxInclusive"), calculated?.totals.payable ?? ""), field("payable") || field("taxInclusive"), calculated?.totals.payable ?? ""),
  ];
  if (extraction.lineItems.length > 0 && draft.lines.length !== extraction.lineItems.length) {
    rows.push({
      path: "lines",
      label: "Leistungen und Artikel",
      kind: "mismatch",
      sourceValue: `${extraction.lineItems.length} Positionen`,
      outputValue: `${draft.lines.length} Positionen`,
    });
  } else {
    for (let index = 0; index < extraction.lineItems.length; index += 1) {
      const source = extraction.lineItems[index]!;
      const line = draft.lines[index];
      if (!line) break;
      const amountKind = compareMoney(source.netAmount, (() => {
        try {
          const allowances = line.allowances ?? [];
          const adjustment = allowances.reduce((sum, item) => item.charge ? sum.add(item.amount) : sum.sub(item.amount), decimal(0));
          return money(decimal(line.quantity).mul(line.netUnitPrice).add(adjustment));
        } catch {
          return line.netUnitPrice;
        }
      })());
      rows.push(item(`lines.${index}.name`, `Position ${index + 1}: Bezeichnung`, compareText(source.description, line.description), source.description, line.description));
      rows.push(item(`lines.${index}.netAmount`, `Position ${index + 1}: Betrag`, amountKind, source.netAmount, line.netUnitPrice));
    }
  }
  const items = rows.filter((row): row is ConsistencyItem => Boolean(row));
  const matches = items.filter((row) => row.kind === "match");
  const supplemented = items.filter((row) => row.kind === "supplemented");
  const mismatches = items.filter((row) => row.kind === "mismatch");
  return {
    items,
    matches,
    supplemented,
    mismatches,
    blocked: mismatches.length > 0,
    needsConfirmation: supplemented.length > 0 || matches.length > 0,
  };
}

export function assertExportableConsistency(consistency: ContentConsistency, confirmed: boolean): void {
  if (consistency.blocked) {
    throw new Error("Die Angaben weichen von der Originalrechnung ab. Bitte die Widersprüche auflösen oder die Quelle im Ursprungsprogramm korrigieren. Eine fertige E-Rechnung wird nicht erzeugt.");
  }
  if (consistency.needsConfirmation && !confirmed) {
    throw new Error("Bitte bestätigen Sie, dass die ergänzten Angaben die Originalrechnung korrekt wiedergeben.");
  }
}
