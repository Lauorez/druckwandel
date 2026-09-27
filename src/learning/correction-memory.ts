import type { ReviewDraft } from "../review/draft.js";
import { parseLocalizedDecimal } from "../domain/localized-decimal.js";
import { decimal, money } from "../domain/money.js";
import { isIsoDate } from "../domain/validate.js";
import { dateSearchNeedles, parseInvoiceDate, relativeDueDate } from "../extraction/dates.js";
import type { BoundingBox, ExtractedField, ExtractedFieldName, ExtractedLineItem, ExtractionResult, SourceToken, TextLine } from "../extraction/types.js";

export const LEARNABLE_FIELD_NAMES = [
  "invoiceNumber", "issueDate", "dueDate", "serviceDate", "currency", "buyerReference",
  "sellerName", "sellerAddressLine1", "sellerPostalCode", "sellerCity", "sellerCountryCode", "sellerVatId",
  "buyerName", "buyerAddressLine1", "buyerPostalCode", "buyerCity", "buyerCountryCode", "buyerVatId",
  "iban", "bic", "paymentTerms",
] as const satisfies readonly ExtractedFieldName[];

export type LearnableFieldName = typeof LEARNABLE_FIELD_NAMES[number];

export const LEARNABLE_FIELD_LABELS: Record<LearnableFieldName, string> = {
  invoiceNumber: "Rechnungsnummer",
  issueDate: "Rechnungsdatum",
  dueDate: "Fälligkeitsdatum",
  serviceDate: "Leistungsdatum",
  currency: "Währung",
  buyerReference: "Bestellnummer oder Leitweg-ID",
  sellerName: "Absender: Name",
  sellerAddressLine1: "Absender: Straße und Hausnummer",
  sellerPostalCode: "Absender: Postleitzahl",
  sellerCity: "Absender: Ort",
  sellerCountryCode: "Absender: Land",
  sellerVatId: "Absender: Umsatzsteuer-ID",
  buyerName: "Empfänger: Name",
  buyerAddressLine1: "Empfänger: Straße und Hausnummer",
  buyerPostalCode: "Empfänger: Postleitzahl",
  buyerCity: "Empfänger: Ort",
  buyerCountryCode: "Empfänger: Land",
  buyerVatId: "Empfänger: Umsatzsteuer-ID",
  iban: "IBAN",
  bic: "BIC",
  paymentTerms: "Zahlungsbedingungen",
};

export type LearnedRuleOrigin = "save" | "mark";

export type FieldSourceSelections = Partial<Record<LearnableFieldName, string[]>>;

interface RelativeBox extends BoundingBox {}

export interface LearnedFieldRule {
  id: string;
  field: LearnableFieldName;
  mode: "fill" | "correct";
  scopeKeys: string[];
  layoutKey: string;
  page: number;
  box: RelativeBox;
  prefix: string;
  suffix: string;
  region?: true;
  templateKey?: string;
  enabled?: boolean;
  origin?: LearnedRuleOrigin;
  createdAt: string;
  updatedAt: string;
}

export interface LearnedTableRule {
  id: string;
  scopeKeys: string[];
  layoutKey: string;
  page: number;
  startY: number;
  columns: {
    description: number;
    quantity: number;
    unitPrice: number;
    total: number;
  };
  defaultTaxRate: string;
  templateKey?: string;
  enabled?: boolean;
  origin?: LearnedRuleOrigin;
  createdAt: string;
  updatedAt: string;
}

export interface CorrectionMemory {
  schemaVersion: 1;
  rules: LearnedFieldRule[];
  tableRules: LearnedTableRule[];
}

export interface LearnCorrectionsResult {
  memory: CorrectionMemory;
  changedFields: LearnableFieldName[];
  learnedFields: LearnableFieldName[];
  skippedFields: LearnableFieldName[];
  tableChanged: boolean;
  tableLearned: boolean;
  tableSkipped: boolean;
}

export interface AppliedCorrectionsResult {
  extraction: ExtractionResult;
  appliedFields: LearnableFieldName[];
  appliedTable: boolean;
  skippedFields: LearnableFieldName[];
}

export interface LearnedAssignmentView {
  id: string;
  kind: "field" | "table";
  field?: LearnableFieldName;
  label: string;
  context: string;
  enabled: boolean;
  origin: LearnedRuleOrigin | "unknown";
}

export function emptyCorrectionMemory(): CorrectionMemory {
  return { schemaVersion: 1, rules: [], tableRules: [] };
}

function isFiniteBox(value: unknown): value is RelativeBox {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<RelativeBox>;
  return [candidate.x, candidate.y, candidate.width, candidate.height]
    .every((number) => typeof number === "number" && Number.isFinite(number) && number >= 0 && number <= 1);
}

export function parseCorrectionMemory(contents: string | null | undefined): CorrectionMemory {
  if (!contents) return emptyCorrectionMemory();
  try {
    const parsed = JSON.parse(contents) as Partial<CorrectionMemory>;
    if (parsed.schemaVersion !== 1 || !Array.isArray(parsed.rules)) return emptyCorrectionMemory();
    const rules = parsed.rules.filter((rule): rule is LearnedFieldRule => {
      if (typeof rule !== "object" || rule === null) return false;
      const candidate = rule as Partial<LearnedFieldRule>;
      return typeof candidate.id === "string"
        && LEARNABLE_FIELD_NAMES.includes(candidate.field as LearnableFieldName)
        && (candidate.mode === "fill" || candidate.mode === "correct")
        && Array.isArray(candidate.scopeKeys)
        && candidate.scopeKeys.every((key) => typeof key === "string")
        && typeof candidate.layoutKey === "string"
        && typeof candidate.page === "number"
        && Number.isInteger(candidate.page)
        && candidate.page > 0
        && isFiniteBox(candidate.box)
        && typeof candidate.prefix === "string"
        && typeof candidate.suffix === "string"
        && (candidate.region === undefined || candidate.region === true)
        && (candidate.templateKey === undefined || typeof candidate.templateKey === "string")
        && (candidate.enabled === undefined || typeof candidate.enabled === "boolean")
        && (candidate.origin === undefined || candidate.origin === "save" || candidate.origin === "mark")
        && typeof candidate.createdAt === "string"
        && typeof candidate.updatedAt === "string";
    });
    const tableRules = (Array.isArray(parsed.tableRules) ? parsed.tableRules : []).filter((rule): rule is LearnedTableRule => {
      if (typeof rule !== "object" || rule === null) return false;
      const candidate = rule as Partial<LearnedTableRule>;
      const columns = candidate.columns as Partial<LearnedTableRule["columns"]> | undefined;
      return typeof candidate.id === "string"
        && Array.isArray(candidate.scopeKeys)
        && candidate.scopeKeys.every((key) => typeof key === "string")
        && typeof candidate.layoutKey === "string"
        && typeof candidate.page === "number"
        && Number.isInteger(candidate.page)
        && candidate.page > 0
        && typeof candidate.startY === "number"
        && candidate.startY >= 0
        && candidate.startY <= 1
        && columns !== undefined
        && [columns.description, columns.quantity, columns.unitPrice, columns.total]
          .every((value) => typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 1)
        && typeof candidate.defaultTaxRate === "string"
        && (candidate.templateKey === undefined || typeof candidate.templateKey === "string")
        && (candidate.enabled === undefined || typeof candidate.enabled === "boolean")
        && (candidate.origin === undefined || candidate.origin === "save" || candidate.origin === "mark")
        && typeof candidate.createdAt === "string"
        && typeof candidate.updatedAt === "string";
    });
    return { schemaVersion: 1, rules: rules.slice(-250), tableRules: tableRules.slice(-50) };
  } catch {
    return emptyCorrectionMemory();
  }
}

