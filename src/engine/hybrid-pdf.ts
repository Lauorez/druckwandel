import {
  AFRelationship,
  PDFArray,
  PDFDict,
  PDFDocument,
  PDFHexString,
  PDFName,
  PDFRawStream,
  PDFStream,
  PDFString,
  decodePDFRawStream,
  type PDFObject,
} from "pdf-lib";
import { SRGB_ICC } from "./srgb-icc.js";
import { HybridPdfError } from "./hybrid-pdf-error.js";
export { HybridPdfError } from "./hybrid-pdf-error.js";

export interface PdfAttachment {
  filename: string;
  bytes: Uint8Array;
  relationship?: string;
}

export const INVOICE_XML_NAMES = ["factur-x.xml", "zugferd-invoice.xml", "xrechnung.xml"] as const;

export interface HybridPdfOptions {
  xmlFilename?: "factur-x.xml" | "zugferd-invoice.xml";
  title?: string;
}

const FACTUR_X_FILENAME = "factur-x.xml";

function lookupDict(owner: PDFDict, name: string): PDFDict | undefined {
  return owner.lookupMaybe(PDFName.of(name), PDFDict);
}

function decodeName(value: PDFObject | undefined): string {
  if (!value) return "";
  if (value instanceof PDFString || value instanceof PDFHexString) return value.decodeText();
  const raw = value.toString();
  return raw.replace(/^\(|\)$/g, "").replace(/^</, "").replace(/>$/, "");
}

function decodeStream(stream: PDFStream): Uint8Array {
  if (stream instanceof PDFRawStream) return decodePDFRawStream(stream).decode();
  return stream.getContents();
}

function filespecBytes(filespec: PDFDict): Uint8Array | undefined {
  const ef = lookupDict(filespec, "EF");
  const stream = ef?.lookup(PDFName.of("F")) ?? ef?.lookup(PDFName.of("UF"));
  if (stream instanceof PDFStream) return decodeStream(stream);
  return undefined;
}

function filespecRelationship(filespec: PDFDict): string | undefined {
  const value = filespec.get(PDFName.of("AFRelationship"));
  return value ? value.toString().replace(/^\//, "") : undefined;
}

function walkNameTree(node: PDFDict, attachments: PdfAttachment[]): void {
  const names = node.lookupMaybe(PDFName.of("Names"), PDFArray);
  if (names) {
    for (let index = 0; index + 1 < names.size(); index += 2) {
      const filename = decodeName(names.get(index));
      const dict = names.lookupMaybe(index + 1, PDFDict);
      if (!dict || !filename) continue;
      const bytes = filespecBytes(dict);
      if (bytes) {
        const relationship = filespecRelationship(dict);
        attachments.push({ filename, bytes, ...(relationship ? { relationship } : {}) });
      }
    }
  }
  const kids = node.lookupMaybe(PDFName.of("Kids"), PDFArray);
  if (kids) {
    for (let index = 0; index < kids.size(); index += 1) {
      const child = kids.lookupMaybe(index, PDFDict);
      if (child) walkNameTree(child, attachments);
    }
  }
}

export async function listPdfAttachments(source: Uint8Array): Promise<PdfAttachment[]> {
  const document = await PDFDocument.load(source, { updateMetadata: false, ignoreEncryption: false });
  const attachments: PdfAttachment[] = [];
  const names = lookupDict(document.catalog, "Names");
  const embedded = names ? lookupDict(names, "EmbeddedFiles") : undefined;
  if (embedded) walkNameTree(embedded, attachments);
  const af = document.catalog.lookupMaybe(PDFName.of("AF"), PDFArray);
  if (af) {
    for (let index = 0; index < af.size(); index += 1) {
      const spec = af.lookupMaybe(index, PDFDict);
      if (!spec) continue;
      const filename = decodeName(spec.lookup(PDFName.of("UF")) ?? spec.lookup(PDFName.of("F")));
      const bytes = filespecBytes(spec);
      if (filename && bytes && !attachments.some((item) => item.filename === filename)) {
        const relationship = filespecRelationship(spec);
        attachments.push({ filename, bytes, ...(relationship ? { relationship } : {}) });
      }
    }
  }
  return attachments;
}

function isInvoiceXmlName(filename: string): boolean {
  const base = filename.split("/").pop()?.toLowerCase() ?? "";
  return INVOICE_XML_NAMES.includes(base as (typeof INVOICE_XML_NAMES)[number])
    || /zugferd.*\.xml$/i.test(base)
    || /factur[-_]?x\.xml$/i.test(base);
}

function looksLikeInvoiceXml(bytes: Uint8Array): boolean {
  const text = new TextDecoder("utf-8", { fatal: false }).decode(bytes.slice(0, 4096));
  return /CrossIndustryInvoice|<Invoice[\s>]/.test(text);
}

export async function extractFacturXXmlFromPdf(source: Uint8Array): Promise<string> {
  const attachments = await listPdfAttachments(source);
  const match = attachments.find((item) => item.filename.toLowerCase() === FACTUR_X_FILENAME)
    ?? attachments.find((item) => isInvoiceXmlName(item.filename));
  if (!match) {
    throw new HybridPdfError("XML_MISSING", "In der PDF-Rechnung fehlt die eingebettete Rechnungsdatei factur-x.xml.");
  }
  return new TextDecoder("utf-8").decode(match.bytes);
}

function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}

