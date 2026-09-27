import type { ExtractedField, ExtractedFieldName, TextLine } from "./types.js";
import { parseLocalizedDecimal } from "../domain/localized-decimal.js";
import { money } from "../domain/money.js";
import { parseInvoiceDate, relativeDueDate } from "./dates.js";

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
  { name: "issueDate", labels: /(?:rechnungsdatum|belegdatum|ausstellungsdatum|invoice date|^\s*datum\s*:)/i, value: new RegExp(dateValue), confidence: 0.88, normalize: normalizeDate },
  { name: "dueDate", labels: /(?:fällig(?:keit| am)?|faellig(?:keit| am)?|zahlbar bis|due date)/i, value: new RegExp(dateValue), confidence: 0.92, normalize: normalizeDate },
  { name: "serviceDate", labels: /(?:leistungsdatum|lieferdatum|delivery date)/i, value: new RegExp(dateValue), confidence: 0.91, normalize: normalizeDate },
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
  { name: "payable", labels: /(?:zahlbetrag|rechnungsbetrag|gesamtbetrag|zu zahlen|restbetrag|noch zu zahlen|amount due|^\s*gesamt\s*:)/i, value: new RegExp(amountValue), confidence: 0.95, normalize: normalizeAmount },
];

const BUYER_MARKER = /^(?:rechnung\s+an|rechnungsempfänger(?:in)?|rechnungsempfaenger(?:in)?|rechnungsadresse|kundenadresse|lieferanschrift|bill(?:ed)?\s+to)\s*:?\s*(.*)$|^(?:empfänger(?:in)?|empfaenger(?:in)?|kunde|an)\s*:\s*(.*)$|^(?:empfänger(?:in)?|empfaenger(?:in)?)\s*$/i;
const SKIP_ADDRESS = /^(?:z\.?\s*h(?:d)?\.?|zu\s+h(?:ä|ae)nden|c\/o|tel\.?|telefon|fax|mobil|e-?mail|www\.|https?:|ust\.?-?id|iban|bic|steuernr)/i;
const STREET = /(?:str(?:a(?:ss|ß)e)?\.?|weg|platz|gasse|allee|ring|damm|ufer|chaussee|hof|markt)\b.*\d|\d+[a-z]?(?:\s*[-\/]\s*\d+[a-z]?)?\s*$/i;
const COUNTRY_NAMES: Record<string, string> = {
  DEUTSCHLAND: "DE", GERMANY: "DE", ÖSTERREICH: "AT", OESTERREICH: "AT", AUSTRIA: "AT", SCHWEIZ: "CH", SWITZERLAND: "CH",
};

function normalizeDate(value: string): string {
  return parseInvoiceDate(value) ?? value;
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

function postalAddress(text: string): { postalCode: string; city: string } | null {
  const match = text.match(/^\s*(?:[A-Z]{2}[- ]?)?(\d{4,6})\s+(.+?)\s*$/);
  const city = match?.[2]?.replace(/[,;]+$/, "").trim();
  if (!match?.[1] || !city || /^\d+$/.test(city)) return null;
  if (/\b(?:iban|bic|bank|steuer|geschäftsführer|geschaeftsfuehrer|ust-?id|www\.|tel\.?)\b/i.test(city)) return null;
  if (city.length > 40) return null;
  return { postalCode: match[1], city };
}

function countryCode(text: string): string | null {
  const compact = text.trim().replace(/\.$/, "").toUpperCase();
  if (COUNTRY_NAMES[compact]) return COUNTRY_NAMES[compact]!;
  return /^[A-Z]{2}$/.test(compact) ? compact : null;
}

function isStreetLine(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed || postalAddress(trimmed) || SKIP_ADDRESS.test(trimmed) || countryCode(trimmed)) return false;
  if (!/\d/.test(trimmed) || !/[A-Za-zÄÖÜäöüß]{3,}/.test(trimmed)) return false;
  if (/^\d{1,2}[./-]\d{1,2}[./-]\d{2,4}/.test(trimmed)) return false;
  return STREET.test(trimmed);
}

function isNameLine(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed || postalAddress(trimmed) || isStreetLine(trimmed) || SKIP_ADDRESS.test(trimmed) || countryCode(trimmed)) return false;
  if (/^(?:rechnung|rechnungsnummer|datum|position|beschreibung|netto|ust|iban|pos\.?)\b/i.test(trimmed)) return false;
  return /[A-Za-zÄÖÜäöüß]{3,}/.test(trimmed);
}

function assignParty(
  fields: Partial<Record<ExtractedFieldName, ExtractedField>>,
  prefix: "seller" | "buyer",
  nameLine: TextLine | undefined,
  addressLine: TextLine | undefined,
  cityLine: TextLine | undefined,
  confidence: number,
  countryLine?: TextLine,
) {
  const postal = cityLine ? postalAddress(cityLine.text) : null;
  if (!nameLine || !addressLine || !cityLine || !postal) return;
  fields[`${prefix}Name`] = extractedField(`${prefix}Name`, nameLine.text.trim(), nameLine, confidence);
  fields[`${prefix}AddressLine1`] = extractedField(`${prefix}AddressLine1`, addressLine.text.trim(), addressLine, confidence);
  fields[`${prefix}PostalCode`] = extractedField(`${prefix}PostalCode`, postal.postalCode, cityLine, confidence);
  fields[`${prefix}City`] = extractedField(`${prefix}City`, postal.city, cityLine, confidence);
  const country = countryLine ? countryCode(countryLine.text) : null;
  fields[`${prefix}CountryCode`] = extractedField(`${prefix}CountryCode`, country ?? "DE", countryLine ?? cityLine, country ? confidence : Math.min(confidence, 0.72));
}