function normalizeLoose(value: string): string {
  return value
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLocaleLowerCase("de-DE")
    .replace(/[^a-z0-9]/g, "");
}

function hash(value: string): string {
  let result = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    result ^= value.charCodeAt(index);
    result = Math.imul(result, 0x01000193);
  }
  return (result >>> 0).toString(16).padStart(8, "0");
}

function normalizeDate(value: string, issueDate?: string): string | null {
  return parseInvoiceDate(value) ?? relativeDueDate(value, issueDate);
}

function normalizedFieldValue(field: LearnableFieldName, value: string): string {
  const trimmed = value.trim();
  if (["issueDate", "dueDate", "serviceDate"].includes(field)) return normalizeDate(trimmed) ?? normalizeLoose(trimmed);
  if (["sellerVatId", "buyerVatId", "iban", "bic", "currency", "sellerCountryCode", "buyerCountryCode"].includes(field)) {
    return trimmed.replace(/\s/g, "").toUpperCase();
  }
  return normalizeLoose(trimmed);
}

export function reviewFieldValues(draft: ReviewDraft): Record<LearnableFieldName, string> {
  return {
    invoiceNumber: draft.invoiceNumber,
    issueDate: draft.issueDate,
    dueDate: draft.dueDate,
    serviceDate: draft.serviceDate,
    currency: draft.currency,
    buyerReference: draft.buyerReference,
    sellerName: draft.seller.name,
    sellerAddressLine1: draft.seller.addressLine1,
    sellerPostalCode: draft.seller.postalCode,
    sellerCity: draft.seller.city,
    sellerCountryCode: draft.seller.countryCode,
    sellerVatId: draft.seller.vatId,
    buyerName: draft.buyer.name,
    buyerAddressLine1: draft.buyer.addressLine1,
    buyerPostalCode: draft.buyer.postalCode,
    buyerCity: draft.buyer.city,
    buyerCountryCode: draft.buyer.countryCode,
    buyerVatId: draft.buyer.vatId,
    iban: draft.payment.iban,
    bic: draft.payment.bic,
    paymentTerms: draft.payment.terms,
  };
}

function identityKeys(values: Partial<Record<LearnableFieldName, string>>): string[] {
  const candidates = [
    values.sellerVatId ? `vat:${normalizedFieldValue("sellerVatId", values.sellerVatId)}` : "",
    values.iban ? `iban:${normalizedFieldValue("iban", values.iban)}` : "",
    values.sellerName ? `seller:${normalizeLoose(values.sellerName)}:${normalizeLoose(values.sellerPostalCode ?? "")}` : "",
  ].filter((value) => value.length > 8);
  return [...new Set(candidates.map(hash))];
}

function extractionIdentityKeys(extraction: ExtractionResult): string[] {
  const values: Partial<Record<LearnableFieldName, string>> = {};
  for (const field of LEARNABLE_FIELD_NAMES) {
    const value = extraction.fields[field]?.value;
    if (value) values[field] = value;
  }
  return identityKeys(values);
}

function layoutKey(extraction: ExtractionResult): string {
  const parts = extraction.pages.flatMap((page) => page.tokens
    .filter((token) => token.box.y / page.height < 0.58)
    .flatMap((token) => {
      const text = token.text.replace(/\d+/g, "");
      const normalized = normalizeLoose(text);
      if (normalized.length < 2) return [];
      const x = Math.round(token.box.x / page.width * 12);
      const y = Math.round(token.box.y / page.height * 18);
      return [`${page.page}:${x}:${y}:${normalized}`];
    }));
  return hash([...new Set(parts)].sort().join("|"));
}

// Only fixed labels and their geometry describe a reusable template. Names,
// addresses, invoice numbers and dates must not invalidate the template.
function templateKey(extraction: ExtractionResult): string | undefined {
  const labels = /^(?:rechnung|rechnungsnummer|rechnungsdatum|belegnummer|datum|kundennummer|kunden-nr\.?|rechnungs-nr\.?|unternehmen|bankverbindung|steuerangaben|iban|bic|ust-idnr\.?|ust-id\.?|steuernr\.?|geschäftsführer|bank|position|pos\.?|beschreibung|bezeichnung|tätigkeit|taetigkeit|menge|anzahl|satz|wert|einzelpreis|gesamtpreis|gesamtbetrag|endbetrag|leistung|artikel|netto|brutto|auftragskennung|zahlungsbedingungen)$/i;
  const anchors = extraction.pages.flatMap((page) => page.tokens.flatMap((token) => {
    const label = token.text.split(":")[0]!.trim();
    if (!labels.test(label)) return [];
    // Vertical flow changes with line wrapping and item count. Column anchors
    // remain stable; the selected field's own region constrains its readout.
    return [`${page.page}:${Math.round(token.box.x / page.width * 100)}:${normalizeLoose(label)}`];
  }));
  const unique = [...new Set(anchors)].sort();
  return unique.length >= 4 ? hash(unique.join("|")) : undefined;
}

