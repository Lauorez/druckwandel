import { invoke } from "@tauri-apps/api/core";
import type { DatevProfile, DatevSource } from "../../../src/export/datev/types.js";
export interface DatevExport { id:string;createdAtMs:number;state:"pending"|"complete";manifest:string;error:string|null }
export interface DatevExportPage { entries:DatevExport[];total:number }
export interface DatevExportRequest {
  id:string;profile:string;repeatReason:string;
  files:Array<{contentsBase64:string;dateFrom:string;dateTo:string;gross:string;bookingCount:number;archiveIds:string[]}>;
  documentPackage:{xml:string;files:Array<{archiveId:string;guid:string;pdfName:string;xmlName:string}>};
}
export const datevStore = {
  profile:()=>invoke<string|null>("datev_get_profile"),
  saveProfile:(contents:string)=>invoke<void>("datev_save_profile",{contents}),
  source:(id:string)=>invoke<DatevSource>("archive_datev_source",{id}),
  status:(documentIds:string[])=>invoke<string[]>("datev_export_status",{documentIds}),
  duplicates:(archiveIds:string[])=>invoke<string[]>("datev_check_duplicates",{archiveIds}),
  create:(request:DatevExportRequest)=>invoke<DatevExport>("datev_create_export",{request}),
  history:(offset=0)=>invoke<DatevExportPage>("datev_list_exports",{offset}),
  resume:(id:string)=>invoke<DatevExport>("datev_resume_export",{id}),
  open:(id:string)=>invoke<void>("datev_open_export",{id}),
};
export function emptyDatevProfile(): DatevProfile {
  return {schemaVersion:1,name:"Meine Steuerkanzlei",consultant:"",client:"",fiscalYearStart:`${new Date().getFullYear()}-01-01`,accountLength:4,chart:"03",seller:{name:"",address:{line1:"",postalCode:"",city:"",countryCode:"DE"}},confirmed:false,accountingMethod:"",periodRule:"",locking:"",collectiveDebtor:"",collectiveDebtorConfirmed:false,debtors:[],revenueAccounts:[{id:"standard19",label:"Erlöse 19 %",taxRate:"19",account:"",mode:"automatic",taxKey:""},{id:"standard7",label:"Erlöse 7 %",taxRate:"7",account:"",mode:"automatic",taxKey:""}]};
}
