import { PDFDocument, StandardFonts, type PDFFont, type PDFPage } from "pdf-lib";
import { decimal, money } from "../domain/money.js";
import { formatGermanDecimal } from "../domain/localized-decimal.js";
import type { CorpusCase, ExpectedLineItem } from "./corpus.js";

export type SyntheticLayout = "headed-separated" | "headed-merged" | "legacy-columns";

export interface SyntheticInvoiceLine {
  description: string;
  serviceDate: string;
  quantity: string;
  unit: string;
  displayUnit: string;
  netUnitPrice: string;
  netAmount: string;
  taxRate: string;
}

export interface SyntheticInvoiceSource {
  seed: number;
  layout: SyntheticLayout;
  invoiceNumber: string;
  issueDate: string;
  dueDate: string;
  serviceDate: string;
  currency: "EUR";
  currencyMarker: "EUR" | "€";
  buyerReference: string;
  seller: { name: string; addressLine1: string; postalCode: string; city: string; vatId: string };
  buyer: { name: string; addressLine1: string; postalCode: string; city: string };
  iban: string;
  bic: string;
  taxRate: string;
  lines: SyntheticInvoiceLine[];
  totals: { lineNet: string; taxTotal: string; payable: string };
}

export interface SyntheticInvoiceFixture {
  source: SyntheticInvoiceSource;
  pdf: Uint8Array;
  corpusCase: CorpusCase;
}

export class SeededRandom {
  private state: number;

  constructor(seed: number) {
    this.state = seed >>> 0;
  }

  next(): number {
    this.state = (this.state + 0x6d2b79f5) >>> 0;
    let value = this.state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4_294_967_296;
  }

  integer(minimum: number, maximum: number): number {
    return minimum + Math.floor(this.next() * (maximum - minimum + 1));
  }

  pick<T>(values: readonly T[]): T {
    const value = values[this.integer(0, values.length - 1)];
    if (value === undefined) throw new Error("Cannot pick from an empty collection.");
    return value;
  }

  boolean(): boolean {
    return this.next() >= 0.5;
  }
}

const DESCRIPTIONS = [
  "Softwarepflege",
  "Technische Beratung",
  "Datenmigration",
  "Einrichtung",
  "Supportpaket",
  "Lizenz",
  "Geräteprüfung",
  "Dokumentation",
] as const;
const QUANTITIES = ["0.5", "1", "1.25", "2", "2.5", "3", "5", "10"] as const;
const PRICES = ["9.90", "12.50", "49.95", "79.00", "95.00", "149.99", "1250.00"] as const;
const UNITS = [
  { unit: "Stk", display: "Stk." },
  { unit: "Std", display: "Std." },
  { unit: "m", display: "m" },
] as const;
const SELLERS = [
  { name: "Anbieter GmbH", street: "Beispielstrasse 12", postalCode: "10115", city: "Berlin" },
  { name: "Müller Technik GmbH", street: "Werkweg 8", postalCode: "50667", city: "Köln" },
  { name: "Nordlicht Service KG", street: "Hafenallee 21", postalCode: "20095", city: "Hamburg" },
] as const;
const BUYERS = [
  { name: "Kunde AG", street: "Kundenweg 5", postalCode: "20095", city: "Hamburg" },
  { name: "Beispiel Handel GmbH", street: "Marktstrasse 17", postalCode: "80331", city: "München" },
  { name: "Testbetrieb OHG", street: "Feldweg 3", postalCode: "01067", city: "Dresden" },
] as const;

function isoDate(day: number): string {
  return `2026-08-${String(day).padStart(2, "0")}`;
}

function germanDate(iso: string, shortYear = false): string {
  const [year, month, day] = iso.split("-");
  return `${day}.${month}.${shortYear ? year?.slice(-2) : year}`;
}

function amountForDisplay(value: string): string {
  return formatGermanDecimal(value, 2, 2);
}

function quantityForDisplay(value: string): string {
  return formatGermanDecimal(value, 0, 3);
}

function drawText(page: PDFPage, font: PDFFont, fontSize: number, text: string, x: number, y: number) {
  page.drawText(text, { x, y, size: fontSize, font });
}

function expectedLine(line: SyntheticInvoiceLine, includeServiceDate: boolean): ExpectedLineItem {
  return {
    description: line.description,
    ...(includeServiceDate ? { serviceDate: line.serviceDate } : {}),
    quantity: decimal(line.quantity).toFixed(3),
    unit: line.unit,
    netUnitPrice: money(line.netUnitPrice),
    netAmount: line.netAmount,
    taxRate: decimal(line.taxRate).toFixed(2),
  };
}

