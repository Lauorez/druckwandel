import { calculateInvoice, documentAdjustmentTotals, lineNetAmount, prepaidAmountOf } from "./calculate.js";
import { decimal, money } from "./money.js";
import { isCreditOrCorrection, isInvoiceTypeCode, resolvedPrecedingInvoices, type AllowanceCharge, type CalculatedInvoice, type InvoiceInput, type PrecedingInvoiceReference, type ValidationIssue, type ValidationResult } from "./types.js";

const ISO_CURRENCY = /^[A-Z]{3}$/;
const COUNTRY = /^[A-Z]{2}$/;

export function isIsoDate(value: string): boolean {
  const match = value.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return false;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (year < 1 || month < 1 || month > 12 || day < 1) return false;
  const daysInMonth = [31, year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return day <= (daysInMonth[month - 1] ?? 0);
}

export function validateEn16931InvoiceInput(invoice: InvoiceInput): ValidationResult {
  const issues: ValidationIssue[] = [];
  const required = (value: string | undefined, path: string, code: string) => {
    if (!value?.trim()) issues.push({ severity: "error", code, path, message: "Bitte tragen Sie diese Angabe ein." });
  };
  required(invoice.invoiceNumber, "invoiceNumber", "BR-02");
  required(invoice.seller.name, "seller.name", "BR-06");
  required(invoice.buyer.name, "buyer.name", "BR-07");
  if (!isIsoDate(invoice.issueDate)) issues.push({ severity: "error", code: "DATE", path: "issueDate", message: "Bitte wählen Sie ein gültiges Datum." });
  if (invoice.dueDate && !isIsoDate(invoice.dueDate)) issues.push({ severity: "error", code: "DATE", path: "dueDate", message: "Bitte wählen Sie ein gültiges Datum." });
  if (invoice.serviceDate && !isIsoDate(invoice.serviceDate)) issues.push({ severity: "error", code: "DATE", path: "serviceDate", message: "Bitte wählen Sie ein gültiges Leistungsdatum." });
  if (invoice.deliveryAddress && !COUNTRY.test(invoice.deliveryAddress.countryCode)) issues.push({ severity: "error", code: "COUNTRY", path: "deliveryAddress.countryCode", message: "Bitte tragen Sie das Lieferland mit zwei Buchstaben ein, zum Beispiel AT." });
  if (!ISO_CURRENCY.test(invoice.currency)) issues.push({ severity: "error", code: "BR-05", path: "currency", message: "Bitte tragen Sie eine gültige Währung ein, zum Beispiel EUR." });
  for (const [partyName, party] of [["seller", invoice.seller], ["buyer", invoice.buyer]] as const) {
    required(party.address.line1, `${partyName}.address.line1`, "BR-10");
    required(party.address.city, `${partyName}.address.city`, "BR-10");
    required(party.address.postalCode, `${partyName}.address.postalCode`, "BR-10");
    if (!COUNTRY.test(party.address.countryCode)) issues.push({ severity: "error", code: "COUNTRY", path: `${partyName}.address.countryCode`, message: "Bitte tragen Sie das Land mit zwei Buchstaben ein, zum Beispiel DE." });
  }
  if (!isInvoiceTypeCode(invoice.invoiceType)) {
    issues.push({ severity: "error", code: "BT-3", path: "invoiceType", message: "Bitte wählen Sie Rechnung, Gutschrift, Korrektur, Abschlag, Anzahlung oder Schlussrechnung." });
  }
  if (invoice.finalInvoice && invoice.prepaymentInvoice) {
    issues.push({ severity: "error", code: "BT-3", path: "invoiceType", message: "Eine Rechnung kann nicht zugleich Anzahlung und Schlussrechnung sein." });
  }
  if (invoice.finalInvoice && invoice.invoiceType !== "380") {
    issues.push({ severity: "error", code: "BT-3", path: "invoiceType", message: "Eine Schlussrechnung bleibt eine Rechnung (380) mit bisherigen Abschlägen, keine eigene Gutschrift." });
  }
  if (invoice.prepaymentInvoice && invoice.invoiceType !== "380") {
    issues.push({ severity: "error", code: "BT-3", path: "invoiceType", message: "Eine Anzahlungsrechnung bleibt eine Rechnung (380). XRechnung lässt den Code 386 nicht zu." });
  }
  validatePrecedingInvoices(issues, invoice);
  validatePrepaidAmount(issues, invoice);
  if (invoice.lines.length === 0) issues.push({ severity: "error", code: "BR-16", path: "lines", message: "Mindestens eine Rechnungsposition ist erforderlich." });
  invoice.lines.forEach((line, index) => {
    required(line.id, `lines.${index}.id`, "BR-21");
    required(line.name, `lines.${index}.name`, "BR-25");
    try {
      if (decimal(line.quantity).lte(0)) throw new Error();
    } catch { issues.push({ severity: "error", code: "QUANTITY", path: `lines.${index}.quantity`, message: "Bitte tragen Sie eine Menge größer als 0 ein." }); }
    try {
      if (decimal(line.netUnitPrice).lt(0)) throw new Error();
    } catch { issues.push({ severity: "error", code: "PRICE", path: `lines.${index}.netUnitPrice`, message: "Bitte tragen Sie einen Preis ab 0 ein." }); }
    try { decimal(line.tax.rate); }
    catch { issues.push({ severity: "error", code: "RATE", path: `lines.${index}.tax.rate`, message: "Bitte tragen Sie einen gültigen Steuersatz ein." }); }
    validateTaxTreatment(issues, line.tax, `lines.${index}.tax`, invoice);
    (line.allowances ?? []).forEach((item, allowanceIndex) => {
      validateAllowance(issues, item, `lines.${index}.allowances.${allowanceIndex}`, item.charge ? "BR-42" : "BR-41", invoice);
    });
  });
  (invoice.allowances ?? []).forEach((item, index) => {
    validateAllowance(issues, item, `allowances.${index}`, item.charge ? "BR-36" : "BR-31", invoice);
  });
  return { valid: !issues.some((issue) => issue.severity === "error"), issues };
}

export function validateInvoiceInput(invoice: InvoiceInput): ValidationResult {
  const base = validateEn16931InvoiceInput(invoice);
  const issues = [...base.issues];
  if (invoice.deliveryAddress) {
    if (!invoice.deliveryAddress.city.trim()) issues.push({ severity: "error", code: "BR-DE-10", path: "deliveryAddress.city", message: "Für Behörden wird der Ort der Lieferanschrift benötigt." });
    if (!invoice.deliveryAddress.postalCode.trim()) issues.push({ severity: "error", code: "BR-DE-11", path: "deliveryAddress.postalCode", message: "Für Behörden wird die Postleitzahl der Lieferanschrift benötigt." });
  }
  if (!invoice.buyerReference.trim()) {
    issues.push({ severity: "error", code: "BR-DE-15", path: "buyerReference", message: "Bitte tragen Sie die Bestellnummer oder Leitweg-ID der Behörde ein." });
  }
  if (!invoice.seller.contact?.name?.trim() || !invoice.seller.contact.phone?.trim() || !invoice.seller.contact.email?.trim()) {
    issues.push({ severity: "error", code: "BR-DE-2", path: "seller.contact.name", message: "Für Behörden werden Ansprechpartner, Telefon und E-Mail des Absenders benötigt." });
  }
  if (!invoice.seller.electronicAddress?.value) {
    issues.push({ severity: "error", code: "BT-34", path: "seller.vatId", message: "Für Behörden wird eine elektronische Adresse des Absenders benötigt. Tragen Sie die Umsatzsteuer-ID ein." });
  }
  if (!invoice.buyer.electronicAddress?.value) {
    issues.push({ severity: "error", code: "BT-49", path: "buyer.vatId", message: "Für Behörden wird eine elektronische Adresse des Empfängers benötigt. Tragen Sie die Umsatzsteuer-ID oder eine Leitweg-ID ein." });
  }
  return { valid: !issues.some((issue) => issue.severity === "error"), issues };
}

export function validateCalculatedInvoice(invoice: CalculatedInvoice): ValidationResult {
  const base = validateInvoiceInput(invoice);
  return validateCalculatedTotals(invoice, base.issues);
}

export function validateEn16931CalculatedInvoice(invoice: CalculatedInvoice): ValidationResult {
  const base = validateEn16931InvoiceInput(invoice);
  return validateCalculatedTotals(invoice, base.issues);
}

function validateAllowance(issues: ValidationIssue[], item: AllowanceCharge, path: string, amountCode: string, invoice: InvoiceInput) {
  try {
    if (decimal(item.amount).lte(0)) throw new Error();
    if (decimal(item.amount).decimalPlaces() > 2) {
      issues.push({ severity: "error", code: "AMOUNT_PRECISION", path: `${path}.amount`, message: "Zu- und Abschläge dürfen höchstens zwei Nachkommastellen haben." });
    }
  } catch {
    issues.push({ severity: "error", code: amountCode, path: `${path}.amount`, message: "Bitte tragen Sie einen Betrag größer als 0 ein." });
  }
  try { decimal(item.tax.rate); }
  catch { issues.push({ severity: "error", code: "RATE", path: `${path}.tax.rate`, message: "Bitte tragen Sie einen gültigen Steuersatz ein." }); }
  validateTaxTreatment(issues, item.tax, `${path}.tax`, invoice);
}

function validateTaxTreatment(
  issues: ValidationIssue[],
  tax: AllowanceCharge["tax"],
  path: string,
  invoice: InvoiceInput,
) {
  let taxRate;
  try { taxRate = decimal(tax.rate); }
  catch { return; }
  if (tax.categoryCode === "S") {
    if (taxRate.lte(0)) {
      issues.push({
        severity: "error",
        code: "BR-S-5",
        path: `${path}.rate`,
        message: "Standardumsatzsteuer braucht einen Satz größer als 0 %. 0 % allein entscheidet nicht zwischen Reverse Charge, Steuerfreiheit und innergemeinschaftlicher Lieferung.",
      });
    }
    return;
  }
  if (["Z", "E", "AE", "K"].includes(tax.categoryCode) && !taxRate.eq(0)) {
    const code = tax.categoryCode === "AE" ? "BR-AE-5" : tax.categoryCode === "K" ? "BR-IC-5" : tax.categoryCode === "E" ? "BR-E-5" : "BR-Z-5";
    issues.push({ severity: "error", code, path: `${path}.rate`, message: "Dieser Steuerfall gilt nur mit 0 %." });
  }
  if (["E", "AE", "K"].includes(tax.categoryCode) && !tax.exemptionReason?.trim()) {
    const code = tax.categoryCode === "AE" ? "BR-AE-10" : tax.categoryCode === "K" ? "BR-IC-11" : "BR-E-10";
    issues.push({ severity: "error", code, path: `${path}.exemptionReason`, message: "Bitte tragen Sie ein, warum keine Umsatzsteuer berechnet wird." });
  }
  if (tax.categoryCode === "AE" || tax.categoryCode === "K") {
    const sellerCode = tax.categoryCode === "AE" ? "BR-AE-1" : "BR-IC-1";
    const buyerCode = tax.categoryCode === "AE" ? "BR-AE-2" : "BR-IC-2";
    const label = tax.categoryCode === "AE" ? "Reverse Charge" : "innergemeinschaftliche Lieferungen";
    if (!invoice.seller.vatId?.trim() && !issues.some((issue) => issue.code === sellerCode)) {
      issues.push({ severity: "error", code: sellerCode, path: "seller.vatId", message: `Für ${label} wird die Umsatzsteuer-ID des Absenders benötigt.` });
    }
    if (!invoice.buyer.vatId?.trim() && !issues.some((issue) => issue.code === buyerCode)) {
      issues.push({ severity: "error", code: buyerCode, path: "buyer.vatId", message: `Für ${label} wird die Umsatzsteuer-ID des Empfängers benötigt.` });
    }
  }
  if (tax.categoryCode === "K") {
    if (!invoice.serviceDate && !issues.some(issue => issue.code === "BR-IC-11")) {
      issues.push({ severity: "error", code: "BR-IC-11", path: "serviceDate", message: "Für innergemeinschaftliche Lieferungen wird das Liefer-/Leistungsdatum benötigt." });
    }
    if (!invoice.deliveryAddress?.countryCode && !issues.some(issue => issue.code === "BR-IC-12")) {
      issues.push({ severity: "error", code: "BR-IC-12", path: "deliveryAddress.countryCode", message: "Bitte tragen Sie das tatsächliche Lieferland ein. Die Rechnungsanschrift legt das Lieferland nicht fest." });
    }
  }
}

function validateCalculatedTotals(invoice: CalculatedInvoice, baseIssues: ValidationIssue[]): ValidationResult {
  if (baseIssues.some(issue => issue.severity === "error")) return { valid: false, issues: baseIssues };
  try {
    return checkCalculatedTotals(invoice, baseIssues);
  } catch {
    return { valid: false, issues: [...baseIssues, { severity: "error", code: "CALCULATION", path: "totals", message: "Die berechneten Beträge sind ungültig oder unvollständig." }] };
  }
}

function checkCalculatedTotals(invoice: CalculatedInvoice, baseIssues: ValidationIssue[]): ValidationResult {
  const issues = [...baseIssues];
  const expected = calculateInvoice(invoice);
  if (invoice.calculatedLines.length !== expected.calculatedLines.length
    || invoice.calculatedLines.some((line, index) => line.id !== expected.calculatedLines[index]?.id
      || !decimal(line.netAmount).eq(expected.calculatedLines[index]!.netAmount))
    || !decimal(invoice.totals.lineNet).eq(expected.totals.lineNet)) {
    issues.push({ severity: "error", code: "BR-CO-10", path: "totals.lineNet", message: "Die berechneten Positionen und ihre Summe passen nicht zu den eingegebenen Positionen." });
  }
  const seenTaxGroups = new Set<string>();
  if (invoice.taxes.length !== expected.taxes.length || invoice.taxes.some(tax => {
    const key = `${tax.categoryCode}|${decimal(tax.rate).toFixed()}`;
    const duplicate = seenTaxGroups.has(key);
    seenTaxGroups.add(key);
    const group = expected.taxes.find(item => item.categoryCode === tax.categoryCode && decimal(item.rate).eq(tax.rate));
    return duplicate || !group || !decimal(tax.taxableAmount).eq(group.taxableAmount) || !decimal(tax.taxAmount).eq(group.taxAmount);
  })) {
    issues.push({ severity: "error", code: "BR-CO-17", path: "taxes", message: "Steuergruppen, Bemessungsgrundlagen und Steuerbeträge passen nicht zu den Positionen und Zu-/Abschlägen." });
  }
  invoice.calculatedLines.forEach((line, index) => {
    try {
      if (!decimal(line.netAmount).eq(money(lineNetAmount(line))) || decimal(line.netAmount).lt(0)) throw new Error();
    } catch {
      issues.push({ severity: "error", code: "BR-CO-10", path: `lines.${index}.netAmount`, message: "Der Positionsbetrag passt nicht zu Menge, Preis und Zu-/Abschlägen." });
    }
  });
  const { allowanceTotal, chargeTotal } = documentAdjustmentTotals(invoice.allowances);
  if (!decimal(invoice.totals.lineNet).sub(allowanceTotal).add(chargeTotal).eq(invoice.totals.taxExclusive)) {
    issues.push({ severity: "error", code: "BR-CO-13", path: "totals.taxExclusive", message: "Der Nettobetrag passt nicht zu Positionen und Zu-/Abschlägen." });
  }
  if (invoice.taxes.some((tax) => decimal(tax.taxableAmount).lt(0))) {
    issues.push({ severity: "error", code: "BR-CO-18", path: "allowances", message: "Nachlässe dürfen die Steuerbemessungsgrundlage nicht unter 0 drücken." });
  }
  if (invoice.taxes.some((tax) => ["Z", "E", "AE", "K"].includes(tax.categoryCode) && !decimal(tax.taxAmount).eq(0))) {
    issues.push({ severity: "error", code: "BR-CO-17", path: "totals.taxTotal", message: "Bei steuerfreien Fällen muss die Umsatzsteuer 0,00 betragen." });
  }
  const expectedTax = invoice.taxes.reduce((sum, tax) => sum.add(tax.taxAmount), decimal(0));
  if (!expectedTax.eq(invoice.totals.taxTotal)) issues.push({ severity: "error", code: "BR-CO-14", path: "totals.taxTotal", message: "Die Umsatzsteuer passt nicht zu den Rechnungspositionen." });
  if (!decimal(invoice.totals.taxExclusive).add(invoice.totals.taxTotal).eq(invoice.totals.taxInclusive)) issues.push({ severity: "error", code: "BR-CO-15", path: "totals.taxInclusive", message: "Der Rechnungsbetrag passt nicht zu Nettobetrag und Umsatzsteuer." });
  const prepaid = prepaidAmountOf(invoice);
  if (prepaid.gt(invoice.totals.taxInclusive)) {
    issues.push({ severity: "error", code: "BT-113", path: "prepaidAmount", message: "Der bereits gezahlte Betrag darf den Rechnungsbetrag nicht übersteigen." });
  }
  if (!decimal(invoice.totals.taxInclusive).sub(prepaid).eq(invoice.totals.payable)) {
    issues.push({
      severity: "error",
      code: "BR-CO-16",
      path: "totals.payable",
      message: prepaid.gt(0)
        ? "Der Zahlbetrag muss dem Rechnungsbetrag abzüglich der bereits gezahlten Beträge entsprechen."
        : "Der zu zahlende Betrag stimmt nicht mit dem Rechnungsbetrag überein.",
    });
  }
  return { valid: !issues.some((issue) => issue.severity === "error"), issues };
}

function validatePrecedingInvoices(issues: ValidationIssue[], invoice: InvoiceInput) {
  const refs = resolvedPrecedingInvoices(invoice);
  const required = (value: string | undefined, path: string, code: string, message: string) => {
    if (!value?.trim()) issues.push({ severity: "error", code, path, message });
  };
  const checkDate = (item: PrecedingInvoiceReference, path: string, message: string) => {
    if (item.issueDate && !isIsoDate(item.issueDate)) {
      issues.push({ severity: "error", code: "BT-26", path, message });
    }
  };
  if (isCreditOrCorrection(invoice.invoiceType)) {
    required(refs[0]?.invoiceNumber, "precedingInvoice.invoiceNumber", "BT-25", "Bitte tragen Sie diese Angabe ein.");
    if (refs[0]) checkDate(refs[0], "precedingInvoice.issueDate", "Bitte wählen Sie ein gültiges Datum der Ursprungsrechnung.");
    return;
  }
  refs.forEach((item, index) => {
    required(item.invoiceNumber, `precedingInvoices.${index}.invoiceNumber`, "BT-25", "Bitte tragen Sie die Nummer der bisherigen Rechnung ein.");
    checkDate(item, `precedingInvoices.${index}.issueDate`, "Bitte wählen Sie ein gültiges Datum der bisherigen Rechnung.");
    if (item.paidAmount?.trim()) validateMoneyAmount(issues, item.paidAmount, `precedingInvoices.${index}.paidAmount`, "Bitte tragen Sie den bereits berechneten Betrag mit höchstens zwei Nachkommastellen ein.");
  });
}

function validatePrepaidAmount(issues: ValidationIssue[], invoice: InvoiceInput) {
  const refs = resolvedPrecedingInvoices(invoice);
  let prepaid: ReturnType<typeof prepaidAmountOf> | undefined;
  try {
    prepaid = prepaidAmountOf(invoice);
  } catch {
    issues.push({ severity: "error", code: "BT-113", path: "prepaidAmount", message: "Bitte tragen Sie den bereits gezahlten Betrag mit höchstens zwei Nachkommastellen ein." });
    return;
  }
  if (invoice.prepaidAmount?.trim()) {
    validateMoneyAmount(issues, invoice.prepaidAmount, "prepaidAmount", "Bitte tragen Sie den bereits gezahlten Betrag mit höchstens zwei Nachkommastellen ein.");
  }
  const fromRefs = refs.reduce((sum, item) => {
    if (!item.paidAmount?.trim()) return sum;
    try { return sum.add(decimal(item.paidAmount)); } catch { return sum; }
  }, decimal(0));
  if (invoice.prepaidAmount?.trim() && fromRefs.gt(0) && !decimal(invoice.prepaidAmount).eq(fromRefs)) {
    issues.push({ severity: "error", code: "BT-113", path: "prepaidAmount", message: "Die Summe der Beträge der bisherigen Rechnungen muss dem bereits gezahlten Gesamtbetrag entsprechen." });
  }
  if (prepaid.gt(0) && refs.length === 0) {
    issues.push({ severity: "error", code: "BT-25", path: "precedingInvoices.0.invoiceNumber", message: "Zu bereits gezahlten Beträgen gehören die Nummern der bisherigen Abschlags- oder Anzahlungsrechnungen." });
  }
  if (invoice.finalInvoice && prepaid.lte(0)) {
    issues.push({ severity: "error", code: "BT-113", path: "prepaidAmount", message: "Eine Schlussrechnung braucht die bereits berechneten Beträge der bisherigen Abschläge oder Anzahlungen." });
  }
  if (invoice.finalInvoice && refs.length === 0) {
    issues.push({ severity: "error", code: "BT-25", path: "precedingInvoices.0.invoiceNumber", message: "Eine Schlussrechnung braucht die Nummern der bisherigen Abschlags- oder Anzahlungsrechnungen." });
  }
}

function validateMoneyAmount(issues: ValidationIssue[], value: string, path: string, message: string) {
  try {
    const amount = decimal(value);
    if (amount.lt(0) || amount.decimalPlaces() > 2) throw new Error();
  } catch {
    if (!issues.some((issue) => issue.path === path)) issues.push({ severity: "error", code: "BT-113", path, message });
  }
}

export function assertValidInvoice(invoice: CalculatedInvoice): void {
  const result = validateCalculatedInvoice(invoice);
  if (!result.valid) throw new Error(result.issues.map((issue) => `${issue.code} ${issue.path}: ${issue.message}`).join("\n"));
}

export function assertValidEn16931Invoice(invoice: CalculatedInvoice): void {
  const result = validateEn16931CalculatedInvoice(invoice);
  if (!result.valid) throw new Error(result.issues.map((issue) => `${issue.code} ${issue.path}: ${issue.message}`).join("\n"));
}
