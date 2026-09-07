import { parseLocalizedDecimal } from "../domain/localized-decimal.js";
import { decimal } from "../domain/money.js";
import { reconstructTableRows, type TableRow } from "./layout.js";
import type { DocumentPage, ExtractedField, ExtractedFieldName, ExtractedLineItem, SourceToken, TextLine } from "./types.js";

interface ColumnGuide {
  page: number;
  rowIndex: number;
  descriptionX: number;
  quantityX: number;
  unitPriceX: number;
  totalX: number;
}

const QUANTITY_AND_UNIT = /^(.*?)\s+(-?\d+(?:[.,]\d+)?)\s*([A-Za-zÄÖÜäöüß]{1,8}\.?)?$/;
const QUANTITY_CELL = /^(-?\d+(?:[.,]\d+)?)\s*([A-Za-zÄÖÜäöüß]{1,8}\.?)?$/;
const ITEM_DATE = /^(\d{2})\.(\d{2})\.(\d{2,4})\s+(.+)$/;

function normalizedNumber(value: string, decimals = 2): string | null {
  const normalized = parseLocalizedDecimal(value);
  return normalized === null ? null : decimal(normalized).toDecimalPlaces(decimals).toFixed(decimals);
}

function field(name: ExtractedFieldName, raw: string, value: string, tokenIds: string[], sourceText: string, confidence: number): ExtractedField {
  return { name, value, confidence, sourceTokenIds: tokenIds, sourceText, transformations: raw === value ? [] : [{ operation: "normalize-value", input: raw, output: value }] };
}

function sourceTokens(row: TableRow, tokensById: Map<string, SourceToken>): SourceToken[] {
  return row.cells
    .flatMap((cell) => cell.tokenIds)
    .map((id) => tokensById.get(id))
    .filter((token): token is SourceToken => token !== undefined)
    .sort((left, right) => left.box.x - right.box.x);
}

function tokenAt(tokens: SourceToken[], pattern: RegExp): SourceToken | undefined {
  return tokens.find((token) => pattern.test(token.text));
}

function findColumnGuides(rows: TableRow[], tokensById: Map<string, SourceToken>): ColumnGuide[] {
  return rows.flatMap((row, rowIndex) => {
    const tokens = sourceTokens(row, tokensById);
    const description = tokenAt(tokens, /^(?:beschreibung|leistung|artikel|description)$/i);
    const quantity = tokenAt(tokens, /^(?:menge|anzahl|qty\.?|quantity)$/i);
    const unitPrice = tokenAt(tokens, /^(?:einzelpreis|stückpreis|preis|unit\s*price)$/i);
    const total = tokenAt(tokens, /^(?:gesamt|betrag|summe|total)$/i);
    if (!description || !quantity || !unitPrice || !total) return [];
    return [{ page: row.page, rowIndex, descriptionX: description.box.x, quantityX: quantity.box.x, unitPriceX: unitPrice.box.x, totalX: total.box.x }];
  });
}

function inferTaxRate(lines: TextLine[]): string | undefined {
  for (const line of lines) {
    const match = line.text.match(/(?:umsatzsteuer|mwst\.?|ust\.?|vat)[^%\d]*(\d+(?:[.,]\d+)?)\s*%/i);
    const rate = match?.[1] ? normalizedNumber(match[1]) : null;
    if (rate !== null) return rate;
  }
  return undefined;
}

function itemDescription(value: string): { description: string; serviceDate?: string } {
  const withoutPosition = value.trim().replace(/^\d+[.)]?\s+/, "");
  const match = withoutPosition.match(ITEM_DATE);
  if (!match) return { description: withoutPosition };
  const [, day, month, year, description] = match;
  return {
    description: description ?? withoutPosition,
    serviceDate: `${year?.length === 2 ? `20${year}` : year}-${month}-${day}`,
  };
}

function parseGuidedItem(
  row: TableRow,
  line: TextLine,
  guide: ColumnGuide,
  tokensById: Map<string, SourceToken>,
  defaultTaxRate: string | undefined,
): ExtractedLineItem | null {
  const tokens = sourceTokens(row, tokensById);
  const priceBoundary = (guide.quantityX + guide.unitPriceX) / 2;
  const totalBoundary = (guide.unitPriceX + guide.totalX) / 2;
  const descriptionAndQuantity = tokens
    .filter((token) => token.box.x >= guide.descriptionX - 4 && token.box.x < priceBoundary)
    .map((token) => token.text)
    .join(" ")
    .trim();
  const quantityMatch = descriptionAndQuantity.match(QUANTITY_AND_UNIT);
  if (!quantityMatch?.[1] || !quantityMatch[2]) return null;

  const priceToken = tokens.find((token) => token.box.x >= priceBoundary && token.box.x < totalBoundary && normalizedNumber(token.text) !== null);
  const totalToken = tokens.find((token) => token.box.x >= totalBoundary && normalizedNumber(token.text) !== null);
  if (!priceToken || !totalToken) return null;
  const netUnitPrice = normalizedNumber(priceToken.text);
  const netAmount = normalizedNumber(totalToken.text);
  const quantity = normalizedNumber(quantityMatch[2], 3);
  if (!netUnitPrice || !netAmount || !quantity) return null;

  const parsedDescription = itemDescription(quantityMatch[1]);
  const rateToken = tokens.find((token) => token.box.x > totalToken.box.x + totalToken.box.width && /%/.test(token.text));
  const rate = rateToken ? normalizedNumber(rateToken.text) ?? defaultTaxRate : defaultTaxRate;
  return {
    ...parsedDescription,
    quantity,
    ...(quantityMatch[3] ? { unit: quantityMatch[3].replace(/\.$/, "") } : {}),
    netUnitPrice,
    netAmount,
    ...(rate ? { taxRate: rate } : {}),
    confidence: 0.91,
    sourceTokenIds: tokens.map((token) => token.id),
    sourceText: line.text,
  };
}