function expectedCase(source: SyntheticInvoiceSource): CorpusCase {
  const commonFields = {
    invoiceNumber: source.invoiceNumber,
    currency: source.currency,
    sellerVatId: source.seller.vatId,
    lineNet: source.totals.lineNet,
    taxTotal: source.totals.taxTotal,
    taxInclusive: source.totals.payable,
    payable: source.totals.payable,
  } as const;
  const headedFields = {
    ...commonFields,
    issueDate: source.issueDate,
    dueDate: source.dueDate,
    serviceDate: source.serviceDate,
    buyerReference: source.buyerReference,
    sellerName: source.seller.name,
    sellerAddressLine1: source.seller.addressLine1,
    sellerPostalCode: source.seller.postalCode,
    sellerCity: source.seller.city,
    sellerCountryCode: "DE",
    buyerName: source.buyer.name,
    buyerAddressLine1: source.buyer.addressLine1,
    buyerPostalCode: source.buyer.postalCode,
    buyerCity: source.buyer.city,
    buyerCountryCode: "DE",
    iban: source.iban,
    bic: source.bic,
  } as const;
  return {
    id: `synthetic-${source.seed}`,
    description: `Seed ${source.seed}, Layout ${source.layout}`,
    pdf: `synthetic://${source.seed}`,
    expected: {
      fields: source.layout === "legacy-columns" ? commonFields : headedFields,
      lineItems: source.lines.map((line) => expectedLine(line, source.layout === "legacy-columns")),
      warningCodes: [],
      unsupportedCaseCodes: [],
    },
  };
}

function createSource(seed: number): SyntheticInvoiceSource {
  const random = new SeededRandom(seed);
  const layout = random.pick<SyntheticLayout>(["headed-separated", "headed-merged", "legacy-columns"]);
  const issueDay = random.integer(1, 20);
  const issueDate = isoDate(issueDay);
  const dueDate = isoDate(Math.min(28, issueDay + random.integer(7, 14)));
  const serviceDate = isoDate(Math.max(1, issueDay - random.integer(0, 3)));
  const sellerTemplate = random.pick(SELLERS);
  const buyerTemplate = random.pick(BUYERS);
  const taxRate = random.pick(["7", "19"] as const);
  const lineCount = seed % 29 === 0 ? random.integer(24, 30) : random.integer(1, 12);
  const lines = Array.from({ length: lineCount }, (_, index) => {
    const quantity = random.pick(QUANTITIES);
    const netUnitPrice = random.pick(PRICES);
    const unit = random.pick(UNITS);
    return {
      description: `${random.pick(DESCRIPTIONS)} ${index + 1}`,
      serviceDate: isoDate(Math.max(1, issueDay - index % 5)),
      quantity,
      unit: unit.unit,
      displayUnit: unit.display,
      netUnitPrice,
      netAmount: money(decimal(quantity).mul(netUnitPrice)),
      taxRate,
    };
  });
  const lineNet = money(lines.reduce((sum, line) => sum.add(line.netAmount), decimal(0)));
  const taxTotal = money(decimal(lineNet).mul(taxRate).div(100));
  const payable = money(decimal(lineNet).add(taxTotal));
  const vatDigits = String(100_000_000 + seed % 900_000_000);

  return {
    seed,
    layout,
    invoiceNumber: `RE-2026-${String(seed).padStart(6, "0")}`,
    issueDate,
    dueDate,
    serviceDate,
    currency: "EUR",
    currencyMarker: random.boolean() ? "EUR" : "€",
    buyerReference: `04011000-${String(seed).padStart(5, "0")}-03`,
    seller: { name: sellerTemplate.name, addressLine1: sellerTemplate.street, postalCode: sellerTemplate.postalCode, city: sellerTemplate.city, vatId: `DE${vatDigits}` },
    buyer: { name: buyerTemplate.name, addressLine1: buyerTemplate.street, postalCode: buyerTemplate.postalCode, city: buyerTemplate.city },
    iban: "DE89370400440532013000",
    bic: "COBADEFFXXX",
    taxRate,
    lines,
    totals: { lineNet, taxTotal, payable },
  };
}

