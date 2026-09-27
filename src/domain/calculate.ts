import { Decimal } from "decimal.js";
import { decimal, money, rate } from "./money.js";
import { resolvedPrecedingInvoices, type AllowanceCharge, type CalculatedInvoice, type InvoiceInput, type InvoiceLine, type TaxBreakdown } from "./types.js";

export function lineNetAmount(line: InvoiceLine): Decimal {
  const base = decimal(line.quantity).mul(decimal(line.netUnitPrice));
  return (line.allowances ?? []).reduce((sum, item) => item.charge ? sum.add(item.amount) : sum.sub(item.amount), base);
}

export function documentAdjustmentTotals(allowances: AllowanceCharge[] | undefined): { allowanceTotal: Decimal; chargeTotal: Decimal } {
  return (allowances ?? []).reduce((totals, item) => {
    if (item.charge) totals.chargeTotal = totals.chargeTotal.add(item.amount);
    else totals.allowanceTotal = totals.allowanceTotal.add(item.amount);
    return totals;
  }, { allowanceTotal: new Decimal(0), chargeTotal: new Decimal(0) });
}

export function prepaidAmountOf(invoice: InvoiceInput): Decimal {
  if (invoice.prepaidAmount?.trim()) return decimal(invoice.prepaidAmount);
  return resolvedPrecedingInvoices(invoice).reduce((sum, item) => {
    if (!item.paidAmount?.trim()) return sum;
    return sum.add(decimal(item.paidAmount));
  }, new Decimal(0));
}

export function hasPrepaidAmount(invoice: InvoiceInput): boolean {
  try {
    return prepaidAmountOf(invoice).gt(0);
  } catch {
    return Boolean(invoice.prepaidAmount?.trim());
  }
}

function taxKey(tax: AllowanceCharge["tax"]): string {
  return `${tax.categoryCode}|${rate(tax.rate)}`;
}

export function calculateInvoice(input: InvoiceInput): CalculatedInvoice {
  const calculatedLines = input.lines.map((line) => ({
    ...line,
    netAmount: money(lineNetAmount(line)),
  }));

  const taxGroups = new Map<string, { taxable: Decimal; category: TaxBreakdown["categoryCode"]; taxRate: string; exemptionReason?: string; exemptionReasonCode?: string }>();
  const addTaxable = (tax: AllowanceCharge["tax"], amount: Decimal) => {
    const key = taxKey(tax);
    const current = taxGroups.get(key);
    if (current) current.taxable = current.taxable.add(amount);
    else taxGroups.set(key, {
      taxable: amount,
      category: tax.categoryCode,
      taxRate: rate(tax.rate),
      ...(tax.exemptionReason ? { exemptionReason: tax.exemptionReason } : {}),
      ...(tax.exemptionReasonCode ? { exemptionReasonCode: tax.exemptionReasonCode } : {}),
    });
  };
  for (const line of calculatedLines) addTaxable(line.tax, decimal(line.netAmount));
  for (const item of input.allowances ?? []) addTaxable(item.tax, item.charge ? decimal(item.amount) : decimal(item.amount).neg());

  const taxes = [...taxGroups.values()].map((group) => ({
    categoryCode: group.category,
    rate: group.taxRate,
    taxableAmount: money(group.taxable),
    taxAmount: money(group.taxable.mul(group.taxRate).div(100)),
    ...(group.exemptionReason ? { exemptionReason: group.exemptionReason } : {}),
    ...(group.exemptionReasonCode ? { exemptionReasonCode: group.exemptionReasonCode } : {}),
  }));
  const lineNet = calculatedLines.reduce((sum, line) => sum.add(line.netAmount), new Decimal(0));
  const { allowanceTotal, chargeTotal } = documentAdjustmentTotals(input.allowances);
  const taxExclusive = lineNet.sub(allowanceTotal).add(chargeTotal);
  const taxTotal = taxes.reduce((sum, tax) => sum.add(tax.taxAmount), new Decimal(0));
  const gross = taxExclusive.add(taxTotal);
  const prepaid = prepaidAmountOf(input);

  return {
    ...input,
    calculatedLines,
    taxes,
    totals: {
      lineNet: money(lineNet),
      taxExclusive: money(taxExclusive),
      taxTotal: money(taxTotal),
      taxInclusive: money(gross),
      payable: money(gross.sub(prepaid)),
    },
  };
}
