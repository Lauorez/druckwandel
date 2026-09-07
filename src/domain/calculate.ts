import { Decimal } from "decimal.js";
import { decimal, money, rate } from "./money.js";
import type { CalculatedInvoice, InvoiceInput, TaxBreakdown } from "./types.js";

export function calculateInvoice(input: InvoiceInput): CalculatedInvoice {
  const calculatedLines = input.lines.map((line) => ({
    ...line,
    netAmount: money(decimal(line.quantity).mul(decimal(line.netUnitPrice))),
  }));

  const taxGroups = new Map<string, { taxable: Decimal; category: TaxBreakdown["categoryCode"]; taxRate: string; exemptionReason?: string }>();
  for (const line of calculatedLines) {
    const normalizedRate = rate(line.tax.rate);
    const key = `${line.tax.categoryCode}|${normalizedRate}|${line.tax.exemptionReason ?? ""}`;
    const current = taxGroups.get(key);
    if (current) current.taxable = current.taxable.add(line.netAmount);
    else taxGroups.set(key, {
      taxable: decimal(line.netAmount),
      category: line.tax.categoryCode,
      taxRate: normalizedRate,
      ...(line.tax.exemptionReason ? { exemptionReason: line.tax.exemptionReason } : {}),
    });
  }

  const taxes = [...taxGroups.values()].map((group) => ({
    categoryCode: group.category,
    rate: group.taxRate,
    taxableAmount: money(group.taxable),
    taxAmount: money(group.taxable.mul(group.taxRate).div(100)),
    ...(group.exemptionReason ? { exemptionReason: group.exemptionReason } : {}),
  }));
  const lineNet = calculatedLines.reduce((sum, line) => sum.add(line.netAmount), new Decimal(0));
  const taxTotal = taxes.reduce((sum, tax) => sum.add(tax.taxAmount), new Decimal(0));
  const gross = lineNet.add(taxTotal);

  return {
    ...input,
    calculatedLines,
    taxes,
    totals: {
      lineNet: money(lineNet),
      taxExclusive: money(lineNet),
      taxTotal: money(taxTotal),
      taxInclusive: money(gross),
      payable: money(gross),
    },
  };
}
