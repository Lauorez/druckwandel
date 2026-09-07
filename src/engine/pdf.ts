import { AFRelationship, PDFDocument, PDFName, PDFString } from "pdf-lib";

export interface HybridPdfOptions {
  xmlFilename?: "factur-x.xml" | "zugferd-invoice.xml";
  title?: string;
}

/**
 * Embeds EN16931 CII into a PDF and writes Factur-X PDF/A extension metadata.
 * The source must already be PDF/A-3. Final conformance must be checked with veraPDF.
 */
export async function embedCiiInPdf(sourcePdf: Uint8Array, ciiXml: string, options: HybridPdfOptions = {}): Promise<Uint8Array> {
  const document = await PDFDocument.load(sourcePdf, { updateMetadata: false });
  const filename = options.xmlFilename ?? "factur-x.xml";
  await document.attach(new TextEncoder().encode(ciiXml), filename, {
    mimeType: "application/xml",
    description: "Factur-X/ZUGFeRD EN 16931 invoice data",
    afRelationship: AFRelationship.Alternative,
    creationDate: new Date(0),
    modificationDate: new Date(0),
  });
  document.setTitle(options.title ?? "E-Rechnung");
  document.setProducer("E-Rechnungs-Assistent 0.2");

  const xmp = `<?xpacket begin="﻿" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/" pdfaid:part="3" pdfaid:conformance="B"/><rdf:Description rdf:about="" xmlns:fx="urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#" fx:DocumentType="INVOICE" fx:DocumentFileName="${filename}" fx:Version="1.0" fx:ConformanceLevel="EN 16931"/></rdf:RDF></x:xmpmeta><?xpacket end="w"?>`;
  const metadata = document.context.stream(new TextEncoder().encode(xmp), {
    Type: PDFName.of("Metadata"),
    Subtype: PDFName.of("XML"),
  });
  document.catalog.set(PDFName.of("Metadata"), document.context.register(metadata));
  document.catalog.set(PDFName.of("Lang"), PDFString.of("de-DE"));
  return document.save({ useObjectStreams: false, addDefaultPage: false, updateFieldAppearances: false });
}
