import { Decimal } from "decimal.js";

const DECIMAL_INPUT = /^-?\d+(?:[.,]\d+)?$/;
const GERMAN_GROUPED_INPUT = /^-?\d{1,3}(?:\.\d{3})+(?:,\d+)?$/;

/** Converts German user/PDF input to the canonical dot-decimal representation. */
export function parseLocalizedDecimal(input: string): string | null {
  const compact = input
    .trim()
    .replace(/[\s\u00a0]/g, "")
    .replace(/(?:EUR|USD|GBP|CHF|€|\$|£)$/i, "")
    .replace(/%$/, "")
    .trim();
  if (!compact) return null;

  let normalized = compact;
  if (compact.includes(",")) {
    normalized = compact.replaceAll(".", "").replace(",", ".");
  } else if (GERMAN_GROUPED_INPUT.test(compact)) {
    normalized = compact.replaceAll(".", "");
  }
  if (!DECIMAL_INPUT.test(normalized)) return null;

  const [integer = "0", fraction] = normalized.split(".");
  const sign = integer.startsWith("-") ? "-" : "";
  const unsignedInteger = integer.replace(/^-/, "").replace(/^0+(?=\d)/, "") || "0";
  return `${sign}${unsignedInteger}${fraction === undefined ? "" : `.${fraction}`}`;
}

export function formatGermanDecimal(value: string, minimumFractionDigits = 2, maximumFractionDigits = 2): string {
  const canonical = parseLocalizedDecimal(value);
  if (canonical === null) return value;
  const rounded = new Decimal(canonical).toDecimalPlaces(maximumFractionDigits, Decimal.ROUND_HALF_UP);
  let [integer = "0", fraction = ""] = rounded.toFixed(maximumFractionDigits).split(".");
  while (fraction.length > minimumFractionDigits && fraction.endsWith("0")) fraction = fraction.slice(0, -1);
  const sign = integer.startsWith("-") ? "-" : "";
  integer = integer.replace(/^-/, "");
  const grouped = integer.replace(/\B(?=(\d{3})+(?!\d))/g, ".");
  return `${sign}${grouped}${fraction ? `,${fraction}` : ""}`;
}
