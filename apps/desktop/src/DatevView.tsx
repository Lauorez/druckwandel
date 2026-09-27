import { useEffect, useMemo, useRef, useState } from "react";
import { formatGermanDecimal } from "../../../src/domain/localized-decimal.js";
import { canonicalJson, partyIdentity, readInvoiceSnapshot } from "../../../src/export/invoice-snapshot.js";
import { documentPackage } from "../../../src/export/datev/documents.js";
import { previewDatev, serializeDatev, validateDatevProfile } from "../../../src/export/datev/export.js";
import type { DatevPreview, DatevProfile, DatevSource, InvoiceAssignment } from "../../../src/export/datev/types.js";
import { listArchiveEntries, type ArchiveEntrySummary } from "./archiveStore.js";
import { datevStore, emptyDatevProfile, parseStoredDatevProfile, type DatevExportPage, type DatevExportRequest } from "./datevStore.js";
import { listenSettingsChanged } from "./settingsWindow.js";
import { subscription } from "./subscription.js";
import { toBase64 } from "./workspaceStore.js";

const describe=(e:unknown)=>e instanceof Error?e.message:String(e);
const amount=(s:string,currency="EUR")=>`${formatGermanDecimal(s)} ${currency}`;
export function DatevView({onBack,onBusyChange,onSettings}:{onBack:()=>void;onBusyChange:(busy:boolean)=>void;onSettings?:()=>void}) {
  const [profile,setProfile]=useState(emptyDatevProfile);
  const profileJson=useMemo(()=>canonicalJson(profile),[profile]);
  const [savedProfile,setSavedProfile]=useState(profileJson);
  const [ready,setReady]=useState(false);
  const [loadFailed,setLoadFailed]=useState(false);
  const [saveFailed,setSaveFailed]=useState(false);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [feedback,setFeedback]=useState("");
  const [entries,setEntries]=useState<ArchiveEntrySummary[]>([]);
  const [total,setTotal]=useState(0);
  const [offset,setOffset]=useState(0);
  const [dateFrom,setDateFrom]=useState("");
  const [dateTo,setDateTo]=useState("");
  const [includeExported,setIncludeExported]=useState(false);
  const [exported,setExported]=useState<string[]>([]);
  const [selected,setSelected]=useState<Map<string,ArchiveEntrySummary>>(new Map());
  const [detail,setDetail]=useState<DatevSource>();
  const [assignments,setAssignments]=useState<InvoiceAssignment[]>([]);
  const [preview,setPreview]=useState<DatevPreview>();
  const [repeated,setRepeated]=useState<string[]>([]);
  const [repeatReason,setRepeatReason]=useState("");
  const [repeatConfirmed,setRepeatConfirmed]=useState(false);
  const [history,setHistory]=useState<DatevExportPage>({entries:[],total:0});
  const [historyOffset,setHistoryOffset]=useState(0);
  const [refresh,setRefresh]=useState(0);
  const [loading,setLoading]=useState(false);
  const writer=useRef(Promise.resolve());
  const request=useRef<DatevExportRequest | undefined>(undefined);
  const alive=useRef(true);
  const dirty=profileJson!==savedProfile;
  const profileDirty=useRef(dirty);profileDirty.current=dirty;
  const editRevision=useRef(0);
  const invoice=useMemo(()=>detail?readInvoiceSnapshot(detail.snapshot,detail.format,detail.xml):undefined,[detail]);
  const visible=entries.filter(e=>includeExported || !e.documentId || !exported.includes(e.documentId));
  const profileProblems=useMemo(()=>validateDatevProfile(profile),[profile]);
  const invalidate=()=>{setPreview(undefined);setRepeated([]);setRepeatConfirmed(false);setRepeatReason("");request.current=undefined;};
  function updateProfile(next:DatevProfile) {editRevision.current+=1;profileDirty.current=true;setProfile(next);invalidate();setFeedback("");}
  function saveProfile(contents=profileJson) {
    setSaveFailed(false);
    const next=writer.current.catch(()=>undefined).then(()=>datevStore.saveProfile(contents));
    writer.current=next;
    void next.then(()=>{if(alive.current){setSavedProfile(contents);setSaveFailed(false);setError("");}}).catch(e=>{if(alive.current){setSaveFailed(true);setError(`Kanzleiangaben nicht gespeichert: ${describe(e)}`);}});
    return next;
  }
  useEffect(()=>{alive.current=true;let cancelled=false;
    void datevStore.profile().then(raw=>{
      if(cancelled)return;
      const p=parseStoredDatevProfile(raw);setProfile(p);setSavedProfile(canonicalJson(p));
      setReady(true);
    }).catch(e=>{if(!cancelled){setError(describe(e));setLoadFailed(true);}});
    return()=>{cancelled=true;alive.current=false;};
  },[]);
  useEffect(()=>{
    return subscription(listenSettingsChanged(scope=>{if(scope!=="datev"||!alive.current||profileDirty.current)return;
      const revision=editRevision.current;
      void datevStore.profile().then(raw=>{if(!alive.current||profileDirty.current||revision!==editRevision.current)return;const p=parseStoredDatevProfile(raw);setProfile(p);setSavedProfile(canonicalJson(p));setReady(true);setLoadFailed(false);invalidate();}).catch(e=>{if(alive.current)setError(describe(e));});
    }));
  },[]);
  useEffect(()=>{onBusyChange(busy||dirty||(!ready&&!loadFailed));},[busy,dirty,ready,loadFailed,onBusyChange]);
  useEffect(()=>()=>onBusyChange(false),[onBusyChange]);
  useEffect(()=>{if(!ready||!dirty)return;const timer=setTimeout(()=>{void saveProfile(profileJson);},450);return()=>clearTimeout(timer);},[ready,profileJson]);
  useEffect(()=>{let cancelled=false;setLoading(true);
    void listArchiveEntries({dateFrom,dateTo,offset,limit:50}).then(async page=>{
      const sent=await datevStore.status([...new Set(page.entries.flatMap(e=>e.documentId?[e.documentId]:[]))]);
      if(!cancelled){setEntries(page.entries);setTotal(page.total);setExported(sent);}
    }).catch(e=>{if(!cancelled)setError(describe(e));}).finally(()=>{if(!cancelled)setLoading(false);});
    return()=>{cancelled=true;};
  },[dateFrom,dateTo,offset,refresh]);
  useEffect(()=>{let cancelled=false;void datevStore.history(historyOffset).then(h=>{if(!cancelled)setHistory(h);}).catch(e=>{if(!cancelled)setError(describe(e));});return()=>{cancelled=true;};},[refresh,historyOffset]);
  async function action(task:()=>Promise<void>) {if(busy)return;setBusy(true);setError("");setFeedback("");try{await task();}catch(e){setError(describe(e));}finally{if(alive.current){setBusy(false);setRefresh(v=>v+1);}}}
  function toggle(entry:ArchiveEntrySummary,checked:boolean) {invalidate();setSelected(current=>{const next=new Map(current);if(checked&&next.size<100)next.set(entry.id,entry);else next.delete(entry.id);return next;});}
  async function loadDetail(id:string) {await action(async()=>{const next=await datevStore.source(id);readInvoiceSnapshot(next.snapshot,next.format,next.xml);setDetail(next);});}
  function assignment(patch:Partial<InvoiceAssignment>) {if(!detail)return;invalidate();setAssignments(all=>[...all.filter(a=>a.archiveId!==detail.archiveId),{...all.find(a=>a.archiveId===detail.archiveId),archiveId:detail.archiveId,...patch}]);}
  async function buildPreview() {await action(async()=>{
    if(dirty)await saveProfile();
    const sources:DatevSource[]=[];
    // Bound memory and make failures explicit for the complete selection.
    for(const id of selected.keys())sources.push(await datevStore.source(id));
    const result=previewDatev(profile,sources,assignments);
    const ids=[...new Set(result.batches.flatMap(b=>b.bookings.map(r=>r.archiveId)))];
    setRepeated(ids.length?await datevStore.duplicates(ids):[]);setPreview(result);request.current=undefined;
  });}
  async function createExport() {await action(async()=>{
    if(!preview || preview.issues.length || !preview.batches.length || dirty)throw new Error("Bitte zuerst eine aktuelle Vorschau erstellen.");
    if(repeated.length&&(!repeatConfirmed||repeatReason.trim().length<10))throw new Error("Bitte den erneuten Export bestätigen und begründen.");
    const createdAt=new Date();
    request.current??={
      id:crypto.randomUUID(),
      profile:profileJson,
      repeatReason:repeated.length?repeatReason:"",
      files:preview.batches.map(batch=>({
        contentsBase64:toBase64(serializeDatev(profile,batch,createdAt)),
        dateFrom:batch.dateFrom,
        dateTo:batch.dateTo,
        gross:batch.gross,
        bookingCount:batch.bookings.length,
        archiveIds:[...new Set(batch.bookings.map(b=>b.archiveId))],
      })),
      documentPackage:documentPackage(preview.batches.flatMap(batch=>batch.bookings),createdAt),
    };
    const result=await datevStore.create(request.current);
    setFeedback("Paket erstellt. Zuerst Belege.zip über DATEV Belegtransfer importieren und nicht entpacken, danach die EXTF-Dateien. Es wurde nichts an DATEV übertragen; der dortige Importstatus ist unbekannt.");
    setPreview(undefined);setSelected(new Map());request.current=undefined;
    setHistoryOffset(0);setRefresh(v=>v+1);
    if(result.state!=="complete")throw new Error("Der Export wurde vorbereitet, aber noch nicht abgeschlossen. Bitte in der Exporthistorie fortsetzen.");
  });}
  const friendly=(message:string)=>{for(const [id,e] of selected)message=message.replaceAll(id,`Rechnung ${e.invoiceNumber}`);return message;};
  const detailAssignment=assignments.find(a=>a.archiveId===detail?.archiveId);
  return <section className="datev-view">
    <div className="archive-heading"><div><h2>Für die Steuerkanzlei exportieren</h2><p>Buchungsstapel und Belegpaket aus fertig gespeicherten Ausgangsrechnungen</p></div><button className="secondary" disabled={busy||dirty} onClick={onBack}>Zurück zum Archiv</button></div>
    {error&&<div className="error-banner" role="alert">{friendly(error)}</div>}
    {feedback&&<div className="datev-feedback" role="status">{feedback}</div>}
    <div className="datev-scroll">
      <p className="datev-notice">Testversion: Vor der ersten echten Übergabe bitte einen Testimport durch Ihre Steuerkanzlei prüfen lassen. Der Export bestätigt keine steuerliche Richtigkeit und keine E-Rechnungs-Konformität.</p>
      <div className="datev-settings">
        <div>
          <strong>Kanzleiangaben {saveFailed?"– nicht gespeichert":loadFailed?"– konnten nicht geladen werden":dirty?"– wird gespeichert …":ready?"– gespeichert":"– werden geladen …"}</strong>
          <p>Betrieb, Konten und Buchungseinstellungen werden in den Einstellungen gepflegt.</p>
          {saveFailed && <button type="button" className="secondary" onClick={()=>{void saveProfile().catch(()=>undefined);}}>Erneut speichern</button>}
        </div>
        {onSettings && <button type="button" className="secondary" onClick={onSettings}>Einstellungen öffnen</button>}
      </div>
      {profileProblems.length>0&&<div className="datev-notice"><strong>Vor dem Export noch zu klären:</strong><ul>{profileProblems.map(p=><li key={p}>{p}</li>)}</ul></div>}
      <fieldset disabled={busy||!ready} className="datev-selection">
        <legend>Rechnungen auswählen</legend>
        <div className="datev-fields"><label><span>Rechnungsdatum von</span><input type="date" value={dateFrom} onChange={e=>{setDateFrom(e.target.value);setOffset(0);}}/></label><label><span>Rechnungsdatum bis</span><input type="date" value={dateTo} onChange={e=>{setDateTo(e.target.value);setOffset(0);}}/></label></div>
        <label className="datev-check"><input type="checkbox" checked={includeExported} onChange={e=>setIncludeExported(e.target.checked)}/>Bereits für die Kanzlei ausgegebene Rechnungen anzeigen</label>
        <p>{selected.size} ausgewählt (höchstens 100). Die Auswahl bleibt beim Blättern und Filtern erhalten. <button type="button" className="secondary" onClick={()=>{setSelected(new Map());invalidate();}}>Auswahl leeren</button></p>
        {loading?<p>Rechnungen werden geladen …</p>:<>
          {visible.length===0&&<p>Auf dieser Seite sind keine passenden Rechnungen vorhanden.</p>}
          <div className="datev-invoices">{visible.map(e=><div className="datev-invoice" key={e.id}>
            <input type="checkbox" aria-label={`Rechnung ${e.invoiceNumber} auswählen`} checked={selected.has(e.id)} disabled={!e.documentId||(!selected.has(e.id)&&selected.size>=100)} onChange={v=>toggle(e,v.target.checked)}/>
            <button type="button" className="datev-invoice-title" onClick={()=>void loadDetail(e.id)}><strong>{e.invoiceNumber}</strong><span>{e.buyerName} · {e.issueDate} · {e.format==="xrechnung"?"Behörden-Datei":"PDF-Rechnung"}</span></button>
            <span>{amount(e.grossAmount,e.currency)}<small>{!e.documentId?"Ältere Ausgabe – Rechnungsstand fehlt":exported.includes(e.documentId)?"Bereits ausgegeben":"Noch nicht ausgegeben"}</small></span>
          </div>)}</div>
        </>}
        <div className="inbox-pagination"><button type="button" className="secondary" disabled={offset===0||loading} onClick={()=>setOffset(Math.max(0,offset-50))}>Zurück</button><span>Archivseite {Math.floor(offset/50)+1} von {Math.max(1,Math.ceil(total/50))} · bereits ausgegebene ggf. ausgeblendet</span><button type="button" className="secondary" disabled={offset+50>=total||loading} onClick={()=>setOffset(offset+50)}>Weiter</button></div>
      </fieldset>
      {detail&&invoice&&<fieldset disabled={busy} className="datev-allocation"><legend>Zuordnung für {invoice.invoiceNumber}</legend>
        <p>{invoice.seller.name} → {invoice.buyer.name}</p>
        <button type="button" className="secondary" onClick={()=>updateProfile({...profile,seller:invoice.seller,confirmed:false})}>Absender als eigenen Betrieb übernehmen</button>
        <label><span>Kundenkonto für {invoice.buyer.name}</span><input value={profile.debtors.find(d=>d.buyerIdentity===partyIdentity(invoice.buyer))?.account??""} placeholder="Leer = bestätigtes Sammelkundenkonto" onChange={e=>updateProfile({...profile,confirmed:false,debtors:[...profile.debtors.filter(d=>d.buyerIdentity!==partyIdentity(invoice.buyer)),...(e.target.value?[{buyerIdentity:partyIdentity(invoice.buyer),account:e.target.value}]:[])]})}/></label>
        <label><span>Buchungstext (höchstens 60 Zeichen)</span><input value={detailAssignment?.bookingText??invoice.buyer.name} onChange={e=>assignment({bookingText:e.target.value})}/></label>
        {invoice.lines.map(line=><label key={line.id}><span>{line.id}: {line.name} ({line.tax.rate} %)</span><select value={detailAssignment?.lineAccounts?.[line.id]??""} onChange={e=>assignment({lineAccounts:{...detailAssignment?.lineAccounts,[line.id]:e.target.value}})}><option value="">Automatisch, wenn eindeutig</option>{profile.revenueAccounts.filter(a=>Number(a.taxRate)===Number(line.tax.rate)).map(a=><option key={a.id} value={a.id}>{a.label} – {a.account||"Kontonummer fehlt"}</option>)}</select></label>)}
      </fieldset>}
      <button className="primary" disabled={busy||dirty||!ready||!selected.size} onClick={()=>void buildPreview()}>{busy?"Wird geprüft …":"Vorschau erstellen"}</button>
      {preview&&<section className="datev-preview"><h3>Exportvorschau</h3>
        {preview.issues.length>0?<ul role="alert">{preview.issues.map((p,i)=><li key={i}>{friendly(p)}</li>)}</ul>:<>
          <p>{preview.invoiceCount} Rechnungen · {preview.batches.length} Buchungsstapel und ein Belegpaket · {amount(preview.gross)}. XML- und PDF-Ausgaben desselben Rechnungsstands werden nur einmal berücksichtigt.</p>
          {preview.batches.map(batch=><div key={batch.dateFrom}><h4>{batch.dateFrom} bis {batch.dateTo}</h4><div className="datev-table-scroll"><table><thead><tr><th>Rechnung</th><th>Kundenkonto (Soll)</th><th>Erlöskonto</th><th>Steuersatz</th><th>Netto</th><th>Steuer</th><th>Brutto</th></tr></thead><tbody>{batch.bookings.map((r,i)=><tr key={i}><td>{r.invoiceNumber}</td><td>{r.debtor}</td><td>{r.revenueAccount}{r.taxKey?` / ${r.taxKey}`:" (automatisch)"}</td><td>{r.taxRate} %</td><td>{amount(r.net)}</td><td>{amount(r.tax)}</td><td>{amount(r.gross)}</td></tr>)}</tbody></table></div></div>)}
          <p>Festschreibung beim Import: {profile.locking==="1"?"Ja":"Nein"}. Steuerperiode: {profile.periodRule==="service-date"?"Leistungsdatum":"Rechnungsdatum"}.</p>
          {repeated.length>0&&<div className="datev-notice"><strong>Achtung: Bereits ausgegebene Rechnungen oder mögliche Kopien in der Auswahl.</strong><p>Ein erneuter Import kann doppelte Buchungen erzeugen. Für dieselbe Übergabe bitte die bestehenden Dateien aus der Exporthistorie verwenden.</p><label className="datev-check"><input type="checkbox" checked={repeatConfirmed} disabled={busy} onChange={e=>{setRepeatConfirmed(e.target.checked);request.current=undefined;}}/>Ich möchte bewusst eine neue Ausgabe erzeugen.</label><label><span>Begründung (mindestens 10 Zeichen)</span><input aria-label="Begründung für erneute Ausgabe" value={repeatReason} disabled={busy} onChange={e=>{setRepeatReason(e.target.value);request.current=undefined;}}/></label></div>}
          <p>Das Paket enthält die EXTF-Buchungsstapel und <code>Belege.zip</code> mit DATEV-Verwaltungsdatei, PDF und Rechnungs-XML. Die Kanzlei importiert zuerst das ZIP über DATEV Belegtransfer, danach die CSV-Dateien. Ein lokaler Dateipfad wird nicht als Beleglink verwendet.</p>
          <button className="primary" disabled={busy||dirty||(repeated.length>0&&(!repeatConfirmed||repeatReason.trim().length<10))} onClick={()=>void createExport()}>Paket für die Steuerkanzlei erstellen</button>
        </>}
      </section>}
      <section className="datev-history"><h3>Exporthistorie</h3><p>Eine vorhandene Ausgabe bleibt unverändert. „Ordner öffnen“ prüft die gespeicherten Dateien und erzeugt keinen neuen Stapel.</p>
        {history.entries.length===0&&<p>Noch keine Pakete für die Kanzlei erstellt.</p>}
        {history.entries.map(e=>{let title="Gespeichertes Paket";try{const m=JSON.parse(e.manifest);title=`${m.profile.name} · ${m.invoices.length} Rechnungen · ${m.files.length} Dateien`;}catch{/* damaged manifest remains visible */}
          return <div className="datev-history-entry" key={e.id}><div><strong>{title}</strong><span>{new Date(e.createdAtMs).toLocaleString("de-DE")} · {e.state==="complete"?"Paket erstellt":"Noch nicht abgeschlossen"}</span>{e.error&&<p role="alert">{e.error}</p>}</div><button className="secondary" disabled={busy} onClick={()=>void action(async()=>{if(e.state==="pending"){await datevStore.resume(e.id);setFeedback("Die vorbereitete Ausgabe wurde abgeschlossen.");}else await datevStore.open(e.id);})}>{e.state==="pending"?"Ausgabe fortsetzen":"Ordner öffnen"}</button></div>;})}
        {history.total>100&&<div className="inbox-pagination"><button disabled={busy||historyOffset===0} onClick={()=>setHistoryOffset(Math.max(0,historyOffset-100))}>Neuere</button><span>{historyOffset+1}–{Math.min(historyOffset+100,history.total)} von {history.total}</span><button disabled={busy||historyOffset+100>=history.total} onClick={()=>setHistoryOffset(historyOffset+100)}>Ältere</button></div>}
      </section>
    </div>
  </section>;
}
