import { PDFDocument, StandardFonts } from "pdf-lib";
import { describe, expect, it } from "vitest";
import { calculateInvoice } from "../src/domain/calculate.js";
import { generateCii } from "../src/engine/cii.js";
import { convertToPdfA3, embedCiiInPdf, extractFacturXXmlFromPdf, HybridPdfError, prepareHybridPdf } from "../src/engine/hybrid-pdf.js";
import { standardInvoice } from "./fixtures/invoice.js";

const invoice = calculateInvoice(standardInvoice);
const xml = generateCii(invoice);

async function blankPdf(): Promise<Uint8Array> {
  const source = await PDFDocument.create();
  source.addPage([420, 595]);
  return source.save();
}

describe("hybrid PDF/A preparation", () => {
  it("embeds the exact CII bytes and keeps the page count", async () => {
    const hybrid = await prepareHybridPdf(await blankPdf(), xml);
    expect(await extractFacturXXmlFromPdf(hybrid)).toBe(xml);
    const reopened = await PDFDocument.load(hybrid);
    expect(reopened.getPageCount()).toBe(1);
    const text = new TextDecoder("latin1").decode(hybrid);
    expect(text).toContain("/Alternative");
    expect(text).toContain("pdfaid:part=\"3\"");
    expect(text).toContain("pdfaSchema:namespaceURI");
  });

  it("rejects a second, different e-invoice attachment", async () => {
    const first = await embedCiiInPdf(await blankPdf(), xml);
    await expect(embedCiiInPdf(first, xml.replace("282.08", "282.09"))).rejects.toMatchObject({ code: "EXISTING_INVOICE_XML" } satisfies Partial<HybridPdfError>);
  });

  it("does not duplicate an identical factur-x.xml attachment", async () => {
    const first = await embedCiiInPdf(await blankPdf(), xml);
    const second = await embedCiiInPdf(first, xml);
    expect(await extractFacturXXmlFromPdf(second)).toBe(xml);
    expect([...new TextDecoder("latin1").decode(second).matchAll(/factur-x\.xml/g)].length).toBeLessThanOrEqual(
      [...new TextDecoder("latin1").decode(first).matchAll(/factur-x\.xml/g)].length + 1,
    );
  });

  it("blocks PDFs that use unembedded standard fonts", async () => {
    const source = await PDFDocument.create();
    const page = source.addPage();
    const font = await source.embedFont(StandardFonts.Helvetica);
    page.drawText("Rechnung 100,00", { font, size: 12, x: 48, y: 500 });
    await expect(convertToPdfA3(await source.save())).rejects.toMatchObject({ code: "UNEMBEDDED_FONT" });
  });
});
