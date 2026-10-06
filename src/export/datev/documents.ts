import {
  DATEV_DOCUMENT_NS,
  DATEV_DOCUMENT_PROCESS_INBOX,
  DATEV_DOCUMENT_SCHEMA,
  DATEV_DOCUMENT_TYPE_OUTGOING,
  DATEV_GENERATING_SYSTEM,
} from "./contract.js";
import type { Booking, DatevDocumentFile, DatevDocumentPackage } from "./types.js";

const GUID = /^[0-9A-F]{8}-[0-9A-F]{4}-5[0-9A-F]{3}-[89AB][0-9A-F]{3}-[0-9A-F]{12}$/;

export function documentGuid(contentHash: string): string {
  const hex = contentHash.toLowerCase().replace(/[^0-9a-f]/g, "");
  if (hex.length < 32) throw new Error("Für den Beleglink fehlt der Rechnungsstand.");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-5${hex.slice(13, 16)}-A${hex.slice(17, 20)}-${hex.slice(20, 32)}`.toUpperCase();
}

export function documentLink(guid: string): string {
  if (!GUID.test(guid)) throw new Error("Ungültige Belegkennung.");
  return `BEDI "${guid}"`;
}

function escapeXml(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

export function documentFiles(bookings: Booking[]): DatevDocumentFile[] {
  const files: DatevDocumentFile[] = [];
  const seen = new Set<string>();
  for (const booking of bookings) {
    if (seen.has(booking.archiveId)) continue;
    seen.add(booking.archiveId);
    const guid = booking.documentGuid || documentGuid(booking.contentHash);
    if (guid !== documentGuid(booking.contentHash)) throw new Error("Die Belegkennung passt nicht zum Rechnungsstand.");
    files.push({ archiveId: booking.archiveId, guid, pdfName: `${guid}.pdf`, xmlName: `${guid}.xml` });
  }
  return files;
}

export function serializeDocumentXml(files: DatevDocumentFile[], createdAt: Date): string {
  if (!files.length || !Number.isFinite(createdAt.getTime())) throw new Error("Leeres oder ungültiges Belegpaket.");
  const stamp = createdAt.toISOString().slice(0, 19);
  const documents = files.map((file) => {
    documentLink(file.guid);
    if (file.pdfName !== `${file.guid}.pdf` || file.xmlName !== `${file.guid}.xml`) {
      throw new Error("Die Belegdateinamen müssen der Belegkennung entsprechen.");
    }
    return `    <document guid="${file.guid}" type="${DATEV_DOCUMENT_TYPE_OUTGOING}" processID="${DATEV_DOCUMENT_PROCESS_INBOX}">
      <extension xsi:type="File" name="${file.pdfName}"/>
      <extension xsi:type="File" name="${file.xmlName}"/>
    </document>`;
  });
  return `<?xml version="1.0" encoding="utf-8"?>
<archive xmlns="${DATEV_DOCUMENT_NS}" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="${DATEV_DOCUMENT_NS} ${DATEV_DOCUMENT_SCHEMA}" version="6.0" generatingSystem="${escapeXml(DATEV_GENERATING_SYSTEM)}">
  <header>
    <date>${stamp}</date>
  </header>
  <content>
${documents.join("\n")}
  </content>
</archive>
`;
}

export function documentPackage(bookings: Booking[], createdAt: Date): DatevDocumentPackage {
  const files = documentFiles(bookings);
  return { xml: serializeDocumentXml(files, createdAt), files };
}