export function selectedSourceLine(extraction: ExtractionResult, tokenIds: readonly string[]): TextLine | null {
  const ids = new Set(tokenIds);
  const tokens = extraction.pages.flatMap((page) => page.tokens).filter((token) => ids.has(token.id));
  if (tokens.length === 0 || tokens.length !== ids.size || tokens.some((token) => token.page !== tokens[0]!.page)) return null;
  const positions = new Map(extraction.lines.flatMap((line, row) => line.tokenIds.map((id) => [id, row] as const)));
  tokens.sort((a, b) => (positions.get(a.id) ?? 0) - (positions.get(b.id) ?? 0) || a.box.x - b.box.x);
  const x = Math.min(...tokens.map((token) => token.box.x));
  const y = Math.min(...tokens.map((token) => token.box.y));
  return {
    id: "selected-source",
    page: tokens[0]!.page,
    text: tokens.map((token) => token.text).join(" "),
    tokenIds: tokens.map((token) => token.id),
    box: {
      x, y,
      width: Math.max(...tokens.map((token) => token.box.x + token.box.width)) - x,
      height: Math.max(...tokens.map((token) => token.box.y + token.box.height)) - y,
    },
  };
}

interface LooseText {
  text: string;
  sourceIndices: number[];
}

function looseTextWithIndices(value: string): LooseText {
  let text = "";
  const sourceIndices: number[] = [];
  let sourceIndex = 0;
  for (const character of value) {
    const normalized = character.normalize("NFKD").replace(/[\u0300-\u036f]/g, "").toLocaleLowerCase("de-DE");
    for (const part of normalized) {
      if (/[a-z0-9]/.test(part)) {
        text += part;
        sourceIndices.push(sourceIndex);
      }
    }
    sourceIndex += character.length;
  }
  return { text, sourceIndices };
}

function valueNeedles(field: LearnableFieldName, value: string): string[] {
  const needles = [normalizeLoose(value)];
  if (["issueDate", "dueDate", "serviceDate"].includes(field)) {
    const date = normalizeDate(value);
    if (date) needles.push(...dateSearchNeedles(date));
  }
  return [...new Set(needles.filter((needle) => needle.length >= 2))].sort((a, b) => b.length - a.length);
}

function findValueRange(text: string, field: LearnableFieldName, value: string): { start: number; end: number } | null {
  const haystack = looseTextWithIndices(text);
  for (const needle of valueNeedles(field, value)) {
    const position = haystack.text.indexOf(needle);
    if (position < 0) continue;
    const start = haystack.sourceIndices[position];
    const next = haystack.sourceIndices[position + needle.length];
    if (start === undefined) continue;
    return { start, end: next ?? text.length };
  }
  if ((field === "sellerCountryCode" || field === "buyerCountryCode") && normalizeSourceValue(field, text) === value.trim().toUpperCase()) {
    return { start: 0, end: text.length };
  }
  return null;
}

const FIELD_HINTS: Partial<Record<LearnableFieldName, RegExp>> = {
  invoiceNumber: /rechnung|beleg|nummer/i,
  issueDate: /rechnung|beleg|ausstellung|datum/i,
  dueDate: /fällig|faellig|zahlbar|zahlung|tagen?/i,
  serviceDate: /leistung|lieferung/i,
  buyerReference: /bestell|auftrag|referenz|leitweg/i,
  sellerVatId: /steuer|ust|vat/i,
  buyerVatId: /steuer|ust|vat/i,
  iban: /iban|konto/i,
  bic: /bic|swift|bank/i,
  paymentTerms: /zahlung|zahlbar|bedingungen/i,
};

function locateValue(extraction: ExtractionResult, field: LearnableFieldName, value: string, selectedIds?: readonly string[]): {
  line: TextLine;
  prefix: string;
  suffix: string;
} | null {
  if (selectedIds) {
    const line = selectedSourceLine(extraction, selectedIds);
    const range = line ? findValueRange(line.text, field, value) : null;
    if (line && range) return { line, prefix: line.text.slice(0, range.start).trimEnd(), suffix: line.text.slice(range.end).trimStart() };
    if (line && field === "dueDate") {
      const computed = relativeDueDate(line.text, extraction.fields.issueDate?.value);
      if (computed && (computed === value || computed === parseInvoiceDate(value))) {
        return { line, prefix: "", suffix: "" };
      }
    }
    return null;
  }
  const hint = FIELD_HINTS[field];
  const matches = extraction.lines.flatMap((line) => {
    const lineTokens = tokensForLine(extraction, line);
    const tokenMatches = lineTokens.flatMap((token) => {
      const range = findValueRange(token.text, field, value);
      if (!range) return [];
      const extraCharacters = Math.max(0, normalizeLoose(token.text).length - normalizeLoose(value).length);
      const score = extraCharacters + (hint?.test(token.text) ? -120 : 0) + line.page;
      const ownPrefix = token.text.slice(0, range.start).trimEnd();
      return [{
        line: selectedSourceLine(extraction, [token.id])!,
        prefix: ownPrefix.slice(-120),
        suffix: token.text.slice(range.end).trimStart().slice(0, 120),
        score,
      }];
    });
    if (tokenMatches.length > 0) return tokenMatches;
    const joined = lineTokens.map((token) => token.text).join(" ");
    const range = findValueRange(joined, field, value);
    if (!range) return [];
    let offset = 0;
    const matchingIds = lineTokens.flatMap((token) => {
      const start = offset;
      offset += token.text.length + 1;
      return start < range.end && offset - 1 > range.start ? [token.id] : [];
    });
    const matchedLine = selectedSourceLine(extraction, matchingIds);
    const localRange = matchedLine ? findValueRange(matchedLine.text, field, value) : null;
    if (!matchedLine || !localRange) return [];
    const extraCharacters = Math.max(0, normalizeLoose(line.text).length - normalizeLoose(value).length);
    const score = extraCharacters + (hint?.test(line.text) ? -100 : 0) + line.page;
    return [{
      line: matchedLine,
      prefix: matchedLine.text.slice(0, localRange.start).trimEnd().slice(-120),
      suffix: matchedLine.text.slice(localRange.end).trimStart().slice(0, 120),
      score,
    }];
  });
  matches.sort((left, right) => left.score - right.score);
  if (matches.length > 1 && matches[0]!.score === matches[1]!.score) return null;
  return matches[0] ?? null;
}