export async function assertInvoiceAttachmentsCompatible(source: Uint8Array, ciiXml: string): Promise<void> {
  const expected = new TextEncoder().encode(ciiXml);
  const attachments = await listPdfAttachments(source);
  for (const attachment of attachments) {
    const invoiceLike = isInvoiceXmlName(attachment.filename) || looksLikeInvoiceXml(attachment.bytes);
    if (!invoiceLike) continue;
    if (bytesEqual(attachment.bytes, expected) && attachment.filename.toLowerCase() === FACTUR_X_FILENAME) continue;
    throw new HybridPdfError(
      "EXISTING_INVOICE_XML",
      "Die PDF enthält bereits andere maschinenlesbare Rechnungsdaten. Diese werden nicht überschrieben oder verdoppelt. Bitte eine PDF ohne eingebettete E-Rechnung verwenden.",
    );
  }
}

function fontHasFile(descriptor: PDFDict | undefined): boolean {
  if (!descriptor) return false;
  return descriptor.has(PDFName.of("FontFile"))
    || descriptor.has(PDFName.of("FontFile2"))
    || descriptor.has(PDFName.of("FontFile3"));
}

function fontEmbedded(font: PDFDict, document: PDFDocument, seen: Set<PDFDict>): boolean {
  if (seen.has(font)) return true;
  seen.add(font);
  const subtype = font.get(PDFName.of("Subtype"))?.toString();
  if (subtype === "/Type3") return true;
  if (subtype === "/Type0") {
    const descendants = font.lookupMaybe(PDFName.of("DescendantFonts"), PDFArray);
    if (!descendants || descendants.size() === 0) return false;
    for (let index = 0; index < descendants.size(); index += 1) {
      const child = descendants.lookupMaybe(index, PDFDict);
      if (!child || !fontEmbedded(child, document, seen)) return false;
    }
    return true;
  }
  return fontHasFile(font.lookupMaybe(PDFName.of("FontDescriptor"), PDFDict));
}

function collectFonts(resources: PDFDict | undefined, fonts: PDFDict[], seen: Set<PDFDict>): void {
  if (!resources || seen.has(resources)) return;
  seen.add(resources);
  const fontDict = lookupDict(resources, "Font");
  if (fontDict) {
    for (const key of fontDict.keys()) {
      const font = fontDict.lookupMaybe(key, PDFDict);
      if (font) fonts.push(font);
    }
  }
  const xobjects = lookupDict(resources, "XObject");
  if (xobjects) {
    for (const key of xobjects.keys()) {
      const object = xobjects.lookup(key);
      if (object instanceof PDFStream) {
        const nested = lookupDict(object.dict, "Resources");
        collectFonts(nested, fonts, seen);
      } else if (object instanceof PDFDict) {
        collectFonts(lookupDict(object, "Resources") ?? object, fonts, seen);
      }
    }
  }
}

function ensureAllFontsEmbedded(document: PDFDocument): void {
  const fonts: PDFDict[] = [];
  const seenResources = new Set<PDFDict>();
  for (const page of document.getPages()) {
    collectFonts(page.node.Resources(), fonts, seenResources);
  }
  const seenFonts = new Set<PDFDict>();
  for (const font of fonts) {
    if (!fontEmbedded(font, document, seenFonts)) {
      throw new HybridPdfError(
        "UNEMBEDDED_FONT",
        "Diese PDF kann nicht als PDF/A-3 gespeichert werden, weil Schriften fehlen. Bitte die Rechnung im Ursprungsprogramm mit eingebetteten Schriften erneut ausgeben.",
      );
    }
  }
}

