import type { ExtractedField, ExtractedFieldName, TextLine } from "./types.js";
import { parseLocalizedDecimal } from "../domain/localized-decimal.js";
import { money } from "../domain/money.js";

interface Rule {
  name: ExtractedFieldName;
  labels: RegExp;
  value: RegExp;
  confidence: number;
  normalize?: (value: string) => string;
}

const dateValue = "(\\d{1,2}[./-]\\d{1,2}[./-]\\d{2,4}|\\d{4}-\\d{2}-\\d{2})";
const amountValue = "(-?[0-9][0-9. ]*(?:,[0-9]{1,4})|-?[0-9]+(?:\\.[0-9]{1,4})?)";
const rules: Rule[] = [
  { name: "invoiceNumber", labels: /(?:rechnungs(?:nummer|nr\.?|\s*nr\.)|beleg[- ]?nr\.?|invoice\s*(?:number|no\.?))/i, value: /(?:[:#]\s*|\s+)([A-Z0-9][A-Z0-9/_-]{2,})/i, confidence: 0.94 },
  { name: "dueDate", labels: /(?:fällig(?:keit| am)?|zahlbar bis|due date)/i, value: new RegExp(dateValue), confidence: 0.92, normalize: normalizeDate },
  { name: "serviceDate", labels: /(?:leistungsdatum|lieferdatum|delivery date)/i, value: new RegExp(dateValue), confidence: 0.91, normalize: normalizeDate },
  { name: "issueDate", labels: /(?:rechnungsdatum|belegdatum|ausstellungsdatum|invoice date|^\s*datum\s*:)/i, value: new RegExp(dateValue), confidence: 0.88, normalize: normalizeDate },
  { name: "buyerReference", labels: /(?:leitweg[- ]?id|bestell(?:nummer|nr\.?)|buyer reference|käuferreferenz)/i, value: /(?:[:#]\s*|\s+)([A-Z0-9][A-Z0-9._/-]{2,})/i, confidence: 0.9 },
  { name: "sellerVatId", labels: /(?:ust\.?-?id(?:nr)?\.?|umsatzsteuer-id|vat id)/i, value: /\b([A-Z]{2}(?:\s*[A-Z0-9]){8,14})\b/i, confidence: 0.96, normalize: (value) => value.replaceAll(" ", "").toUpperCase() },
  { name: "sellerContact", labels: /(?:ansprechpartner(?:in)?|kontakt(?:person)?)/i, value: /(?:[:#]\s*|\s+)(.+)/i, confidence: 0.9, normalize: (value) => value.trim() },
  { name: "sellerPhone", labels: /(?:telefon|tel\.?|fon)\b/i, value: /(?:[:#]\s*|\s+)(\+?[0-9][0-9 /().-]{6,})/i, confidence: 0.9, normalize: (value) => value.replace(/\s+/g, " ").trim() },
  { name: "sellerEmail", labels: /e-?mail/i, value: /([A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,})/i, confidence: 0.96, normalize: (value) => value.trim() },
  { name: "iban", labels: /\bIBAN\b/i, value: /\b([A-Z]{2}\s?[0-9]{2}(?:\s?[A-Z0-9]){11,30})\b/i, confidence: 0.98, normalize: normalizeIban },
  { name: "bic", labels: /\b(?:BIC|SWIFT)\b/i, value: /(?:[:#]\s*|\s+)([A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)/i, confidence: 0.96, normalize: (value) => value.replaceAll(" ", "").toUpperCase() },
  { name: "lineNet", labels: /(?:nettobetrag|summe netto|net total|^\s*netto\s*:)/i, value: new RegExp(amountValue), confidence: 0.91, normalize: normalizeAmount },
  { name: "taxTotal", labels: /\b(?:umsatzsteuer|mwst|ust|vat)\b\.?(?![-.\s]*(?:id|ident))(?:\s+\d+[,.]?\d*\s*%)?/i, value: new RegExp(amountValue), confidence: 0.89, normalize: normalizeAmount },
  { name: "taxInclusive", labels: /(?:bruttobetrag|summe brutto|gross total|^\s*gesamt\s*:)/i, value: new RegExp(amountValue), confidence: 0.92, normalize: normalizeAmount },
  { name: "payable", labels: /(?:zahlbetrag|rechnungsbetrag|gesamtbetrag|zu zahlen|amount due|^\s*gesamt\s*:)/i, value: new RegExp(amountValue), confidence: 0.95, normalize: normalizeAmount },
];

function normalizeDate(value: string): string {
  if (/^\d{4}-\d{2}-\d{2}$/.test(value)) return value;
  const [day, month, year] = value.split(/[./-]/);
  const fullYear = year?.length === 2 ? `20${year}` : year;
  return `${fullYear}-${month?.padStart(2, "0")}-${day?.padStart(2, "0")}`;
}

function normalizeAmount(value: string): string {
  const normalized = parseLocalizedDecimal(value);
  return normalized === null ? value : money(normalized);
}

function normalizeIban(value: string): string {
  const compact = value.replace(/\s/g, "").toUpperCase().replace(/(?:BIC|SWIFT)$/, "");
  // German IBANs always contain 22 characters. Limiting known German values also
  // protects against adjacent PDF tokens when IBAN and BIC share one visual line.
  return compact.startsWith("DE") && compact.length >= 22 ? compact.slice(0, 22) : compact;
}

function extractedField(name: ExtractedFieldName, value: string, line: TextLine, confidence: number): ExtractedField {
  return { name, value, confidence, sourceTokenIds: line.tokenIds, sourceText: line.text, transformations: [] };
}

function postalAddress(line: TextLine): { postalCode: string; city: string } | null {
  const match = line.text.match(/^\s*(?:[A-Z]{2}[- ]?)?(\d{4,6})\s+(.+?)\s*$/);
  return match?.[1] && match[2] ? { postalCode: match[1], city: match[2] } : null;
}

function assignParty(
  fields: Partial<Record<ExtractedFieldName, ExtractedField>>,
  prefix: "seller" | "buyer",
  nameLine: TextLine | undefined,
  addressLine: TextLine | undefined,
  cityLine: TextLine | undefined,
  confidence: number,
) {
  const postal = cityLine ? postalAddress(cityLine) : null;
  if (!nameLine || !addressLine || !cityLine || !postal) return;
  fields[`${prefix}Name`] = extractedField(`${prefix}Name`, nameLine.text.trim(), nameLine, confidence);
  fields[`${prefix}AddressLine1`] = extractedField(`${prefix}AddressLine1`, addressLine.text.trim(), addressLine, confidence);
  fields[`${prefix}PostalCode`] = extractedField(`${prefix}PostalCode`, postal.postalCode, cityLine, confidence);
  fields[`${prefix}City`] = extractedField(`${prefix}City`, postal.city, cityLine, confidence);
  fields[`${prefix}CountryCode`] = extractedField(`${prefix}CountryCode`, "DE", cityLine, Math.min(confidence, 0.72));
}

function classifyParties(lines: TextLine[], fields: Partial<Record<ExtractedFieldName, ExtractedField>>) {
  const buyerMarker = lines.findIndex((line) => /^(?:rechnung\s+an|rechnungsempfänger(?:in)?|bill\s+to)\s*:?$/i.test(line.text.trim()));
  if (buyerMarker < 0) return;

  let sellerCityIndex = -1;
  for (let index = buyerMarker - 1; index >= 0; index -= 1) {
    const candidate = lines[index];
    if (candidate && postalAddress(candidate) !== null) {
      sellerCityIndex = index;
      break;
    }
  }
  if (sellerCityIndex >= 2) {
    assignParty(fields, "seller", lines[sellerCityIndex - 2], lines[sellerCityIndex - 1], lines[sellerCityIndex], 0.82);
  }

  const buyerWindow = lines.slice(buyerMarker + 1, buyerMarker + 8);
  const buyerCityOffset = buyerWindow.findIndex((line) => postalAddress(line) !== null);
  if (buyerCityOffset >= 2) {
    const cityIndex = buyerMarker + 1 + buyerCityOffset;
    assignParty(fields, "buyer", lines[cityIndex - 2], lines[cityIndex - 1], lines[cityIndex], 0.84);
  }

  const buyerVatLine = lines.slice(buyerMarker + 1).find((line) => /(?:ust\.?-?id(?:nr)?\.?|umsatzsteuer-id|vat id)/i.test(line.text));
  const buyerVat = buyerVatLine?.text.match(/\b([A-Z]{2}(?:\s*[A-Z0-9]){8,14})\b/i)?.[1];
  if (buyerVatLine && buyerVat) {
    fields.buyerVatId = extractedField("buyerVatId", buyerVat.replaceAll(" ", "").toUpperCase(), buyerVatLine, 0.9);
  }
}

export function classifyFields(lines: TextLine[]): Partial<Record<ExtractedFieldName, ExtractedField>> {
  const fields: Partial<Record<ExtractedFieldName, ExtractedField>> = {};
  for (const rule of rules) {
    for (const line of lines) {
      if (!rule.labels.test(line.text)) continue;
      const labelMatch = line.text.match(rule.labels);
      const remainder = labelMatch ? line.text.slice((labelMatch.index ?? 0) + labelMatch[0].length) : line.text;
      const match = remainder.match(rule.value) ?? line.text.match(rule.value);
      const raw = match?.[1];
      if (!raw) continue;
      const value = rule.normalize?.(raw) ?? raw.trim();
      fields[rule.name] = {
        name: rule.name,
        value,
        confidence: rule.confidence,
        sourceTokenIds: line.tokenIds,
        sourceText: line.text,
        transformations: value === raw ? [] : [{ operation: rule.normalize === normalizeDate ? "normalize-date" : "normalize-value", input: raw, output: value }],
      };
      break;
    }
  }
  const currencyLine = lines.find((line) => /\b(EUR|USD|GBP|CHF)\b|€|\$|£/i.test(line.text));
  const currencyMatch = currencyLine?.text.match(/\b(EUR|USD|GBP|CHF)\b|€|\$|£/i)?.[0].toUpperCase();
  const currency = currencyMatch === "€" ? "EUR" : currencyMatch === "$" ? "USD" : currencyMatch === "£" ? "GBP" : currencyMatch;
  if (currency && currencyLine) fields.currency = { name: "currency", value: currency, confidence: 0.9, sourceTokenIds: currencyLine.tokenIds, sourceText: currencyLine.text, transformations: currency === currencyMatch ? [] : [{ operation: "normalize-currency", input: currencyMatch ?? "", output: currency }] };
  classifyParties(lines, fields);
  const termsIndex = lines.findIndex((line) => /^(?:zahlungsbedingungen|zahlungsziel|payment terms)\s*:?/i.test(line.text.trim()));
  if (termsIndex >= 0) {
    const labelLine = lines[termsIndex];
    const inline = labelLine?.text.replace(/^(?:zahlungsbedingungen|zahlungsziel|payment terms)\s*:?\s*/i, "").trim();
    const source = inline ? labelLine : lines[termsIndex + 1];
    if (source) fields.paymentTerms = extractedField("paymentTerms", inline || source.text.trim(), source, inline ? 0.86 : 0.8);
  }
  return fields;
}
