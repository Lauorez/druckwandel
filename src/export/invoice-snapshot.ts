import { calculateInvoice } from "../domain/calculate.js";
import { assertValidEn16931Invoice } from "../domain/validate.js";
import type { CalculatedInvoice, Party } from "../domain/types.js";
import { generateUbl } from "../engine/ubl.js";
import { generateCii } from "../engine/cii.js";

/** Stable serialization, independent of object insertion order; decimals remain strings. */
export function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value !== null && typeof value === "object") return `{${Object.entries(value).filter(([,v])=>v!==undefined).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([k,v])=>`${JSON.stringify(k)}:${canonicalJson(v)}`).join(",")}}`;
  const encoded = JSON.stringify(value);
  if (encoded === undefined) throw new Error("Nicht unterstützte Rechnungsangabe.");
  return encoded;
}
export function invoiceSnapshot(invoice: CalculatedInvoice): string {
  assertValidEn16931Invoice(invoice);
  if (canonicalJson(calculateInvoice(invoice)) !== canonicalJson(invoice)) throw new Error("Die gespeicherten Rechnungssummen passen nicht zu den Positionen.");
  return canonicalJson({schemaVersion:1,serializerVersion:"ubl-cii-v1",invoice});
}
export function readInvoiceSnapshot(contents: string, format?: "xrechnung"|"zugferd", xml?: string): CalculatedInvoice {
  try {
    const snapshot = JSON.parse(contents);
    if (snapshot.schemaVersion!==1 || snapshot.serializerVersion!=="ubl-cii-v1") throw new Error();
    const invoice = snapshot.invoice as CalculatedInvoice;
    if (!invoice || !Array.isArray(invoice.lines) || invoice.lines.length>10000
      || !invoice.lines.every(l=>typeof l.quantity==="string" && typeof l.netUnitPrice==="string" && typeof l.tax?.rate==="string")) throw new Error();
    if (invoiceSnapshot(invoice)!==contents) throw new Error();
    if (format && xml!==undefined && (format==="xrechnung"?generateUbl(invoice):generateCii(invoice))!==xml) throw new Error();
    return invoice;
  } catch { throw new Error("Der archivierte Rechnungsstand ist beschädigt, nicht unterstützt oder passt nicht zur Rechnungsdatei."); }
}
export function partyIdentity(party: Party): string {
  const normalize = (s: string|undefined) => (s??"").normalize("NFKC").trim().replace(/\s+/g," ").toUpperCase();
  return canonicalJson([normalize(party.name),normalize(party.address.line1),normalize(party.address.postalCode),normalize(party.address.city),normalize(party.address.countryCode),normalize(party.vatId),normalize(party.taxRegistrationId)]);
}