function ensureNoInteractiveForm(document: PDFDocument): void {
  const acro = lookupDict(document.catalog, "AcroForm");
  const fields = acro?.lookupMaybe(PDFName.of("Fields"), PDFArray);
  if (fields && fields.size() > 0) {
    throw new HybridPdfError(
      "FORM",
      "Formulare in der PDF können nicht sicher in eine PDF/A-3-Rechnung übernommen werden. Bitte eine normale Druck-PDF ohne Formularfelder verwenden.",
    );
  }
}

function addOutputIntent(document: PDFDocument): void {
  if (document.catalog.has(PDFName.of("OutputIntents"))) return;
  const icc = document.context.flateStream(SRGB_ICC, { N: 3 });
  const iccRef = document.context.register(icc);
  const intent = document.context.register(document.context.obj({
    Type: "OutputIntent",
    S: "GTS_PDFA1",
    OutputConditionIdentifier: PDFString.of("sRGB IEC61966-2.1"),
    OutputCondition: PDFString.of("sRGB IEC61966-2.1"),
    Info: PDFString.of("sRGB IEC61966-2.1"),
    RegistryName: PDFString.of("http://www.color.org"),
    DestOutputProfile: iccRef,
  }));
  document.catalog.set(PDFName.of("OutputIntents"), document.context.obj([intent]));
}

function updateFileIdentifier(document: PDFDocument): void {
  const identifier = PDFHexString.of(crypto.randomUUID().replaceAll("-", ""));
  const existing = document.context.trailerInfo.ID;
  const original = existing instanceof PDFArray ? existing.get(0) : undefined;
  document.context.trailerInfo.ID = document.context.obj([
    original instanceof PDFHexString || original instanceof PDFString ? original : identifier,
    identifier,
  ]);
}

export async function convertToPdfA3(sourcePdf: Uint8Array): Promise<Uint8Array> {
  let document: PDFDocument;
  try {
    document = await PDFDocument.load(sourcePdf, { updateMetadata: false, ignoreEncryption: false, throwOnInvalidObject: true });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (/encrypt/i.test(message)) {
      throw new HybridPdfError("ENCRYPTED", "Verschlüsselte PDFs können nicht in eine PDF/A-3-Rechnung übernommen werden.");
    }
    throw new HybridPdfError("DAMAGED", "Die PDF ist beschädigt oder kein unterstütztes PDF-Dokument.");
  }
  const pageCount = document.getPageCount();
  if (pageCount < 1) throw new HybridPdfError("DAMAGED", "Die PDF enthält keine Seiten.");
  ensureNoInteractiveForm(document);
  ensureAllFontsEmbedded(document);
  addOutputIntent(document);
  updateFileIdentifier(document);
  document.catalog.set(PDFName.of("Lang"), PDFString.of("de-DE"));
  const markInfo = document.context.obj({ Marked: true });
  document.catalog.set(PDFName.of("MarkInfo"), document.context.register(markInfo));
  const output = await document.save({ useObjectStreams: false, addDefaultPage: false, updateFieldAppearances: false });
  const reopened = await PDFDocument.load(output, { updateMetadata: false });
  if (reopened.getPageCount() !== pageCount) {
    throw new HybridPdfError("CONVERT", "Die PDF/A-Umwandlung hat die Seitenzahl verändert. Die Datei wird nicht ausgegeben.");
  }
  return output;
}