function findAddressBlock(lines: TextLine[], from: number, until: number): {
  name: TextLine;
  street: TextLine;
  city: TextLine;
  country?: TextLine;
} | null {
  const window = lines.slice(from, until);
  const cityOffset = window.findIndex((line) => postalAddress(line.text) !== null);
  if (cityOffset < 0) return null;
  const city = window[cityOffset]!;
  const before = window.slice(0, cityOffset).filter((line) => line.text.trim() && !SKIP_ADDRESS.test(line.text.trim()));
  const street = [...before].reverse().find((line) => isStreetLine(line.text));
  const name = [...before].reverse().find((line) => line !== street && isNameLine(line.text));
  if (!name || !street) return null;
  const afterCity = window[cityOffset + 1];
  const country = afterCity && countryCode(afterCity.text) ? afterCity : undefined;
  return country ? { name, street, city, country } : { name, street, city };
}

function classifyParties(lines: TextLine[], fields: Partial<Record<ExtractedFieldName, ExtractedField>>) {
  const buyerMarker = lines.findIndex((line) => BUYER_MARKER.test(line.text.trim()));
  if (buyerMarker >= 0) {
    const remainder = lines[buyerMarker]?.text.trim().match(BUYER_MARKER)?.slice(1).find((part) => part)?.trim();
    const seller = findAddressBlock(lines, 0, buyerMarker);
    if (seller) assignParty(fields, "seller", seller.name, seller.street, seller.city, 0.82, seller.country);

    if (remainder && isNameLine(remainder)) {
      const synthetic: TextLine = { ...lines[buyerMarker]!, text: remainder };
      const rest = findAddressBlock(lines, buyerMarker + 1, buyerMarker + 8);
      if (rest) assignParty(fields, "buyer", synthetic, rest.street, rest.city, 0.84, rest.country);
      else {
        const cityLine = lines.slice(buyerMarker + 1, buyerMarker + 8).find((line) => postalAddress(line.text));
        const street = lines.slice(buyerMarker + 1, buyerMarker + 8).find((line) => isStreetLine(line.text));
        if (street && cityLine) assignParty(fields, "buyer", synthetic, street, cityLine, 0.84);
      }
    } else {
      const buyer = findAddressBlock(lines, buyerMarker + 1, buyerMarker + 8);
      if (buyer) assignParty(fields, "buyer", buyer.name, buyer.street, buyer.city, 0.84, buyer.country);
    }

    const buyerVatLine = lines.slice(buyerMarker + 1).find((line) => /(?:ust\.?-?id(?:nr)?\.?|umsatzsteuer-id|vat id)/i.test(line.text));
    const buyerVat = buyerVatLine?.text.match(/\b([A-Z]{2}(?:\s*[A-Z0-9]){8,14})\b/i)?.[1];
    if (buyerVatLine && buyerVat) {
      fields.buyerVatId = extractedField("buyerVatId", buyerVat.replaceAll(" ", "").toUpperCase(), buyerVatLine, 0.9);
    }
    return;
  }

  const headerLimit = lines.findIndex((line) => /^(?:position|pos\.?|beschreibung|leistung|artikel|netto|rechnungsnummer)\b/i.test(line.text.trim()));
  const seller = findAddressBlock(lines, 0, headerLimit >= 0 ? headerLimit : Math.min(lines.length, 16));
  if (seller && !fields.sellerName) assignParty(fields, "seller", seller.name, seller.street, seller.city, 0.74, seller.country);
}

function applyNamedDates(lines: TextLine[], fields: Partial<Record<ExtractedFieldName, ExtractedField>>) {
  const dateFields = [
    { name: "issueDate" as const, labels: /(?:rechnungsdatum|belegdatum|ausstellungsdatum|invoice date|^\s*datum\s*:)/i, confidence: 0.88 },
    { name: "dueDate" as const, labels: /(?:fällig(?:keit| am)?|faellig(?:keit| am)?|zahlbar bis|due date)/i, confidence: 0.92 },
    { name: "serviceDate" as const, labels: /(?:leistungsdatum|lieferdatum|delivery date)/i, confidence: 0.91 },
  ];
  for (const rule of dateFields) {
    if (fields[rule.name]) continue;
    for (const line of lines) {
      if (!rule.labels.test(line.text)) continue;
      const value = parseInvoiceDate(line.text);
      if (!value) continue;
      fields[rule.name] = {
        name: rule.name,
        value,
        confidence: rule.confidence,
        sourceTokenIds: line.tokenIds,
        sourceText: line.text,
        transformations: [{ operation: "normalize-date", input: line.text, output: value }],
      };
      break;
    }
  }
}

function applyRelativeDueDate(lines: TextLine[], fields: Partial<Record<ExtractedFieldName, ExtractedField>>) {
  if (fields.dueDate || !fields.issueDate) return;
  for (const line of lines) {
    const value = relativeDueDate(line.text, fields.issueDate.value);
    if (!value) continue;
    fields.dueDate = {
      name: "dueDate",
      value,
      confidence: 0.86,
      sourceTokenIds: line.tokenIds,
      sourceText: line.text,
      transformations: [{ operation: "normalize-date", input: line.text, output: value }],
    };
    return;
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
      if (rule.normalize === normalizeDate && parseInvoiceDate(raw) === null) continue;
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
  applyNamedDates(lines, fields);
  applyRelativeDueDate(lines, fields);
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
