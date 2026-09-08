import type { ValidationIssue } from "../domain/types.js";

export type OfficialEngine = "kosit" | "mustang" | "verapdf";
export type OfficialValidationStatus =
  | "passed"
  | "failed"
  | "timeout"
  | "missing-report"
  | "unreadable-report"
  | "cancelled"
  | "unavailable"
  | "unknown";

export interface OfficialValidationIssue extends ValidationIssue {
  engine: OfficialEngine;
}

export interface OfficialValidationReport {
  schemaVersion: 1;
  status: OfficialValidationStatus;
  valid: boolean;
  engine: OfficialEngine;
  engineVersion: string;
  ruleVersion: string;
  xmlSha256: string;
  pdfSha256: string;
  reportSha256?: string;
  issues: OfficialValidationIssue[];
}

export interface ProcessOutcome {
  code: number;
  output: string;
  reportXml?: string;
  timedOut?: boolean;
  cancelled?: boolean;
}

const FIELD_HINTS: Array<[RegExp, string]> = [
  [/BuyerReference|BT-10|BR-DE-15|Leitweg/i, "buyerReference"],
  [/ID\[|cbc:ID|InvoiceNumber|BT-1\b|BR-02/i, "invoiceNumber"],
  [/IssueDate|BT-2\b/i, "issueDate"],
  [/DueDate|BT-9\b/i, "dueDate"],
  [/DocumentCurrency|BT-5\b|BR-05/i, "currency"],
  [/EndpointID|BT-34|electronic address|elektronische Adresse/i, "seller.vatId"],
  [/BT-49|Buyer.*Endpoint/i, "buyer.vatId"],
  [/BR-DE-2|BR-DE-5|Contact|Ansprechpartner|BT-41/i, "seller.contact.name"],
  [/BR-DE-6|BT-42|Telephone/i, "seller.contact.phone"],
  [/BR-DE-7|BT-43|ElectronicMail|E-Mail/i, "seller.contact.email"],
  [/AccountingSupplier|SellerTradeParty|seller.*name|BR-06/i, "seller.name"],
  [/Seller.*Street|Seller.*Line|seller.*address\.line1/i, "seller.address.line1"],
  [/Seller.*City|seller.*address\.city/i, "seller.address.city"],
  [/Seller.*Postal|seller.*address\.postalCode/i, "seller.address.postalCode"],
  [/Seller.*Country|seller.*address\.countryCode/i, "seller.address.countryCode"],
  [/AccountingCustomer|BuyerTradeParty|buyer.*name|BR-07/i, "buyer.name"],
  [/Buyer.*Street|Buyer.*Line|buyer.*address\.line1/i, "buyer.address.line1"],
  [/Buyer.*City|buyer.*address\.city/i, "buyer.address.city"],
  [/Buyer.*Postal|buyer.*address\.postalCode/i, "buyer.address.postalCode"],
  [/Buyer.*Country|buyer.*address\.countryCode/i, "buyer.address.countryCode"],
  [/InvoiceLine|IncludedSupplyChainTradeLineItem|BR-16|lines/i, "lines"],
  [/GrandTotal|PayableAmount|BT-112|BR-CO-15|totals\.taxInclusive/i, "totals.taxInclusive"],
  [/TaxTotal|BT-110|BR-CO-14|totals\.taxTotal/i, "totals.taxTotal"],
  [/DuePayable|BT-115|BR-CO-16|totals\.payable/i, "totals.payable"],
];

export function mapOfficialLocationToPath(location: string, code = ""): string {
  const haystack = `${location} ${code}`;
  const line = location.match(/InvoiceLine\[(?:@id=)?["']?(\d+)/i) ?? location.match(/line[^\d]*(\d+)/i);
  if (line?.[1]) {
    const index = Math.max(0, Number(line[1]) - 1);
    if (/quantity|BT-129/i.test(haystack)) return `lines.${index}.quantity`;
    if (/price|BT-146/i.test(haystack)) return `lines.${index}.netUnitPrice`;
    return `lines.${index}.name`;
  }
  return FIELD_HINTS.find(([pattern]) => pattern.test(haystack))?.[1] ?? "document";
}

export function germanFieldLabel(path: string): string {
  const lineMatch = path.match(/^lines\.(\d+)\.(.+)$/);
  if (lineMatch?.[1]) {
    const field = lineMatch[2] === "name"
      ? "Beschreibung"
      : lineMatch[2] === "quantity"
        ? "Menge"
        : lineMatch[2] === "netUnitPrice"
          ? "Einzelpreis"
          : lineMatch[2] === "tax.rate"
            ? "Steuersatz"
            : lineMatch[2];
    return `Position ${Number(lineMatch[1]) + 1}: ${field}`;
  }
  return {
    invoiceNumber: "Rechnungsnummer",
    buyerReference: "Bestellnummer oder Leitweg-ID",
    issueDate: "Rechnungsdatum",
    currency: "Währung",
    "seller.name": "Absender: Name",
    "seller.address.line1": "Absender: Straße und Hausnummer",
    "seller.address.city": "Absender: Ort",
    "seller.address.postalCode": "Absender: Postleitzahl",
    "seller.address.countryCode": "Absender: Land",
    "seller.vatId": "Absender: Umsatzsteuer-ID",
    "seller.contact.name": "Absender: Ansprechpartner",
    "seller.contact.phone": "Absender: Telefon",
    "seller.contact.email": "Absender: E-Mail",
    "buyer.name": "Empfänger: Name",
    "buyer.address.line1": "Empfänger: Straße und Hausnummer",
    "buyer.address.city": "Empfänger: Ort",
    "buyer.address.postalCode": "Empfänger: Postleitzahl",
    "buyer.address.countryCode": "Empfänger: Land",
    lines: "Leistungen und Artikel",
    "totals.taxTotal": "Umsatzsteuer",
    "totals.taxInclusive": "Rechnungsbetrag",
    "totals.payable": "Zahlbetrag",
    document: "Rechnung",
  }[path] ?? path;
}

export function evaluateVeraPdfOutcome(outcome: ProcessOutcome, ruleVersion: string, engineVersion = ""): Omit<OfficialValidationReport, "xmlSha256" | "pdfSha256" | "reportSha256"> {
  if (outcome.cancelled) {
    return { schemaVersion: 1, status: "cancelled", valid: false, engine: "verapdf", engineVersion, ruleVersion, issues: [{ engine: "verapdf", severity: "error", code: "CANCELLED", path: "document", message: "Die PDF/A-Prüfung wurde abgebrochen." }] };
  }
  if (outcome.timedOut) {
    return { schemaVersion: 1, status: "timeout", valid: false, engine: "verapdf", engineVersion, ruleVersion, issues: [{ engine: "verapdf", severity: "error", code: "TIMEOUT", path: "document", message: "Die PDF/A-Prüfung hat zu lange gedauert." }] };
  }
  const report = outcome.reportXml?.trim() || outcome.output;
  if (!/<(?:[\w.-]+:)?report\b/i.test(report) && !/<(?:[\w.-]+:)?validationReport\b/i.test(report)) {
    return { schemaVersion: 1, status: outcome.output.trim() ? "unreadable-report" : "missing-report", valid: false, engine: "verapdf", engineVersion, ruleVersion, issues: [{ engine: "verapdf", severity: "error", code: "VERAPDF-REPORT", path: "document", message: "Der maschinenlesbare PDF/A-Bericht fehlt. Die Datei gilt nicht als PDF/A-geprüft." }] };
  }
  const validationTag = report.match(/<(?:[\w.-]+:)?validationReport\b[^>]*>/i)?.[0] ?? "";
  const compliant = attribute(validationTag, "isCompliant")?.toLowerCase();
  const flavour = attribute(validationTag, "flavour") ?? "";
  const failedParse = /failedToParse\s*=\s*"([1-9]\d*)"/i.test(report) || /encrypted\s*=\s*"([1-9]\d*)"/i.test(report);
  const failedChecks = Number(attribute(report.match(/<(?:[\w.-]+:)?details\b[^>]*>/i)?.[0] ?? "", "failedChecks") ?? "0");
  const issues = collectTags(report, "rule").filter((block) => attribute(block, "status")?.toLowerCase() === "failed")
    .map((block) => issue("verapdf", attribute(block, "clause") ?? "PDFA", flavour, innerText(block, "description") || innerText(block, "message") || "Die PDF erfüllt PDF/A-3 nicht.", "error"));
  const passed = outcome.code === 0 && compliant === "true" && /3b/i.test(flavour) && !failedParse && failedChecks === 0;
  if (!passed) {
    if (!issues.length) issues.push({ engine: "verapdf", severity: "error", code: "PDFA", path: "document", message: "Die PDF/A-3-Prüfung ist fehlgeschlagen. Nicht jedes PDF kann umgewandelt werden." });
    return { schemaVersion: 1, status: compliant ? "failed" : "unreadable-report", valid: false, engine: "verapdf", engineVersion, ruleVersion, issues };
  }
  return { schemaVersion: 1, status: "passed", valid: true, engine: "verapdf", engineVersion, ruleVersion, issues: [] };
}

function attribute(source: string, name: string): string | undefined {
  return source.match(new RegExp(`\\b${name}\\s*=\\s*"([^"]*)"`, "i"))?.[1]
    ?? source.match(new RegExp(`\\b${name}\\s*=\\s*'([^']*)'`, "i"))?.[1];
}

function innerText(source: string, tag: string): string | undefined {
  return source.match(new RegExp(`<(?:[\\w.-]+:)?${tag}\\b[^>]*>([\\s\\S]*?)</(?:[\\w.-]+:)?${tag}>`, "i"))?.[1]
    ?.replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function collectTags(source: string, tag: string): string[] {
  const matches: string[] = [];
  const pattern = new RegExp(`<(?:[\\w.-]+:)?${tag}\\b([^>]*)>([\\s\\S]*?)</(?:[\\w.-]+:)?${tag}>|<(?:[\\w.-]+:)?${tag}\\b([^>]*)/>`, "gi");
  for (const match of source.matchAll(pattern)) {
    matches.push(`${match[1] ?? match[3] ?? ""} ${match[2] ?? ""}`);
  }
  return matches;
}

function issue(engine: OfficialEngine, code: string, location: string, message: string, severity: "error" | "warning" = "error"): OfficialValidationIssue {
  return {
    engine,
    severity,
    code: code || "XML",
    path: mapOfficialLocationToPath(location, code),
    message: message.trim() || "Die unabhängige Prüfung hat einen Fehler gemeldet.",
  };
}

export function evaluateKositOutcome(outcome: ProcessOutcome, ruleVersion: string, engineVersion = ""): Omit<OfficialValidationReport, "xmlSha256" | "pdfSha256" | "reportSha256"> {
  if (outcome.cancelled) {
    return { schemaVersion: 1, status: "cancelled", valid: false, engine: "kosit", engineVersion, ruleVersion, issues: [{ engine: "kosit", severity: "error", code: "CANCELLED", path: "document", message: "Die Prüfung wurde abgebrochen." }] };
  }
  if (outcome.timedOut) {
    return { schemaVersion: 1, status: "timeout", valid: false, engine: "kosit", engineVersion, ruleVersion, issues: [{ engine: "kosit", severity: "error", code: "TIMEOUT", path: "document", message: "Die unabhängige Prüfung hat zu lange gedauert." }] };
  }
  const report = outcome.reportXml?.trim();
  if (!report) {
    return { schemaVersion: 1, status: "missing-report", valid: false, engine: "kosit", engineVersion, ruleVersion, issues: [{ engine: "kosit", severity: "error", code: "KOSIT-REPORT", path: "document", message: "Der maschinenlesbare Prüfbericht fehlt. Die Datei gilt nicht als geprüft." }] };
  }
  const reportTag = report.match(/<(?:[\w.-]+:)?report\b[^>]*>/i)?.[0];
  const validAttr = reportTag ? attribute(reportTag, "valid")?.toLowerCase() : undefined;
  if (validAttr !== "true" && validAttr !== "false") {
    return { schemaVersion: 1, status: "unreadable-report", valid: false, engine: "kosit", engineVersion, ruleVersion, issues: [{ engine: "kosit", severity: "error", code: "KOSIT-REPORT", path: "document", message: "Der Prüfbericht konnte nicht gelesen werden." }] };
  }
  const rejected = collectTags(report, "reject").length > 0
    || /Acceptance\|Error[\s\S]*\|\s*REJECT/i.test(outcome.output);
  const accepted = collectTags(report, "accept").length > 0;
  const issues = [
    ...collectTags(report, "failed-assert").map((block) => issue("kosit", attribute(block, "id") ?? "", attribute(block, "location") ?? block, innerText(block, "text") ?? block)),
    ...collectTags(report, "xml-syntax-error").map((block) => issue("kosit", "XML", "", attribute(block, "message") ?? block)),
  ];
  const passed = validAttr === "true" && !rejected && (accepted || !report.includes("assessment")) && outcome.code === 0;
  if (validAttr === "true" && outcome.code === 0 && rejected) {
    return { schemaVersion: 1, status: "failed", valid: false, engine: "kosit", engineVersion, ruleVersion, issues: issues.length ? issues : [{ engine: "kosit", severity: "error", code: "KOSIT", path: "document", message: "KoSIT empfiehlt die Ablehnung des Dokuments." }] };
  }
  if (!passed) {
    if (!issues.length) issues.push({ engine: "kosit", severity: "error", code: "KOSIT", path: "document", message: "KoSIT meldet ein nicht akzeptables Dokument." });
    return { schemaVersion: 1, status: outcome.code < 0 && validAttr === undefined ? "unknown" : "failed", valid: false, engine: "kosit", engineVersion, ruleVersion, issues };
  }
  return { schemaVersion: 1, status: "passed", valid: true, engine: "kosit", engineVersion, ruleVersion, issues: [] };
}

export function evaluateMustangOutcome(outcome: ProcessOutcome, ruleVersion: string, engineVersion = ""): Omit<OfficialValidationReport, "xmlSha256" | "pdfSha256" | "reportSha256"> {
  if (outcome.cancelled) {
    return { schemaVersion: 1, status: "cancelled", valid: false, engine: "mustang", engineVersion, ruleVersion, issues: [{ engine: "mustang", severity: "error", code: "CANCELLED", path: "document", message: "Die Prüfung wurde abgebrochen." }] };
  }
  if (outcome.timedOut) {
    return { schemaVersion: 1, status: "timeout", valid: false, engine: "mustang", engineVersion, ruleVersion, issues: [{ engine: "mustang", severity: "error", code: "TIMEOUT", path: "document", message: "Die unabhängige Prüfung hat zu lange gedauert." }] };
  }
  const source = `${outcome.reportXml ?? ""}\n${outcome.output}`;
  if (!/<(?:[\w.-]+:)?summary\b/i.test(source) && !/<(?:[\w.-]+:)?validation\b/i.test(source)) {
    return { schemaVersion: 1, status: outcome.output.trim() ? "unreadable-report" : "missing-report", valid: false, engine: "mustang", engineVersion, ruleVersion, issues: [{ engine: "mustang", severity: "error", code: "MUSTANG-REPORT", path: "document", message: "Der maschinenlesbare Prüfbericht fehlt oder ist unlesbar. Die Datei gilt nicht als geprüft." }] };
  }
  const summaries = [...source.matchAll(/<(?:[\w.-]+:)?summary\b[^>]*status\s*=\s*"([^"]+)"/gi)].map((match) => match[1]?.toLowerCase());
  const finalStatus = summaries.at(-1);
  const issues = collectTags(source, "error").map((block) => issue("mustang", attribute(block, "criterion") ?? "MUSTANG", attribute(block, "location") ?? "", innerText(block, "error") || attribute(block, "message") || block));
  const passed = outcome.code === 0 && finalStatus === "valid";
  if (!passed) {
    if (!issues.length) issues.push({ engine: "mustang", severity: "error", code: "MUSTANG", path: "document", message: "Die Factur-X-/ZUGFeRD-Prüfung ist fehlgeschlagen." });
    return { schemaVersion: 1, status: finalStatus ? "failed" : "unknown", valid: false, engine: "mustang", engineVersion, ruleVersion, issues };
  }
  return { schemaVersion: 1, status: "passed", valid: true, engine: "mustang", engineVersion, ruleVersion, issues: [] };
}

export function officialResultFromOutcome(
  engine: OfficialEngine,
  outcome: ProcessOutcome,
  ruleVersion: string,
  hashes: { xmlSha256: string; pdfSha256: string; reportSha256?: string },
  engineVersion = "",
): OfficialValidationReport {
  const evaluated = engine === "kosit"
    ? evaluateKositOutcome(outcome, ruleVersion, engineVersion)
    : evaluateMustangOutcome(outcome, ruleVersion, engineVersion);
  return { ...evaluated, ...hashes };
}
