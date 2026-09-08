import { isIsoDate } from "../domain/validate.js";

const MONTH_ALIASES: Array<[RegExp, number, string[]]> = [
  [/jan(?:uar)?/i, 1, ["januar", "jan"]],
  [/febr?(?:uar)?/i, 2, ["februar", "feb", "febr"]],
  [/(?:märz|maerz|mrz|mär)/i, 3, ["maerz", "marz", "mrz", "mar", "märz"]],
  [/apr(?:il)?/i, 4, ["april", "apr"]],
  [/mai/i, 5, ["mai"]],
  [/juni|jun/i, 6, ["juni", "jun"]],
  [/juli|jul/i, 7, ["juli", "jul"]],
  [/aug(?:ust)?/i, 8, ["august", "aug"]],
  [/sept(?:ember)?|sep/i, 9, ["september", "sept", "sep"]],
  [/okt(?:ober)?/i, 10, ["oktober", "okt"]],
  [/nov(?:ember)?/i, 11, ["november", "nov"]],
  [/dez(?:ember)?|dec(?:ember)?/i, 12, ["dezember", "dez", "december", "dec"]],
];

const MONTH_PATTERN = MONTH_ALIASES.map(([pattern]) => pattern.source).join("|");
const NUMERIC_DATE = /\b(\d{1,2})[./-](\d{1,2})[./-](\d{2,4})\b/;
const ISO_DATE = /\b(\d{4})-(\d{2})-(\d{2})\b/;
const NAMED_DATE = new RegExp(`\\b(\\d{1,2})\\.?\\s*(${MONTH_PATTERN})\\.?\\s+(\\d{4})\\b`, "i");
const RELATIVE_DUE_DAYS = /(?:fällig(?:keit)?|faellig(?:keit)?|zahlbar|zu\s+zahlen)(?:\s+(?:am|bis|innerhalb|binnen|in))?\s*(?:von\s+)?(\d{1,3})\s+tagen?|(?:zahlungsziel|zahlungsfrist)\s*:?\s*(\d{1,3})\s+tage(?:n)?|(?:binnen|innerhalb(?:\s+von)?)\s+(\d{1,3})\s+tagen?|(\d{1,3})\s+tage(?:n)?\s+netto|netto\s+(\d{1,3})\s+tage(?:n)?/i;

function monthNumber(name: string): number | undefined {
  return MONTH_ALIASES.find(([pattern]) => pattern.test(name.replace(/\.$/, "")))?.[1];
}

function isoFromParts(year: string, month: string | number, day: string): string | null {
  const fullYear = year.length === 2 ? `20${year}` : year;
  const iso = `${fullYear}-${String(month).padStart(2, "0")}-${day.padStart(2, "0")}`;
  return isIsoDate(iso) ? iso : null;
}

export function parseInvoiceDate(value: string): string | null {
  const iso = value.match(ISO_DATE);
  if (iso?.[1] && iso[2] && iso[3]) return isoFromParts(iso[1], iso[2], iso[3]);
  const named = value.match(NAMED_DATE);
  if (named?.[1] && named[2] && named[3]) {
    const month = monthNumber(named[2]);
    if (month) return isoFromParts(named[3], month, named[1]);
  }
  const numeric = value.match(NUMERIC_DATE);
  if (numeric?.[1] && numeric[2] && numeric[3]) return isoFromParts(numeric[3], numeric[2], numeric[1]);
  return null;
}

export function parseRelativeDueDays(value: string): number | null {
  const match = value.match(RELATIVE_DUE_DAYS);
  const days = Number(match?.slice(1).find((part) => part));
  return Number.isInteger(days) && days > 0 && days <= 366 ? days : null;
}

export function addIsoDays(iso: string, days: number): string | null {
  if (!isIsoDate(iso)) return null;
  const [year, month, day] = iso.split("-").map(Number);
  if (!year || !month || !day) return null;
  const date = new Date(Date.UTC(year, month - 1, day + days));
  return date.toISOString().slice(0, 10);
}

export function relativeDueDate(value: string, issueDate: string | undefined): string | null {
  const days = parseRelativeDueDays(value);
  return days === null || !issueDate ? null : addIsoDays(issueDate, days);
}

export function dateSearchNeedles(iso: string): string[] {
  const match = iso.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match?.[1] || !match[2] || !match[3]) return [];
  const year = match[1];
  const month = match[2];
  const day = match[3];
  const names = MONTH_ALIASES.find(([, number]) => number === Number(month))?.[2] ?? [];
  const dayNumber = String(Number(day));
  return [
    `${day}${month}${year}`,
    `${day}${month}${year.slice(2)}`,
    `${dayNumber}${month}${year}`,
    ...names.flatMap((name) => [`${dayNumber}${name}${year}`, `${day}${name}${year}`]),
  ];
}
