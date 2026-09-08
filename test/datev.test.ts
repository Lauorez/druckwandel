import { describe, expect, it } from "vitest";
import { createHash } from "node:crypto";
import { calculateInvoice } from "../src/domain/calculate.js";
import type { InvoiceInput } from "../src/domain/types.js";
import { generateUbl } from "../src/engine/ubl.js";
import { generateCii } from "../src/engine/cii.js";
import { invoiceSnapshot, readInvoiceSnapshot } from "../src/export/invoice-snapshot.js";
import { documentPackage } from "../src/export/datev/documents.js";
import { previewDatev, serializeDatev, validateDatevProfile } from "../src/export/datev/export.js";
import { encodeWindows1252 } from "../src/export/datev/encoding.js";
import type { DatevProfile, DatevSource } from "../src/export/datev/types.js";
import { standardInvoice } from "./fixtures/invoice.js";

export function profile(): DatevProfile {
  return {schemaVersion:1,name:"Testkanzlei",consultant:"1001",client:"12345",fiscalYearStart:"2026-01-01",accountLength:4,chart:"03",seller:structuredClone(standardInvoice.seller),confirmed:true,accountingMethod:"accrual",periodRule:"invoice-date",locking:"0",collectiveDebtor:"10000",collectiveDebtorConfirmed:true,debtors:[],revenueAccounts:[{id:"19",label:"Erlöse 19 %",taxRate:"19",account:"8400",mode:"automatic",taxKey:""},{id:"7",label:"Erlöse 7 %",taxRate:"7",account:"8300",mode:"automatic",taxKey:""}]};
}
export function source(input:InvoiceInput=standardInvoice, format:DatevSource["format"]="xrechnung", documentId="document-1"):DatevSource {
  const invoice=calculateInvoice(input), snapshot=invoiceSnapshot(invoice);
  return {archiveId:`${documentId}-${format}`,documentId,contentHash:createHash("sha256").update(snapshot).digest("hex"),originalHash:"original",snapshot,format,xml:format==="xrechnung"?generateUbl(invoice):generateCii(invoice)};
}
// Independent character-by-character reader, not the writer's quoting logic.
function csv(bytes:Uint8Array):string[][] {
  const text=Buffer.from(bytes).toString("latin1").replace(/\u0080/g,"€"), rows:string[][]=[];
  let row:string[]=[],cell="",quoted=false;
  for(let i=0;i<text.length;i++) { const c=text[i];
    if(c==='"') {if(quoted&&text[i+1]==='"'){cell+='"';i++;}else quoted=!quoted;}
    else if(c===';'&&!quoted){row.push(cell);cell="";}
    else if(c==='\r'&&!quoted){expect(text[++i]).toBe('\n');row.push(cell);rows.push(row);row=[];cell="";}
    else cell+=c;
  }
  expect(quoted).toBe(false);expect(cell).toBe("");return rows;
}
describe("DATEV immutable source and EXTF adapter",()=>{
  it("round-trips both archived formats and rejects changed snapshots or XML",()=>{
    for(const format of ["xrechnung","zugferd"] as const){const s=source(standardInvoice,format);expect(readInvoiceSnapshot(s.snapshot,s.format,s.xml).totals.payable).toBe("282.08");expect(()=>readInvoiceSnapshot(s.snapshot,s.format,s.xml+" ")).toThrow();expect(()=>readInvoiceSnapshot(s.snapshot.replace('282.08','282.09'))).toThrow();}
  });
  it("emits independently readable 31/125-column CP1252 rows with exact monetary and date fields",()=>{
    const p=profile(),s=source(),v=previewDatev(p,[s],[{archiveId:s.archiveId,bookingText:'Müller; "Öl" €'}]);
    expect(v.issues).toEqual([]);expect(v.gross).toBe("282.08");
    const bytes=serializeDatev(p,v.batches[0]!,new Date("2026-09-07T10:11:12.345Z"));
    expect([...bytes.slice(0,3)]).toEqual([34,69,88]);expect(bytes).toContain(128);
    const [header,columns,row]=csv(bytes);
    expect(header).toHaveLength(31);expect(columns).toHaveLength(125);expect(row).toHaveLength(125);
    expect(header!.slice(0,6)).toEqual(["EXTF","700","21","Buchungsstapel","13","20260907101112345"]);
    expect(header!.slice(10,16)).toEqual(["1001","12345","20260101","4","20260820","20260820"]);
    expect(row!.slice(0,3)).toEqual(["282,08","S","EUR"]);
    expect(row!.slice(6,11)).toEqual(["10000","8400","","2008","RE-2026-0001"]);
    expect(row![13]).toBe('Müller; "Öl" €');expect(row![19]).toBe(`BEDI "${v.batches[0]!.bookings[0]!.belegGuid}"`);expect(row!.slice(113,117)).toEqual(["0","19082026","20082026","03092026"]);
  });
  it("uses one Beleg GUID per invoice and a DATEV document.xml package",()=>{
    const first=source(), pdf=source(standardInvoice,"zugferd");
    const shared=previewDatev(profile(),[first,pdf]);
    expect(shared.issues).toEqual([]);expect(shared.invoiceCount).toBe(1);
    const guid=shared.batches[0]!.bookings[0]!.belegGuid;
    expect(guid).toMatch(/^[0-9A-F]{8}-[0-9A-F]{4}-5[0-9A-F]{3}-A[0-9A-F]{3}-[0-9A-F]{12}$/);
    const p=profile();p.revenueAccounts.push({...p.revenueAccounts[0]!,id:"other",account:"8410"});
    const s=source({...standardInvoice,lines:[...standardInvoice.lines,{...standardInvoice.lines[0]!,id:"3",quantity:"1",netUnitPrice:"10",tax:{categoryCode:"S",rate:"7"}}]});
    const v=previewDatev(p,[s],[{archiveId:s.archiveId,lineAccounts:{"1":"19","2":"other"}}]);
    expect(v.issues).toEqual([]);
    expect(new Set(v.batches[0]!.bookings.map(b=>b.belegGuid)).size).toBe(1);
    const created=new Date("2026-09-07T10:11:12.345Z");
    const pack=documentPackage(v.batches[0]!.bookings,created);
    expect(pack.files).toHaveLength(1);
    expect(pack.files[0]).toEqual({archiveId:s.archiveId,guid:v.batches[0]!.bookings[0]!.belegGuid,pdfName:`${v.batches[0]!.bookings[0]!.belegGuid}.pdf`,xmlName:`${v.batches[0]!.bookings[0]!.belegGuid}.xml`});
    expect(pack.xml).toContain('xmlns="http://xml.datev.de/bedi/tps/document/v06.0"');
    expect(pack.xml).toContain(`guid="${v.batches[0]!.bookings[0]!.belegGuid}"`);
    expect(pack.xml).toContain('type="2"');
    expect(pack.xml).toContain('processID="1"');
    expect(pack.xml).toContain("<date>2026-09-07T10:11:12</date>");
  });
  it("deduplicates XML and PDF, but blocks separate documents with the same invoice number",()=>{
    expect(previewDatev(profile(),[source(),source(standardInvoice,"zugferd")]).invoiceCount).toBe(1);
    expect(previewDatev(profile(),[source(),source(standardInvoice,"zugferd","another")]).issues.join()).toContain("doppelte");
  });
  it("partitions 7/19 percent and separate revenue accounts without changing cents",()=>{
    const p=profile();p.revenueAccounts.push({...p.revenueAccounts[0]!,id:"other",account:"8410"});
    const s=source({...standardInvoice,lines:[...standardInvoice.lines,{...standardInvoice.lines[0]!,id:"3",quantity:"1",netUnitPrice:"10",tax:{categoryCode:"S",rate:"7"}}]});
    expect(previewDatev(p,[s]).issues.join()).toContain("eindeutig");
    const v=previewDatev(p,[s],[{archiveId:s.archiveId,lineAccounts:{"1":"19","2":"other"}}]);
    expect(v.issues).toEqual([]);expect(v.batches[0]!.bookings.map(b=>b.gross).sort()).toEqual(["10.70","238.00","44.08"]);expect(v.gross).toBe("292.78");
  });
  it("blocks split rounding differences rather than inventing balancing bookings",()=>{
    const p=profile();p.revenueAccounts.push({...p.revenueAccounts[0]!,id:"other",account:"8410"});
    const s=source({...standardInvoice,lines:standardInvoice.lines.map(l=>({...l,quantity:"1",netUnitPrice:"0.03"}))});
    const v=previewDatev(p,[s],[{archiveId:s.archiveId,lineAccounts:{"1":"19","2":"other"}}]);
    expect(v.issues.join()).toContain("Rundungsdifferenz");expect(v.batches).toEqual([]);
  });
  it.each(["RE.1","RE 1","RE-Ä","A".repeat(37)])("does not silently sanitize invalid invoice number %s",invoiceNumber=>{
    expect(previewDatev(profile(),[source({...standardInvoice,invoiceNumber})]).issues.join()).toContain("Rechnungsnummer");
  });
  it.each(["A".repeat(61),"Text\nZeile","Emoji 😀"])("blocks unsuitable booking text %s",bookingText=>{
    const s=source();expect(previewDatev(profile(),[s],[{archiveId:s.archiveId,bookingText}]).issues.length).toBeGreaterThan(0);
  });
  it.each([{confirmed:false},{accountingMethod:"cash"},{locking:""},{periodRule:""},{consultant:"1000"},{client:"0"},{collectiveDebtorConfirmed:false},{accountLength:3}])("requires explicit valid profile %j",patch=>{
    expect(validateDatevProfile({...profile(),...patch} as DatevProfile).length).toBeGreaterThan(0);
  });
  it("blocks wrong business, unsupported tax, and the entire selection on any invalid source",()=>{
    const p=profile();p.seller.name="Anderer Betrieb";expect(previewDatev(p,[source()]).issues.join()).toContain("Absender");
    const zero=source({...standardInvoice,invoiceNumber:"ZERO",lines:[{...standardInvoice.lines[0]!,tax:{categoryCode:"Z",rate:"0"}}]});
    const v=previewDatev(profile(),[source(),zero]);expect(v.batches).toEqual([]);expect(v.invoiceCount).toBe(0);
  });
  it("splits an off-calendar fiscal year into two unambiguous date batches",()=>{
    const p=profile();p.fiscalYearStart="2026-07-01";
    const next=source({...standardInvoice,invoiceNumber:"RE-2027-1",issueDate:"2027-01-02",serviceDate:"2027-01-01",dueDate:"2027-02-01"},"xrechnung","next");
    const v=previewDatev(p,[source(),next]);expect(v.issues).toEqual([]);expect(v.batches.map(b=>b.dateFrom)).toEqual(["2026-08-20","2027-01-02"]);
    expect(previewDatev(profile(),[next]).issues.join()).toContain("Wirtschaftsjahres");
  });
  it("requires service dates when chosen, and separates explicit tax keys from automatic accounts",()=>{
    const p=profile();p.periodRule="service-date";const {serviceDate:_ignored,...withoutServiceDate}=standardInvoice;expect(previewDatev(p,[source(withoutServiceDate)]).issues.join()).toContain("Leistungsdatum");
    p.revenueAccounts[0]!.taxKey="0003";expect(validateDatevProfile(p).length).toBeGreaterThan(0);
    p.revenueAccounts[0]!.mode="tax-key";expect(validateDatevProfile(p)).toEqual([]);
    const v=previewDatev(p,[source()]);expect(csv(serializeDatev(p,v.batches[0]!,new Date()))[2]![8]).toBe("0003");
  });
  it("rejects unrepresentable CP1252 characters",()=>{expect(()=>encodeWindows1252("😀")).toThrow();});
});