function classifySummary(row: TableRow, sourceLine: string): Partial<Record<ExtractedFieldName, ExtractedField>> | null {
  const label = row.cells[0]?.text ?? "";
  if (!/(?:umsatzsteuer|mwst\.?|ust\.?|vat)\s+\d+[,.]?\d*\s*%/i.test(label)) return null;
  const numeric = row.cells.slice(1).flatMap((cell) => normalizedNumber(cell.text) === null ? [] : [cell]);
  if (numeric.length < 4) return {};
  const [net, , tax, gross] = numeric;
  if (!net || !tax || !gross) return {};
  return {
    lineNet: field("lineNet", net.text, normalizedNumber(net.text)!, net.tokenIds, sourceLine, 0.93),
    taxTotal: field("taxTotal", tax.text, normalizedNumber(tax.text)!, tax.tokenIds, sourceLine, 0.94),
    taxInclusive: field("taxInclusive", gross.text, normalizedNumber(gross.text)!, gross.tokenIds, sourceLine, 0.93),
    payable: field("payable", gross.text, normalizedNumber(gross.text)!, gross.tokenIds, sourceLine, 0.88),
  };
}

function parseUnguidedItem(row: TableRow, line: TextLine, defaultTaxRate: string | undefined): ExtractedLineItem | null {
  const quantityIndex = row.cells.findIndex((cell, index) => index > 0 && QUANTITY_CELL.test(cell.text));
  if (quantityIndex < 1) return null;
  const quantityCell = row.cells[quantityIndex];
  const quantityMatch = quantityCell?.text.match(QUANTITY_CELL);
  const priceCell = row.cells[quantityIndex + 1];
  const totalCell = row.cells[quantityIndex + 2];
  if (!quantityCell || !quantityMatch?.[1] || !priceCell || !totalCell) return null;
  const quantity = normalizedNumber(quantityMatch[1], 3);
  const netUnitPrice = normalizedNumber(priceCell.text);
  const netAmount = normalizedNumber(totalCell.text);
  if (!quantity || !netUnitPrice || !netAmount) return null;
  const descriptionText = row.cells.slice(0, quantityIndex).map((cell) => cell.text).join(" ");
  const parsedDescription = itemDescription(descriptionText);
  const possibleRate = row.cells[quantityIndex + 3];
  const taxRate = possibleRate ? normalizedNumber(possibleRate.text) ?? defaultTaxRate : defaultTaxRate;
  return {
    ...parsedDescription,
    quantity,
    ...(quantityMatch[2] ? { unit: quantityMatch[2].replace(/\.$/, "") } : {}),
    netUnitPrice,
    netAmount,
    ...(taxRate ? { taxRate } : {}),
    confidence: 0.86,
    sourceTokenIds: row.cells.flatMap((cell) => cell.tokenIds),
    sourceText: line.text,
  };
}

export function classifyInvoiceTables(lines: TextLine[], pages: DocumentPage[]): {
  fields: Partial<Record<ExtractedFieldName, ExtractedField>>;
  lineItems: ExtractedLineItem[];
} {
  const rows = reconstructTableRows(lines, pages);
  const tokensById = new Map(pages.flatMap((page) => page.tokens.map((token) => [token.id, token] as const)));
  const fields: Partial<Record<ExtractedFieldName, ExtractedField>> = {};
  const lineItems: ExtractedLineItem[] = [];
  const processedRows = new Set<number>();
  const defaultTaxRate = inferTaxRate(lines);

  for (const guide of findColumnGuides(rows, tokensById)) {
    for (let rowIndex = guide.rowIndex + 1; rowIndex < rows.length; rowIndex += 1) {
      const row = rows[rowIndex];
      const line = lines[rowIndex];
      if (!row || !line || row.page !== guide.page) break;
      if (/^(?:netto|zwischensumme|umsatzsteuer|mwst\.?|ust\.?|gesamt|zahlbetrag|zahlungsbedingungen)\b/i.test(line.text.trim())) break;
      const item = parseGuidedItem(row, line, guide, tokensById, defaultTaxRate);
      if (item) {
        lineItems.push(item);
        processedRows.add(rowIndex);
      } else if (lineItems.length > 0) {
        break;
      }
    }
  }

  rows.forEach((row, rowIndex) => {
    const line = lines[rowIndex];
    if (!line) return;
    const summary = classifySummary(row, line.text);
    if (summary !== null) {
      Object.assign(fields, summary);
      return;
    }
    if (processedRows.has(rowIndex)) return;
    const item = parseUnguidedItem(row, line, defaultTaxRate);
    if (item) lineItems.push(item);
  });
  return { fields, lineItems };
}