function facturXmp(filename: string, title: string): string {
  return `<?xpacket begin="\uFEFF" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/" pdfaid:part="3" pdfaid:conformance="B"/><rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:format>application/pdf</dc:format><dc:title><rdf:Alt><rdf:li xml:lang="x-default">${escapeXml(title)}</rdf:li></rdf:Alt></dc:title></rdf:Description><rdf:Description rdf:about="" xmlns:pdf="http://ns.adobe.com/pdf/1.3/" pdf:Producer="E-Rechnungs-Assistent"/><rdf:Description rdf:about="" xmlns:fx="urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#" fx:DocumentType="INVOICE" fx:DocumentFileName="${filename}" fx:Version="1.0" fx:ConformanceLevel="EN 16931"/><rdf:Description rdf:about="" xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/" xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#" xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#"><pdfaExtension:schemas><rdf:Bag><rdf:li rdf:parseType="Resource"><pdfaSchema:schema>Factur-X PDFA Extension Schema</pdfaSchema:schema><pdfaSchema:namespaceURI>urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#</pdfaSchema:namespaceURI><pdfaSchema:prefix>fx</pdfaSchema:prefix><pdfaSchema:property><rdf:Seq><rdf:li rdf:parseType="Resource"><pdfaProperty:name>DocumentFileName</pdfaProperty:name><pdfaProperty:valueType>Text</pdfaProperty:valueType><pdfaProperty:category>external</pdfaProperty:category><pdfaProperty:description>Name of the embedded XML invoice file</pdfaProperty:description></rdf:li><rdf:li rdf:parseType="Resource"><pdfaProperty:name>DocumentType</pdfaProperty:name><pdfaProperty:valueType>Text</pdfaProperty:valueType><pdfaProperty:category>external</pdfaProperty:category><pdfaProperty:description>INVOICE</pdfaProperty:description></rdf:li><rdf:li rdf:parseType="Resource"><pdfaProperty:name>Version</pdfaProperty:name><pdfaProperty:valueType>Text</pdfaProperty:valueType><pdfaProperty:category>external</pdfaProperty:category><pdfaProperty:description>Factur-X version</pdfaProperty:description></rdf:li><rdf:li rdf:parseType="Resource"><pdfaProperty:name>ConformanceLevel</pdfaProperty:name><pdfaProperty:valueType>Text</pdfaProperty:valueType><pdfaProperty:category>external</pdfaProperty:category><pdfaProperty:description>EN 16931</pdfaProperty:description></rdf:li></rdf:Seq></pdfaSchema:property></rdf:li></rdf:Bag></pdfaExtension:schemas></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>`;
}

function escapeXml(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

export async function embedCiiInPdf(sourcePdf: Uint8Array, ciiXml: string, options: HybridPdfOptions = {}): Promise<Uint8Array> {
  const filename = options.xmlFilename ?? FACTUR_X_FILENAME;
  if (filename !== FACTUR_X_FILENAME) {
    throw new HybridPdfError("XML_NAME", "Die Hybrid-PDF verwendet factur-x.xml als eingebetteten Dateinamen.");
  }
  await assertInvoiceAttachmentsCompatible(sourcePdf, ciiXml);
  const attachments = await listPdfAttachments(sourcePdf);
  const identical = attachments.some((item) => item.filename.toLowerCase() === filename && bytesEqual(item.bytes, new TextEncoder().encode(ciiXml)));
  const document = await PDFDocument.load(sourcePdf, { updateMetadata: false, ignoreEncryption: false });
  if (!identical) {
    await document.attach(new TextEncoder().encode(ciiXml), filename, {
      mimeType: "text/xml",
      description: "Factur-X/ZUGFeRD EN 16931 invoice data",
      afRelationship: AFRelationship.Alternative,
      creationDate: new Date(0),
      modificationDate: new Date(0),
    });
  }
  const title = options.title ?? "E-Rechnung";
  document.setTitle(title);
  document.setProducer("E-Rechnungs-Assistent");
  document.setCreator("E-Rechnungs-Assistent");
  const metadata = document.context.stream(new TextEncoder().encode(facturXmp(filename, title)), {
    Type: PDFName.of("Metadata"),
    Subtype: PDFName.of("XML"),
  });
  document.catalog.set(PDFName.of("Metadata"), document.context.register(metadata));
  document.catalog.set(PDFName.of("Lang"), PDFString.of("de-DE"));
  addOutputIntent(document);
  updateFileIdentifier(document);
  return document.save({ useObjectStreams: false, addDefaultPage: false, updateFieldAppearances: false });
}

export async function prepareHybridPdf(sourcePdf: Uint8Array, ciiXml: string, options: HybridPdfOptions = {}): Promise<Uint8Array> {
  const pdfa = await convertToPdfA3(sourcePdf);
  const hybrid = await embedCiiInPdf(pdfa, ciiXml, options);
  const extracted = await extractFacturXXmlFromPdf(hybrid);
  if (extracted !== ciiXml) {
    throw new HybridPdfError("XML_MISMATCH", "Die eingebetteten Rechnungsdaten weichen von der geprüften XML-Datei ab. Es wird keine fertige PDF erzeugt.");
  }
  return hybrid;
}
