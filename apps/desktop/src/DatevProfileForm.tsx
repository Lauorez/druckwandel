import type { DatevProfile, RevenueAccount } from "../../../src/export/datev/types.js";

export function DatevProfileForm({profile:p,onChange,onSave,busy,dirty,embedded}:{profile:DatevProfile;onChange:(p:DatevProfile)=>void;onSave:()=>void;busy:boolean;dirty:boolean;embedded?:boolean}) {
  const change=(patch:Partial<DatevProfile>)=>onChange({...p,...patch,confirmed:false});
  const field=(label:string,key:"name"|"consultant"|"client"|"fiscalYearStart"|"collectiveDebtor",type="text")=><label><span>{label}</span><input type={type} value={p[key]} onChange={e=>change({[key]:e.target.value})}/></label>;
  const seller=(label:string,key:"name"|"vatId"|"taxRegistrationId")=><label><span>{label}</span><input value={p.seller[key]??""} onChange={e=>change({seller:{...p.seller,[key]:e.target.value}})}/></label>;
  const address=(label:string,key:"line1"|"postalCode"|"city")=><label><span>{label}</span><input value={p.seller.address[key]} onChange={e=>change({seller:{...p.seller,address:{...p.seller.address,[key]:e.target.value}}})}/></label>;
  const updateAccount=(index:number,patch:Partial<RevenueAccount>)=>change({revenueAccounts:p.revenueAccounts.map((a,i)=>i===index?{...a,...patch}:a)});
  return <fieldset disabled={busy} className={`datev-profile${embedded?" embedded":""}`}>
    <legend>Angaben der Steuerkanzlei</legend>
    {!embedded && <p>Bitte gemeinsam mit Ihrer Steuerkanzlei einrichten. Kontonummern werden nicht aus Rechnungen geraten. Diese erste Version unterstützt normale deutsche Ausgangsrechnungen in EUR mit 7 % oder 19 % Umsatzsteuer und Sollversteuerung.</p>}
    <div className="datev-fields">{field("Bezeichnung","name")}{field("Beraternummer","consultant")}{field("Mandantennummer","client")}{field("Beginn des Wirtschaftsjahres","fiscalYearStart","date")}
      <label><span>Sachkontenlänge</span><select value={p.accountLength} onChange={e=>change({accountLength:Number(e.target.value)})}>{[4,5,6,7,8].map(n=><option key={n} value={n}>{n} Stellen</option>)}</select></label>
      <label><span>Kontenrahmen</span><select value={p.chart} onChange={e=>change({chart:e.target.value as "03"|"04"})}><option value="03">SKR03</option><option value="04">SKR04</option></select></label>
    </div>
    <h3>Eigener Betrieb</h3><p>Nur Rechnungen dieses Absenders dürfen als Ausgangsrechnungen exportiert werden. Schreibweise und Anschrift müssen mit den gespeicherten Rechnungen übereinstimmen.</p>
    <div className="datev-fields">{seller("Name des Betriebs","name")}{address("Straße und Hausnummer","line1")}{address("Postleitzahl","postalCode")}{address("Ort","city")}{seller("Umsatzsteuer-ID (falls vorhanden)","vatId")}{seller("Steuernummer (falls vorhanden)","taxRegistrationId")}</div>
    <h3>Vereinbarte Buchungseinstellungen</h3><div className="datev-fields">
      <label><span>Versteuerung</span><select value={p.accountingMethod} onChange={e=>change({accountingMethod:e.target.value as DatevProfile["accountingMethod"]})}><option value="">Bitte auswählen</option><option value="accrual">Sollversteuerung</option><option value="cash">Istversteuerung – noch nicht unterstützt</option></select></label>
      <label><span>Zuordnung zur Steuerperiode</span><select value={p.periodRule} onChange={e=>change({periodRule:e.target.value as DatevProfile["periodRule"]})}><option value="">Bitte mit der Kanzlei klären</option><option value="invoice-date">Nach Rechnungsdatum</option><option value="service-date">Nach Leistungsdatum</option></select></label>
      <label><span>Festschreibung beim Import</span><select value={p.locking} onChange={e=>change({locking:e.target.value as DatevProfile["locking"]})}><option value="">Bitte mit der Kanzlei klären</option><option value="0">Nicht automatisch festschreiben</option><option value="1">Beim Import festschreiben</option></select></label>
    </div>
    <h3>Erlöskonten</h3><p>Automatikkonten dürfen nicht zusätzlich einen Steuerschlüssel erhalten. Mehrere Konten für denselben Steuersatz werden bei der Rechnung einzeln zugeordnet.</p>
    {p.revenueAccounts.map((a,i)=><div className="datev-account" key={a.id}>
      <label><span>Bezeichnung</span><input aria-label={`Erlöskonto ${i+1}: Bezeichnung`} value={a.label} onChange={e=>updateAccount(i,{label:e.target.value})}/></label>
      <label><span>Steuersatz</span><select aria-label={`Erlöskonto ${i+1}: Steuersatz`} value={a.taxRate} onChange={e=>updateAccount(i,{taxRate:e.target.value as "7"|"19",taxKey:a.mode==="automatic"?"":e.target.value==="7"?"0002":"0003"})}><option value="19">19 %</option><option value="7">7 %</option></select></label>
      <label><span>Kontonummer</span><input aria-label={`Erlöskonto ${i+1}: Kontonummer`} value={a.account} onChange={e=>updateAccount(i,{account:e.target.value})}/></label>
      <label><span>Steuerberechnung</span><select value={a.mode} onChange={e=>updateAccount(i,{mode:e.target.value as RevenueAccount["mode"],taxKey:e.target.value==="automatic"?"":a.taxRate==="7"?"0002":"0003"})}><option value="automatic">Automatikkonto</option><option value="tax-key">Mit Steuerschlüssel {a.taxRate==="7"?"0002":"0003"}</option></select></label>
      <button type="button" className="secondary" onClick={()=>change({revenueAccounts:p.revenueAccounts.filter((_,n)=>i!==n)})}>Entfernen</button>
    </div>)}
    <button type="button" className="secondary" onClick={()=>change({revenueAccounts:[...p.revenueAccounts,{id:crypto.randomUUID(),label:"Weiteres Erlöskonto",account:"",taxRate:"19",mode:"automatic",taxKey:""}]})}>Erlöskonto hinzufügen</button>
    <h3>Kundenkonten</h3><p>Einzelne Kundenkonten können unten an der ausgewählten Rechnung hinterlegt werden. Alternativ kann die Kanzlei ein gemeinsames Konto freigeben.</p>
    <div className="datev-fields">{field("Sammelkundenkonto (optional)","collectiveDebtor")}</div>
    <label className="datev-check"><input type="checkbox" checked={p.collectiveDebtorConfirmed} onChange={e=>change({collectiveDebtorConfirmed:e.target.checked})}/>Die Kanzlei hat dieses Sammelkundenkonto ausdrücklich freigegeben.</label>
    <label className="datev-check"><input type="checkbox" checked={p.confirmed} onChange={e=>onChange({...p,confirmed:e.target.checked})}/>Betrieb, Konten und Buchungseinstellungen wurden mit der Steuerkanzlei abgestimmt.</label>
    <div className="datev-save-row">
      <button type="button" className="primary" onClick={onSave} disabled={!dirty}>Speichern</button>
    </div>
  </fieldset>;
}
