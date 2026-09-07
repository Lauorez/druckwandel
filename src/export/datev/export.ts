import { Decimal } from "decimal.js";
import { money } from "../../domain/money.js";
import { isIsoDate } from "../../domain/validate.js";
import { partyIdentity, readInvoiceSnapshot } from "../invoice-snapshot.js";
import { DATEV_COLUMNS, DATEV_TEXT_COLUMNS } from "./contract.js";
import { checkedText, encodeWindows1252, quote } from "./encoding.js";
import type { Booking, DatevBatch, DatevPreview, DatevProfile, DatevSource, InvoiceAssignment } from "./types.js";

const date = (value: string) => isIsoDate(value) && /^20\d\d-/.test(value);
const account = (value: string,length: number) => new RegExp(`^[1-9][0-9]{${length-1}}$`).test(value);
export function validateDatevProfile(profile: DatevProfile): string[] {
  const errors: string[] = [];
  try {
    if (profile.schemaVersion!==1 || !profile.confirmed) errors.push("Bitte lassen Sie die Angaben von Ihrer Steuerkanzlei bestätigen.");
    checkedText(profile.name,60,"Bezeichnung");
    if (!/^[1-9]\d{3,6}$/.test(profile.consultant) || Number(profile.consultant)<1001) errors.push("Beraternummer: 1001 bis 9999999 erforderlich.");
    if (!/^[1-9]\d{0,4}$/.test(profile.client)) errors.push("Mandantennummer: 1 bis 99999 erforderlich.");
    if (!date(profile.fiscalYearStart) || profile.fiscalYearStart.endsWith("02-29")) errors.push("Bitte einen gültigen Wirtschaftsjahresbeginn angeben (nicht 29. Februar).");
    if (!Number.isInteger(profile.accountLength) || profile.accountLength<4 || profile.accountLength>8) errors.push("Sachkontenlänge: 4 bis 8 Stellen erforderlich.");
    if (!["03","04"].includes(profile.chart)) errors.push("Bitte SKR03 oder SKR04 auswählen.");
    if (!profile.seller?.name?.trim() || !profile.seller.address?.line1?.trim() || !profile.seller.address?.postalCode?.trim() || !profile.seller.address?.city?.trim() || profile.seller.address?.countryCode!=="DE") errors.push("Bitte den eigenen Betrieb vollständig mit deutscher Anschrift angeben.");
    if (profile.accountingMethod!=="accrual") errors.push("Diese erste Exportversion unterstützt nur bestätigte Sollversteuerung, keine Istversteuerung.");
    if (!["invoice-date","service-date"].includes(profile.periodRule)) errors.push("Bitte mit der Kanzlei die Zuordnung zur Steuerperiode festlegen.");
    if (!["0","1"].includes(profile.locking)) errors.push("Bitte mit der Kanzlei festlegen, ob der Stapel beim Import festgeschrieben werden soll.");
    if (profile.collectiveDebtor && (!profile.collectiveDebtorConfirmed || !account(profile.collectiveDebtor,profile.accountLength+1))) errors.push("Das Sammelkundenkonto muss ausdrücklich bestätigt sein und eine Stelle länger als ein Sachkonto sein.");
    if (new Set(profile.debtors.map(d=>d.buyerIdentity)).size!==profile.debtors.length || profile.debtors.some(d=>!d.buyerIdentity || !account(d.account,profile.accountLength+1))) errors.push("Die Kundenzuordnungen sind unvollständig oder doppelt.");
    if (!profile.revenueAccounts.length || new Set(profile.revenueAccounts.map(a=>a.id)).size!==profile.revenueAccounts.length) errors.push("Bitte eindeutige Erlöskonten hinterlegen.");
    for (const a of profile.revenueAccounts) {
      if (!a.id || !["7","19"].includes(a.taxRate) || !account(a.account,profile.accountLength)) errors.push("Ein Erlöskonto oder Steuersatz ist ungültig.");
      if (a.mode==="automatic" ? a.taxKey!=="" : a.mode!=="tax-key" || a.taxKey!==(a.taxRate==="7"?"0002":"0003")) errors.push("Automatikkonten benötigen keinen Steuerschlüssel; sonst werden hier nur 0002 (7 %) und 0003 (19 %) unterstützt.");
      const sameAccount = profile.revenueAccounts.filter(b=>b.account===a.account);
      if (sameAccount.some(b=>b.mode!==a.mode || (a.mode==="automatic" && b.taxRate!==a.taxRate))) errors.push("Dasselbe Automatikkonto darf nicht widersprüchlich zugeordnet sein.");
    }
  } catch (e) { errors.push(e instanceof Error?e.message:"Die Kanzleiangaben sind unvollständig."); }
  return [...new Set(errors)];
}
export function previewDatev(profile: DatevProfile, sources: DatevSource[], assignments: InvoiceAssignment[] = []): DatevPreview {
  const issues = validateDatevProfile(profile);
  const result: DatevPreview = {batches:[],issues,invoiceCount:0,gross:"0.00"};
  if (issues.length) return result;
  if (!sources.length) { issues.push("Bitte mindestens eine Rechnung auswählen."); return result; }
  const seen = new Set<string>(), duplicates = new Set<string>(), rows: Booking[] = [];
  const end = `${Number(profile.fiscalYearStart.slice(0,4))+1}${profile.fiscalYearStart.slice(4)}`;
  for (const source of sources) {
    try {
      const invoice = readInvoiceSnapshot(source.snapshot,source.format,source.xml);
      const identity = `${source.documentId}:${source.contentHash}`;
      if (seen.has(identity)) continue; // XML and PDF of the same content are one invoice.
      seen.add(identity);
      const duplicateKey = `${partyIdentity(invoice.seller)}|${invoice.invoiceNumber}`;
      if (duplicates.has(duplicateKey)) throw new Error("Mögliche doppelte oder abweichende Ausgabe derselben Rechnungsnummer. Bitte nur die richtige Rechnung auswählen.");
      duplicates.add(duplicateKey);
      if (partyIdentity(invoice.seller)!==partyIdentity(profile.seller)) throw new Error("Der Absender stimmt nicht mit dem bestätigten eigenen Betrieb überein.");
      if (invoice.currency!=="EUR" || invoice.invoiceType!=="380" || invoice.buyer.address.countryCode!=="DE") throw new Error("Unterstützt werden normale inländische Ausgangsrechnungen in EUR.");
      if (!date(invoice.issueDate) || invoice.issueDate<profile.fiscalYearStart || invoice.issueDate>=end) throw new Error("Das Rechnungsdatum liegt außerhalb des eingerichteten Wirtschaftsjahres. Bitte das Kanzleiprofil entsprechend einstellen.");
      if (!/^[A-Za-z0-9_$&%*+\-/]{1,36}$/.test(invoice.invoiceNumber)) throw new Error("Die Rechnungsnummer ist für DATEV nicht zulässig (maximal 36 Zeichen; keine Leerzeichen, Umlaute oder Punkte). Sie wird nicht automatisch geändert.");
      if (invoice.dueDate && !date(invoice.dueDate)) throw new Error("Das Fälligkeitsdatum ist für DATEV ungültig.");
      const serviceDate = invoice.serviceDate??"";
      if (serviceDate && (!date(serviceDate) || serviceDate<profile.fiscalYearStart || serviceDate>=end)) throw new Error("Das Leistungsdatum liegt außerhalb des eingerichteten Wirtschaftsjahres.");
      if (profile.periodRule==="service-date" && !serviceDate) throw new Error("Für die vereinbarte Steuerperiode fehlt das Leistungsdatum.");
      const debtor = profile.debtors.find(d=>d.buyerIdentity===partyIdentity(invoice.buyer))?.account || (profile.collectiveDebtorConfirmed?profile.collectiveDebtor:"");
      if (!debtor) throw new Error("Für diesen Kunden fehlt die von der Kanzlei bestätigte Kontonummer.");
      const assignment = assignments.find(a=>a.archiveId===source.archiveId);
      const text = checkedText(assignment?.bookingText??invoice.buyer.name,60,"Buchungstext");
      const groups = new Map<string,{net:Decimal;accountId:string}>();
      if (new Set(invoice.calculatedLines.map(l=>l.id)).size!==invoice.calculatedLines.length) throw new Error("Doppelte Positionsnummern verhindern eine eindeutige Kontierung.");
      for (const line of invoice.calculatedLines) {
        const rate = new Decimal(line.tax.rate).toFixed();
        if (line.tax.categoryCode!=="S" || !["7","19"].includes(rate)) throw new Error("Steuerbefreiung, Reverse Charge und andere Steuersätze sind noch nicht für DATEV freigegeben.");
        const available = profile.revenueAccounts.filter(a=>a.taxRate===rate);
        const chosen = assignment?.lineAccounts?.[line.id];
        const a = chosen?available.find(a=>a.id===chosen):available.length===1?available[0]:undefined;
        if (!a) throw new Error(`Bitte das Erlöskonto für Position ${line.id} eindeutig zuordnen.`);
        const net = new Decimal(line.netAmount);
        if (net.lt(0)) throw new Error("Negative Rechnungspositionen werden noch nicht unterstützt.");
        const group = groups.get(a.id)??{net:new Decimal(0),accountId:a.id};
        group.net=group.net.add(net); groups.set(a.id,group);
      }
      const invoiceRows: Booking[] = [];
      for (const group of groups.values()) {
        if (group.net.isZero()) continue;
        const a=profile.revenueAccounts.find(a=>a.id===group.accountId)!;
        const tax=money(group.net.mul(a.taxRate).div(100));
        const gross=money(group.net.add(tax));
        if (new Decimal(gross).gt("9999999999.99") || money(new Decimal(gross).mul(a.taxRate).div(new Decimal(100).add(a.taxRate)))!==tax) throw new Error("Dieser Betrag lässt sich mit DATEV-Steuerautomatik nicht centgenau abbilden.");
        invoiceRows.push({archiveId:source.archiveId,documentId:source.documentId,contentHash:source.contentHash,duplicateKey,invoiceNumber:invoice.invoiceNumber,issueDate:invoice.issueDate,dueDate:invoice.dueDate??"",serviceDate,taxPeriodDate:profile.periodRule==="service-date"?serviceDate:invoice.issueDate,debtor,revenueAccount:a.account,taxKey:a.taxKey,taxRate:a.taxRate,net:money(group.net),tax,gross,text});
      }
      for (const tax of invoice.taxes) {
        const sum = invoiceRows.filter(r=>new Decimal(r.taxRate).eq(tax.rate)).reduce((s,r)=>s.add(r.tax),new Decimal(0));
        if (!sum.eq(tax.taxAmount)) throw new Error("Die Aufteilung auf Erlöskonten verursacht eine Steuer-Rundungsdifferenz. Bitte die Kontierung mit der Kanzlei prüfen.");
      }
      if (!invoiceRows.length || !invoiceRows.reduce((s,r)=>s.add(r.gross),new Decimal(0)).eq(invoice.totals.payable)) throw new Error("Die Buchungen stimmen nicht mit dem archivierten Rechnungsbetrag überein.");
      rows.push(...invoiceRows); result.invoiceCount++;
    } catch(e) { issues.push(`${source.archiveId}: ${e instanceof Error?e.message:String(e)}`); }
  }
  // No silent partial export. Keep every selected invoice visible in the UI.
  if (issues.length) return {...result,batches:[],invoiceCount:0};
  const years = [...new Set(rows.map(r=>r.issueDate.slice(0,4)))].sort();
  result.batches = years.map(year=>{
    const bookings=rows.filter(r=>r.issueDate.startsWith(year)).sort((a,b)=>a.issueDate.localeCompare(b.issueDate)||a.invoiceNumber.localeCompare(b.invoiceNumber)||a.revenueAccount.localeCompare(b.revenueAccount));
    return {fiscalYearStart:profile.fiscalYearStart,dateFrom:bookings[0]!.issueDate,dateTo:bookings.at(-1)!.issueDate,bookings,gross:money(bookings.reduce((s,b)=>s.add(b.gross),new Decimal(0)))};
  });
  result.gross=money(rows.reduce((s,r)=>s.add(r.gross),new Decimal(0)));
  return result;
}
export function serializeDatev(profile: DatevProfile,batch: DatevBatch,createdAt: Date): Uint8Array {
  const problems=validateDatevProfile(profile);
  if (problems.length || !batch.bookings.length || !Number.isFinite(createdAt.getTime())) throw new Error(problems.join(" ")||"Leerer oder ungültiger Export.");
  const ymd=(s:string)=>s.replace(/-/g,"");
  const dm=(s:string)=>s.slice(8,10)+s.slice(5,7);
  const dmy=(s:string)=>s?dm(s)+s.slice(0,4):"";
  // UTC timestamp is documented in the manifest; DATEV has no timezone field.
  const stamp=createdAt.toISOString().replace(/[-:TZ.]/g,"");
  const header=[quote("EXTF"),"700","21",quote("Buchungsstapel"),"13",stamp,"",quote("ER"),quote("ERechnungsAssistent"),quote(""),profile.consultant,profile.client,ymd(batch.fiscalYearStart),String(profile.accountLength),ymd(batch.dateFrom),ymd(batch.dateTo),quote(`Ausgang ${batch.dateFrom}`),quote(""),"1","0",profile.locking,quote("EUR"),"",quote(""),"","",quote(profile.chart),"","",quote(""),quote("ERechnung")];
  const lines=[header.join(";"),DATEV_COLUMNS.map(quote).join(";")];
  for (const b of batch.bookings) {
    const cells=Array<string>(125).fill("");
    cells[0]=b.gross.replace(".",","); cells[1]="S"; cells[2]="EUR";
    cells[6]=b.debtor; cells[7]=b.revenueAccount; cells[8]=b.taxKey;
    cells[9]=dm(b.issueDate); cells[10]=b.invoiceNumber; cells[13]=b.text;
    cells[113]=profile.locking; cells[114]=dmy(b.serviceDate); cells[115]=b.serviceDate?dmy(b.taxPeriodDate):""; cells[116]=dmy(b.dueDate);
    lines.push(cells.map((c,i)=>DATEV_TEXT_COLUMNS.has(i)?quote(c):c).join(";"));
  }
  return encodeWindows1252(lines.join("\r\n")+"\r\n");
}