function relativeBox(extraction: ExtractionResult, line: TextLine): RelativeBox | null {
  const page = extraction.pages.find((candidate) => candidate.page === line.page);
  if (!page || page.width <= 0 || page.height <= 0) return null;
  return {
    x: line.box.x / page.width,
    y: line.box.y / page.height,
    width: line.box.width / page.width,
    height: line.box.height / page.height,
  };
}

function sameContext(rule: Pick<LearnedFieldRule, "layoutKey" | "scopeKeys" | "templateKey">, keys: string[], currentLayoutKey: string, currentTemplateKey?: string): boolean {
  const templateMatch = Boolean(rule.templateKey && currentTemplateKey && rule.templateKey === currentTemplateKey);
  const layoutMatch = rule.layoutKey === currentLayoutKey;
  if (templateMatch || layoutMatch) return true;
  if (rule.templateKey || currentTemplateKey) return false;
  return rule.scopeKeys.some((key) => keys.includes(key));
}

function ruleEnabled(rule: { enabled?: boolean }): boolean {
  return rule.enabled !== false;
}

function affixRange(text: string, prefix: string, suffix: string): { start: number; end: number } | null {
  let start = 0;
  let end = text.length;
  if (prefix) {
    const range = findValueRange(text, "paymentTerms", prefix);
    if (!range) return null;
    start = range.end;
  }
  if (suffix) {
    const range = findValueRange(text.slice(start), "paymentTerms", suffix);
    if (!range) return null;
    end = start + range.start;
  }
  if (end <= start) return null;
  return { start, end };
}