async function renderSource(source: SyntheticInvoiceSource): Promise<Uint8Array> {
  const document = await PDFDocument.create();
  const font = await document.embedFont(StandardFonts.Helvetica);
  const fontSize = 9 + source.seed % 3;
  const jitter = (source.seed % 7) - 3;
  let page = document.addPage([595, 842]);
  let y = 0;
  const draw = (text: string, x: number, targetY: number) => drawText(page, font, fontSize, text, x + jitter, targetY);
  const tableHeader = (targetY: number) => {
    if (source.layout === "legacy-columns") return;
    draw("Pos.", 58, targetY);
    draw("Beschreibung", 90, targetY);
    draw("Menge", 278, targetY);
    draw("Einzelpreis", 338, targetY);
    draw("Gesamt", 425, targetY);
    draw("USt.", 520, targetY);
  };
  const newContinuationPage = () => {
    page = document.addPage([595, 842]);
    y = 770;
    tableHeader(y);
    y -= 16;
  };

  if (source.layout === "legacy-columns") {
    draw("RECHNUNG", 60, 790);
    draw("Beleg-Nr:", 420, 760);
    draw(source.invoiceNumber, 480, 760);
    draw(`Beleg-Datum: ${germanDate(source.issueDate)}`, 420, 742);
    y = 690;
  } else {
    draw("RECHNUNG", 60, 805);
    draw(source.seller.name, 69, 780);
    draw(source.seller.addressLine1, 69, 764);
    draw(`${source.seller.postalCode} ${source.seller.city}`, 69, 748);
    draw(`USt-IdNr.: ${source.seller.vatId}`, 69, 732);
    draw("Rechnung an:", 69, 702);
    draw(source.buyer.name, 69, 686);
    draw(source.buyer.addressLine1, 69, 670);
    draw(`${source.buyer.postalCode} ${source.buyer.city}`, 69, 654);
    draw(`Rechnungsnummer: ${source.invoiceNumber}`, 69, 622);
    draw(`Leitweg-ID: ${source.buyerReference}`, 330, 622);
    draw(`Rechnungsdatum: ${germanDate(source.issueDate)}`, 69, 606);
    draw(`Leistungsdatum: ${germanDate(source.serviceDate)}`, 69, 590);
    draw(`Zahlbar bis: ${germanDate(source.dueDate)}`, 69, 574);
    draw("Ansprechpartner: Buchhaltung", 330, 606);
    draw("Telefon: +49 30 1234567", 330, 590);
    draw("E-Mail: rechnung@muster.invalid", 330, 574);
    y = 530;
    tableHeader(y);
    y -= 16;
  }

  source.lines.forEach((line, index) => {
    if (y < 115) newContinuationPage();
    const quantity = `${quantityForDisplay(line.quantity)} ${line.displayUnit}`;
    const price = `${amountForDisplay(line.netUnitPrice)} ${source.currencyMarker}`;
    const total = `${amountForDisplay(line.netAmount)} ${source.currencyMarker}`;
    if (source.layout === "legacy-columns") {
      draw(`${germanDate(line.serviceDate, true)} ${line.description}`, 60, y);
      draw(quantity, 300, y);
      draw(amountForDisplay(line.netUnitPrice), 390, y);
      draw(amountForDisplay(line.netAmount), 460, y);
      draw(decimal(line.taxRate).toFixed(1).replace(".", ","), 530, y);
    } else {
      draw(String(index + 1), 58, y);
      if (source.layout === "headed-merged") draw(`${line.description} ${quantity}`, 90, y);
      else {
        draw(line.description, 90, y);
        draw(quantity, 278, y);
      }
      draw(price, 348, y);
      draw(total, 430, y);
      draw(`${line.taxRate} %`, 520, y);
    }
    y -= 16;
  });

  if (y < 100) {
    page = document.addPage([595, 842]);
    y = 760;
  }
  y -= 12;
  if (source.layout === "legacy-columns") {
    draw(`Umsatzsteuer ${decimal(source.taxRate).toFixed(2).replace(".", ",")} %`, 60, y);
    draw(amountForDisplay(source.totals.lineNet), 285, y);
    draw(decimal(source.taxRate).toFixed(1).replace(".", ","), 355, y);
    draw(amountForDisplay(source.totals.taxTotal), 420, y);
    draw(amountForDisplay(source.totals.payable), 485, y);
    draw(source.currencyMarker, 550, y);
    draw(`USt.-IdNr.: ${source.seller.vatId}`, 60, 60);
    draw("Ansprechpartner: Buchhaltung", 60, 44);
    draw("Telefon: +49 30 1234567", 250, 44);
    draw("E-Mail: rechnung@muster.invalid", 60, 28);
  } else {
    draw(`Netto: ${amountForDisplay(source.totals.lineNet)} ${source.currencyMarker}`, 338, y);
    draw(`USt. ${source.taxRate} %: ${amountForDisplay(source.totals.taxTotal)} ${source.currencyMarker}`, 338, y - 16);
    draw(`Gesamt: ${amountForDisplay(source.totals.payable)} ${source.currencyMarker}`, 338, y - 32);
    draw("Zahlungsbedingungen:", 69, Math.max(80, y - 70));
    draw("Zahlbar innerhalb von 14 Tagen ohne Abzug.", 69, Math.max(64, y - 86));
    draw(`IBAN: ${source.iban.replace(/(.{4})/g, "$1 ").trim()}`, 69, 46);
    draw(`BIC: ${source.bic}`, 330, 46);
  }
  return document.save();
}

export async function generateSyntheticInvoice(seed: number): Promise<SyntheticInvoiceFixture> {
  if (!Number.isSafeInteger(seed) || seed < 0) throw new Error("Seed must be a non-negative safe integer.");
  const source = createSource(seed);
  return { source, pdf: await renderSource(source), corpusCase: expectedCase(source) };
}