export function normalizeSourceValue(field: LearnableFieldName, raw: string, context: { issueDate?: string } = {}): string | null {
  const trimmed = raw.trim().replace(/^[\s:#|]+|[\s|]+$/g, "");
  if (!trimmed) return null;
  if (["issueDate", "dueDate", "serviceDate"].includes(field)) {
    const date = normalizeDate(trimmed, field === "dueDate" ? context.issueDate : undefined);
    return date && isIsoDate(date) ? date : null;
  }
  if (field === "currency") {
    const match = trimmed.match(/\b(EUR|USD|GBP|CHF)\b|€|\$|£/i)?.[0].toUpperCase();
    return match === "€" ? "EUR" : match === "$" ? "USD" : match === "£" ? "GBP" : match ?? null;
  }
  if (field === "iban") {
    const compact = trimmed.replace(/\s/g, "").toUpperCase();
    return /^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(compact) ? compact : null;
  }
  if (field === "bic") {
    const compact = trimmed.replace(/\s/g, "").toUpperCase();
    return /^[A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?$/.test(compact) ? compact : null;
  }
  if (field === "sellerVatId" || field === "buyerVatId") {
    const compact = trimmed.replace(/\s/g, "").toUpperCase();
    return /^[A-Z]{2}[A-Z0-9]{8,14}$/.test(compact) ? compact : null;
  }
  if (field === "sellerCountryCode" || field === "buyerCountryCode") {
    const compact = trimmed.toUpperCase();
    const countries: Record<string, string> = { DEUTSCHLAND: "DE", GERMANY: "DE", ÖSTERREICH: "AT", AUSTRIA: "AT", SCHWEIZ: "CH", SWITZERLAND: "CH" };
    if (countries[compact]) return countries[compact]!;
    return /^[A-Z]{2}$/.test(compact) ? compact : null;
  }
  if (field === "sellerPostalCode" || field === "buyerPostalCode") {
    const match = trimmed.match(/\b(\d{4,6})\b/)?.[1]
      ?? trimmed.match(/\b([A-Z0-9][A-Z0-9 -]{2,9})\b/i)?.[1]?.trim();
    if (!match || !/\d/.test(match)) return null;
    return match;
  }
  if (field === "sellerCity" || field === "buyerCity") return trimmed.replace(/^\d{5}\s+/, "");
  if (field === "invoiceNumber" || field === "buyerReference") {
    const compact = trimmed.replace(/^[-–—:;,.]+|[-–—:;,.]+$/g, "").trim();
    return compact.length >= 3 && compact.length <= 100 ? compact : null;
  }
  if (/Name$|AddressLine1$/.test(field) && /^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(trimmed.replace(/\s/g, "").toUpperCase())) {
    return null;
  }
  return trimmed.length <= 500 ? trimmed : null;
}

const FIELD_ASSIGNMENT_HINTS: Partial<Record<LearnableFieldName, string>> = {
  issueDate: "Bitte markieren Sie ein gültiges Kalenderdatum.",
  dueDate: "Bitte markieren Sie ein gültiges Datum oder eine Angabe wie „fällig in 14 Tagen“.",
  serviceDate: "Bitte markieren Sie ein gültiges Kalenderdatum.",
  currency: "Bitte markieren Sie eine Währung mit drei Buchstaben, zum Beispiel EUR.",
  sellerCountryCode: "Bitte markieren Sie das Land mit zwei Buchstaben, zum Beispiel DE.",
  buyerCountryCode: "Bitte markieren Sie das Land mit zwei Buchstaben, zum Beispiel DE.",
  sellerVatId: "Bitte markieren Sie eine Umsatzsteuer-ID, zum Beispiel DE123456789.",
  buyerVatId: "Bitte markieren Sie eine Umsatzsteuer-ID, zum Beispiel DE123456789.",
  iban: "Bitte markieren Sie eine gültige IBAN.",
  bic: "Bitte markieren Sie einen gültigen BIC.",
  sellerPostalCode: "Bitte markieren Sie eine Postleitzahl.",
  buyerPostalCode: "Bitte markieren Sie eine Postleitzahl.",
};

export function sourceAssignmentError(field: LearnableFieldName, raw: string, context: { issueDate?: string } = {}): string | null {
  if (normalizeSourceValue(field, raw, context)) return null;
  return FIELD_ASSIGNMENT_HINTS[field] ?? "Bitte prüfen Sie den markierten Wert. Entfernen Sie zum Beispiel eine mitmarkierte Beschriftung.";
}

type RegionMatch =
  | { quality: "exact" | "shifted"; line: TextLine; raw: string; score: number }
  | { quality: "ambiguous"; score: number };

function matchingRegion(extraction: ExtractionResult, rule: LearnedFieldRule): RegionMatch | null {
  const page = extraction.pages.find((candidate) => candidate.page === rule.page);
  if (page) {
    // Match the saved text block, not its entire horizontal row. A small margin
    // tolerates PDF rounding while keeping adjacent footer columns separate.
    const tokens = page.tokens.filter((token) => {
      const x = token.box.x / page.width;
      const y = (token.box.y + token.box.height / 2) / page.height;
      return x >= rule.box.x - 0.006 && x < rule.box.x + rule.box.width + 0.006
        && y >= rule.box.y - 0.006 && y <= rule.box.y + rule.box.height + 0.006;
    });
    const exact = readoutFromTokens(extraction, rule, tokens);
    if (exact) return { ...exact, quality: "exact", score: 0 };
  }
  return matchingShiftedRegion(extraction, rule);
}

function readoutFromTokens(
  extraction: ExtractionResult,
  rule: LearnedFieldRule,
  tokens: SourceToken[],
): { line: TextLine; raw: string } | null {
  const line = selectedSourceLine(extraction, tokens.map((token) => token.id));
  if (!line) return null;
  // Postal code and city often share one PDF text run. Neither is a fixed
  // anchor: both may change on the next invoice made from the same template.
  const postalCity = line.text.match(/^(\d{5})\s+(.+)$/);
  if (postalCity && (rule.field === "sellerCity" || rule.field === "buyerCity")) return { line, raw: postalCity[2]! };
  if (postalCity && (rule.field === "sellerPostalCode" || rule.field === "buyerPostalCode")) return { line, raw: postalCity[1]! };
  const range = affixRange(line.text, rule.prefix, rule.suffix);
  return range ? { line, raw: line.text.slice(range.start, range.end) } : null;
}

function matchingShiftedRegion(extraction: ExtractionResult, rule: LearnedFieldRule): RegionMatch | null {
  const hint = FIELD_HINTS[rule.field];
  const issueDate = extraction.fields.issueDate?.value;
  const candidates = extraction.lines.flatMap((line) => {
    if (line.page < rule.page) return [];
    const box = relativeBox(extraction, line);
    if (!box) return [];
    const centerDistance = Math.abs((box.x + box.width / 2) - (rule.box.x + rule.box.width / 2));
    if (centerDistance > 0.12) return [];
    const verticalDistance = Math.abs((box.y + box.height / 2) - (rule.box.y + rule.box.height / 2));
    const hinted = Boolean(hint?.test(line.text));
    if (hint && !hinted) return [];
    if (!hint && (line.page !== rule.page || verticalDistance > 0.05)) return [];
    const parts = tokensForLine(extraction, line).map((token) => selectedSourceLine(extraction, [token.id])!);
    const isolated = parts.find((part) => {
      if (affixRange(part.text, rule.prefix, rule.suffix)) return true;
      return !rule.prefix && !rule.suffix && Boolean(normalizeSourceValue(rule.field, part.text, issueDate ? { issueDate } : {}));
    });
    const source = isolated ?? line;
    const range = affixRange(source.text, rule.prefix, rule.suffix);
    const postalCity = source.text.match(/^(\d{5})\s+(.+)$/);
    let raw: string | null = range ? source.text.slice(range.start, range.end) : null;
    if (!raw && postalCity && (rule.field === "sellerCity" || rule.field === "buyerCity")) raw = postalCity[2]!;
    if (!raw && postalCity && (rule.field === "sellerPostalCode" || rule.field === "buyerPostalCode")) raw = postalCity[1]!;
    if (!raw && !rule.prefix && !rule.suffix) raw = source.text;
    if (!raw) return [];
    const value = normalizeSourceValue(rule.field, raw, issueDate ? { issueDate } : {});
    if (!value) return [];
    const pagePenalty = (source.page - rule.page) * 0.45;
    const hintBonus = hinted ? -0.3 : 0;
    return [{ line: source, raw, score: verticalDistance + pagePenalty + centerDistance + hintBonus }];
  });
  candidates.sort((left, right) => left.score - right.score);
  const best = candidates[0];
  if (!best) return null;
  if (candidates[1] && candidates[1].score - best.score < 0.04) return { quality: "ambiguous", score: best.score };
  if (best.score > 0.28) return null;
  return { line: best.line, raw: best.raw, quality: "shifted", score: best.score };
}

function nearestMatchingLine(extraction: ExtractionResult, rule: LearnedFieldRule): RegionMatch | null {
  if (rule.region) return matchingRegion(extraction, rule);
  const page = extraction.pages.find((candidate) => candidate.page === rule.page);
  if (!page) return null;
  const candidates = extraction.lines.flatMap((line) => {
    if (line.page !== rule.page) return [];
    const box = relativeBox(extraction, line);
    if (!box) return [];
    // Legacy rules stored the whole row. Prefer a single text run matching
    // their affixes so neighboring bank/tax columns do not become field data.
    const isolated = tokensForLine(extraction, line).map((token) => selectedSourceLine(extraction, [token.id])!)
      .find((part) => affixRange(part.text, rule.prefix, rule.suffix));
    const source = isolated ?? line;
    const range = affixRange(source.text, rule.prefix, rule.suffix);
    if (!range) return [];
    const centerDistance = Math.abs((box.x + box.width / 2) - (rule.box.x + rule.box.width / 2));
    const verticalDistance = Math.abs((box.y + box.height / 2) - (rule.box.y + rule.box.height / 2));
    const score = verticalDistance * 4 + centerDistance;
    if (verticalDistance > 0.08 || centerDistance > 0.35) return [];
    return [{ line: source, raw: source.text.slice(range.start, range.end), score }];
  });
  candidates.sort((left, right) => left.score - right.score);
  const best = candidates[0];
  if (!best) return null;
  if (candidates[1] && candidates[1].score - best.score < 0.02) return { quality: "ambiguous", score: best.score };
  return { line: best.line, raw: best.raw, quality: "exact", score: best.score };
}

function extractionField(rule: LearnedFieldRule, line: TextLine, raw: string, value: string, quality: "exact" | "shifted"): ExtractedField {
  return {
    name: rule.field,
    value,
    confidence: quality === "exact" ? 0.84 : 0.66,
    sourceTokenIds: line.tokenIds,
    sourceText: line.text,
    transformations: [{ operation: "learned-layout", input: raw.trim(), output: value }],
  };
}

function tokensForLine(extraction: ExtractionResult, line: TextLine): SourceToken[] {
  const tokens = new Map(extraction.pages.flatMap((page) => page.tokens.map((token) => [token.id, token] as const)));
  return line.tokenIds
    .map((id) => tokens.get(id))
    .filter((token): token is SourceToken => token !== undefined)
    .sort((left, right) => left.box.x - right.box.x);
}

function canonicalNumber(value: string, fractionDigits: number): string | null {
  const parsed = parseLocalizedDecimal(value);
  if (parsed === null) return null;
  try {
    return decimal(parsed).toFixed(fractionDigits);
  } catch {
    return null;
  }
}

interface NumericTokenMatch {
  token: SourceToken;
  value: string;
}

function numericMatches(tokens: SourceToken[], fractionDigits: number): NumericTokenMatch[] {
  return tokens.flatMap((token) => [...token.text.matchAll(/-?\d[\d. ]*(?:,\d{1,4}|\.\d{1,4})?/g)].flatMap((match) => {
    const value = match[0] ? canonicalNumber(match[0], fractionDigits) : null;
    return value ? [{ token, value }] : [];
  }));
}

function lineItemSignature(line: ReviewDraft["lines"][number]): string {
  return [normalizeLoose(line.description), line.quantity, line.unitCode, line.netUnitPrice, line.taxRate].join("|");
}

interface LocatedTableRow {
  page: number;
  y: number;
  description: number;
  quantity: number;
  unitPrice: number;
  total: number;
}

function locateManualTableRow(
  extraction: ExtractionResult,
  manualLine: ReviewDraft["lines"][number],
): LocatedTableRow | null {
  const descriptionNeedle = normalizeLoose(manualLine.description);
  if (descriptionNeedle.length < 2) return null;
  const quantity = canonicalNumber(manualLine.quantity, 3);
  const unitPrice = canonicalNumber(manualLine.netUnitPrice, 2);
  let total: string | null = null;
  try {
    total = money(decimal(manualLine.quantity).mul(manualLine.netUnitPrice));
  } catch {
    return null;
  }
  if (!quantity || !unitPrice || !total) return null;

  for (const line of extraction.lines) {
    if (!normalizeLoose(line.text).includes(descriptionNeedle)) continue;
    const page = extraction.pages.find((candidate) => candidate.page === line.page);
    if (!page) continue;
    const tokens = tokensForLine(extraction, line);
    const descriptionToken = tokens.find((token) => {
      const text = normalizeLoose(token.text);
      return text.includes(descriptionNeedle) || descriptionNeedle.includes(text);
    });
    if (!descriptionToken) continue;
    const quantityMatch = numericMatches(tokens, 3).find((match) => match.value === quantity && match.token.box.x > descriptionToken.box.x);
    if (!quantityMatch) continue;
    const priceMatch = numericMatches(tokens, 2).find((match) => match.value === unitPrice && match.token.box.x > quantityMatch.token.box.x);
    if (!priceMatch) continue;
    const totalMatch = numericMatches(tokens, 2).find((match) => match.value === total && match.token.box.x > priceMatch.token.box.x);
    if (!totalMatch) continue;
    const center = (token: SourceToken) => (token.box.x + token.box.width / 2) / page.width;
    return {
      page: line.page,
      y: (line.box.y + line.box.height / 2) / page.height,
      description: center(descriptionToken),
      quantity: center(quantityMatch.token),
      unitPrice: center(priceMatch.token),
      total: center(totalMatch.token),
    };
  }
  return null;
}

function mostCommon(values: string[], fallback: string): string {
  const counts = new Map<string, number>();
  for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1);
  return [...counts].sort((left, right) => right[1] - left[1])[0]?.[0] ?? fallback;
}

function learnTableRule(
  extraction: ExtractionResult,
  draft: ReviewDraft,
  scopeKeys: string[],
  currentLayoutKey: string,
  currentTemplateKey: string | undefined,
  now: string,
  existingRules: LearnedTableRule[],
): LearnedTableRule | null {
  const locations = draft.lines.flatMap((line) => {
    const location = locateManualTableRow(extraction, line);
    return location ? [location] : [];
  });
  if (locations.length === 0) return null;
  const page = mostCommon(locations.map((location) => String(location.page)), "1");
  const samePage = locations.filter((location) => location.page === Number(page));
  const average = (select: (location: LocatedTableRow) => number) => samePage.reduce((sum, location) => sum + select(location), 0) / samePage.length;
  const columns = {
    description: average((location) => location.description),
    quantity: average((location) => location.quantity),
    unitPrice: average((location) => location.unitPrice),
    total: average((location) => location.total),
  };
  if (!(columns.description < columns.quantity && columns.quantity < columns.unitPrice && columns.unitPrice < columns.total)) return null;
  const startY = Math.min(...samePage.map((location) => location.y));
  const id = `table-${hash(`${scopeKeys.join("-")}|${currentLayoutKey}|${page}|${columns.description.toFixed(3)}|${startY.toFixed(3)}`)}`;
  const existing = existingRules.find((rule) => rule.id === id);
  return {
    id,
    scopeKeys,
    layoutKey: currentLayoutKey,
    page: Number(page),
    startY,
    columns,
    defaultTaxRate: mostCommon(draft.lines.map((line) => canonicalNumber(line.taxRate, 2) ?? ""), "19.00"),
    ...(currentTemplateKey ? { templateKey: currentTemplateKey } : {}),
    enabled: true,
    origin: "save",
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
}

function textInRange(tokens: SourceToken[], minimum: number, maximum: number, pageWidth: number): string {
  return tokens
    .filter((token) => {
      const center = (token.box.x + token.box.width / 2) / pageWidth;
      return center >= minimum && center < maximum;
    })
    .map((token) => token.text)
    .join(" ")
    .trim();
}

function parseLearnedTable(extraction: ExtractionResult, rule: LearnedTableRule): ExtractedLineItem[] {
  const page = extraction.pages.find((candidate) => candidate.page === rule.page);
  if (!page) return [];
  const descriptionQuantityBoundary = (rule.columns.description + rule.columns.quantity) / 2;
  const quantityPriceBoundary = (rule.columns.quantity + rule.columns.unitPrice) / 2;
  const priceTotalBoundary = (rule.columns.unitPrice + rule.columns.total) / 2;
  const descriptionMinimum = Math.max(0, rule.columns.description - 0.04);
  const results: ExtractedLineItem[] = [];

  for (const line of extraction.lines) {
    if (line.page !== rule.page || (line.box.y + line.box.height / 2) / page.height < rule.startY - 0.025) continue;
    if (/^(?:netto|zwischensumme|umsatzsteuer|mwst\.?|ust\.?|gesamt|zahlbetrag|zahlungsbedingungen)\b/i.test(line.text.trim())) {
      if (results.length > 0) break;
      continue;
    }
    const tokens = tokensForLine(extraction, line);
    const rawDescription = textInRange(tokens, descriptionMinimum, descriptionQuantityBoundary, page.width)
      .replace(/^\d+[.)]?\s+/, "")
      .trim();
    const rawQuantity = textInRange(tokens, descriptionQuantityBoundary, quantityPriceBoundary, page.width);
    const rawPrice = textInRange(tokens, quantityPriceBoundary, priceTotalBoundary, page.width);
    const rawTotal = textInRange(tokens, priceTotalBoundary, Math.min(1, rule.columns.total + 0.12), page.width);
    const quantityMatch = rawQuantity.match(/(-?\d[\d. ]*(?:,\d{1,4}|\.\d{1,4})?)\s*([A-Za-zÄÖÜäöüß]{0,10}\.?)/);
    const priceMatch = rawPrice.match(/-?\d[\d. ]*(?:,\d{1,4}|\.\d{1,4})?/);
    const totalMatch = rawTotal.match(/-?\d[\d. ]*(?:,\d{1,4}|\.\d{1,4})?/);
    const quantity = quantityMatch?.[1] ? canonicalNumber(quantityMatch[1], 3) : null;
    const netUnitPrice = priceMatch?.[0] ? canonicalNumber(priceMatch[0], 2) : null;
    const netAmount = totalMatch?.[0] ? canonicalNumber(totalMatch[0], 2) : null;
    if (!rawDescription || !quantity || !netUnitPrice || !netAmount) continue;
    try {
      if (!decimal(quantity).mul(netUnitPrice).toDecimalPlaces(2).eq(netAmount)) continue;
    } catch {
      continue;
    }
    const unitText = quantityMatch?.[2]?.replace(/\.$/, "") ?? "";
    results.push({
      description: rawDescription,
      quantity,
      ...(unitText ? { unit: unitText } : {}),
      netUnitPrice,
      netAmount,
      taxRate: rule.defaultTaxRate,
      confidence: 0.82,
      sourceTokenIds: tokens.map((token) => token.id),
      sourceText: line.text,
    });
  }
  return results;
}

export function applyLearnedCorrections(extraction: ExtractionResult, memory: CorrectionMemory): AppliedCorrectionsResult {
  const fields = { ...extraction.fields };
  const appliedFields: LearnableFieldName[] = [];
  const skippedFields: LearnableFieldName[] = [];
  const keys = extractionIdentityKeys(extraction);
  const currentLayoutKey = layoutKey(extraction);
  const currentTemplateKey = templateKey(extraction);
  const newestRules = [...memory.rules].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
  for (const rule of newestRules) {
    if (!ruleEnabled(rule) || appliedFields.includes(rule.field) || skippedFields.includes(rule.field) || !sameContext(rule, keys, currentLayoutKey, currentTemplateKey)) continue;
    if (rule.mode === "fill" && fields[rule.field]) continue;
    const match = nearestMatchingLine(extraction, rule);
    if (!match || match.quality === "ambiguous") {
      if (match?.quality === "ambiguous") skippedFields.push(rule.field);
      continue;
    }
    const existing = fields[rule.field];
    if (existing && match.quality === "shifted" && match.score > 0.08) {
      skippedFields.push(rule.field);
      continue;
    }
    const issueDate = fields.issueDate?.value ?? extraction.fields.issueDate?.value;
    const value = normalizeSourceValue(rule.field, match.raw, issueDate ? { issueDate } : {});
    if (!value) continue;
    fields[rule.field] = extractionField(rule, match.line, match.raw, value, match.quality);
    appliedFields.push(rule.field);
  }
  let lineItems = extraction.lineItems;
  let appliedTable = false;
  if (lineItems.length === 0) {
    const newestTableRules = [...memory.tableRules].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
    for (const rule of newestTableRules) {
      if (!ruleEnabled(rule) || !sameContext(rule, keys, currentLayoutKey, currentTemplateKey)) continue;
      const learnedItems = parseLearnedTable(extraction, rule);
      if (learnedItems.length === 0) continue;
      lineItems = learnedItems;
      appliedTable = true;
      break;
    }
  }
  const warnings = [...extraction.warnings];
  if (appliedTable) {
    warnings.push({ code: "LEARNED_TABLE", message: "Leistungen und Artikel wurden anhand einer ähnlichen Rechnung ergänzt. Bitte prüfen Sie die Angaben." });
  }
  if (skippedFields.length > 0) {
    warnings.push({
      code: "LEARNED_UNCERTAIN",
      message: "Mindestens eine gemerkte Stelle passt in dieser Rechnung nicht eindeutig. Der bisher erkannte Wert bleibt unverändert.",
    });
  }
  return { extraction: { ...extraction, fields, lineItems, warnings }, appliedFields, appliedTable, skippedFields };
}

/** Upgrade legacy whole-row rules only using a document in their original scope. */
export function upgradeLegacyFieldRules(extraction: ExtractionResult, memory: CorrectionMemory): CorrectionMemory {
  const keys = extractionIdentityKeys(extraction);
  const currentLayoutKey = layoutKey(extraction);
  const currentTemplateKey = templateKey(extraction);
  let changed = false;
  const rules = memory.rules.map((rule) => {
    if (rule.region || !sameContext(rule, keys, currentLayoutKey)) return rule;
    const match = nearestMatchingLine(extraction, rule);
    const value = match && match.quality !== "ambiguous"
      ? normalizeSourceValue(rule.field, match.raw, extraction.fields.issueDate?.value ? { issueDate: extraction.fields.issueDate.value } : {})
      : null;
    const located = match && match.quality !== "ambiguous" && value ? locateValue(extraction, rule.field, value, match.line.tokenIds) : null;
    const box = located ? relativeBox(extraction, located.line) : null;
    if (!located || !box) return rule;
    changed = true;
    const postalCity = /^(?:seller|buyer)(?:PostalCode|City)$/.test(rule.field) && /^\d{5}\s+.+$/.test(located.line.text);
    return {
      ...rule, box, region: true as const,
      prefix: postalCity ? "" : located.prefix,
      suffix: postalCity ? "" : located.suffix,
      ...(currentTemplateKey ? { templateKey: currentTemplateKey } : {}),
    };
  });
  return changed ? { ...memory, rules } : memory;
}

function ruleId(field: LearnableFieldName, scopeKeys: string[], currentLayoutKey: string, page: number, box: RelativeBox): string {
  return `${field}-${hash(`${scopeKeys.join("-")}|${currentLayoutKey}|${page}|${box.x.toFixed(3)}|${box.y.toFixed(3)}`)}`;
}

export function learnCorrections(
  memory: CorrectionMemory,
  extraction: ExtractionResult,
  initialDraft: ReviewDraft,
  correctedDraft: ReviewDraft,
  now = new Date().toISOString(),
  selections: FieldSourceSelections = {},
): LearnCorrectionsResult {
  const initial = reviewFieldValues(initialDraft);
  const corrected = reviewFieldValues(correctedDraft);
  const changedFields = LEARNABLE_FIELD_NAMES.filter((field) => {
    const next = corrected[field].trim();
    return next.length > 0 && (selections[field] !== undefined || normalizedFieldValue(field, initial[field]) !== normalizedFieldValue(field, next));
  });
  const tableChanged = initialDraft.lines.map(lineItemSignature).join("\n") !== correctedDraft.lines.map(lineItemSignature).join("\n")
    && correctedDraft.lines.length > 0;
  if (changedFields.length === 0 && !tableChanged) {
    return {
      memory,
      changedFields,
      learnedFields: [],
      skippedFields: [],
      tableChanged: false,
      tableLearned: false,
      tableSkipped: false,
    };
  }

  const scopeKeys = [...new Set([...identityKeys(corrected), ...extractionIdentityKeys(extraction)])];
  const currentLayoutKey = layoutKey(extraction);
  const currentTemplateKey = templateKey(extraction);
  const learnedFields: LearnableFieldName[] = [];
  const skippedFields: LearnableFieldName[] = [];
  let rules = [...memory.rules];
  let tableRules = [...memory.tableRules];

  for (const field of changedFields) {
    const located = locateValue(extraction, field, corrected[field], selections[field]);
    const box = located ? relativeBox(extraction, located.line) : null;
    if (!located || !box) {
      skippedFields.push(field);
      continue;
    }
    const id = ruleId(field, scopeKeys, currentLayoutKey, located.line.page, box);
    const existing = rules.find((rule) => rule.id === id);
    const postalCity = /^(?:seller|buyer)(?:PostalCode|City)$/.test(field) && /^\d{5}\s+.+$/.test(located.line.text);
    const nextRule: LearnedFieldRule = {
      id,
      field,
      mode: extraction.fields[field] || selections[field] ? "correct" : "fill",
      scopeKeys,
      layoutKey: currentLayoutKey,
      page: located.line.page,
      box,
      prefix: postalCity ? "" : located.prefix,
      suffix: postalCity ? "" : located.suffix,
      region: true,
      enabled: true,
      origin: selections[field] ? "mark" : "save",
      ...(currentTemplateKey ? { templateKey: currentTemplateKey } : {}),
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
    };
    rules = [...rules.filter((rule) => rule.id !== id && !(rule.field === field
      && sameContext(rule, scopeKeys, currentLayoutKey, currentTemplateKey))), nextRule].slice(-250);
    learnedFields.push(field);
  }

  let tableLearned = false;
  let tableSkipped = false;
  if (tableChanged) {
    const tableRule = extraction.lineItems.length === 0
      ? learnTableRule(extraction, correctedDraft, scopeKeys, currentLayoutKey, currentTemplateKey, now, tableRules)
      : null;
    if (tableRule) {
      tableRules = [...tableRules.filter((rule) => rule.id !== tableRule.id), tableRule].slice(-50);
      tableLearned = true;
    } else {
      tableSkipped = true;
    }
  }

  return {
    memory: { schemaVersion: 1, rules, tableRules },
    changedFields,
    learnedFields,
    skippedFields,
    tableChanged,
    tableLearned,
    tableSkipped,
  };
}

function assignmentContext(prefix: string, suffix: string): string {
  const snippet = [prefix, suffix]
    .map((value) => value.replace(/\s+/g, " ").trim())
    .find((value) => value.length >= 3 && /[A-Za-zÄÖÜäöüß]/.test(value) && !/^[A-Z]{2}\d{2}[A-Z0-9]+$/.test(value.replace(/\s/g, "")));
  return snippet ? `neben „${snippet.slice(0, 40)}${snippet.length > 40 ? "…" : ""}“` : "Gemerkte Stelle in der Rechnung";
}

export function listLearnedAssignments(memory: CorrectionMemory): LearnedAssignmentView[] {
  const fields = [...memory.rules]
    .sort((left, right) => left.field.localeCompare(right.field) || right.updatedAt.localeCompare(left.updatedAt))
    .map((rule) => ({
      id: rule.id,
      kind: "field" as const,
      field: rule.field,
      label: LEARNABLE_FIELD_LABELS[rule.field],
      context: assignmentContext(rule.prefix, rule.suffix),
      enabled: ruleEnabled(rule),
      origin: rule.origin ?? "unknown" as const,
    }));
  const tables = memory.tableRules.map((rule) => ({
    id: rule.id,
    kind: "table" as const,
    label: "Leistungen und Artikel",
    context: "Gemerkte Spaltenanordnung",
    enabled: ruleEnabled(rule),
    origin: rule.origin ?? "unknown" as const,
  }));
  return [...fields, ...tables];
}

export function setLearnedAssignmentEnabled(memory: CorrectionMemory, id: string, enabled: boolean, now = new Date().toISOString()): CorrectionMemory {
  return {
    schemaVersion: 1,
    rules: memory.rules.map((rule) => rule.id === id ? { ...rule, enabled, updatedAt: now } : rule),
    tableRules: memory.tableRules.map((rule) => rule.id === id ? { ...rule, enabled, updatedAt: now } : rule),
  };
}

export function removeLearnedAssignment(memory: CorrectionMemory, id: string): CorrectionMemory {
  return {
    schemaVersion: 1,
    rules: memory.rules.filter((rule) => rule.id !== id),
    tableRules: memory.tableRules.filter((rule) => rule.id !== id),
  };
}
