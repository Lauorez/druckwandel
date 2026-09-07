// Read-only download of the public content used by the official DATEV portal.
import { mkdir, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
const url = "https://developer.datev.de/mediator/strapi/file-formats/slug/datev-format";
const response = await fetch(url);
if (!response.ok) throw new Error(`DATEV reference: HTTP ${response.status}`);
const bytes = Buffer.from(await response.arrayBuffer());
const document = JSON.parse(bytes.toString("utf8"));
if (document.id !== 5 || !document.fileFormatPages?.length) throw new Error("Unexpected DATEV document.");
await mkdir("artifacts/datev/reference",{recursive:true});
await writeFile("artifacts/datev/reference/portal.json",bytes);
const manifest = {url,retrievedAt:new Date().toISOString(),sha256:createHash("sha256").update(bytes).digest("hex")};
await writeFile("artifacts/datev/reference/source.json",JSON.stringify(manifest,null,2)+"\n");
console.log(manifest);
function list(pages,prefix="") { for (const p of pages) { console.log(`${prefix}${p.id}: ${p.title}`); list(p.fileFormatPages??[],prefix+"  "); } }
list(document.localization?.fileFormatPages??document.fileFormatPages);
if (process.argv.includes("--tools")) {
  for (const name of ["Musterdaten_DATEV_Format_0_7f9322b9cc.zip","Datev_Format_Pruefprogramm_2_2_3_0_76439824cb.zip"]) {
    const source = `https://developer.datev.de/assets/${name}`;
    const download = await fetch(source);
    if (!download.ok) throw new Error(`DATEV download: HTTP ${download.status}`);
    const data = Buffer.from(await download.arrayBuffer());
    if (data[0]!==0x50 || data[1]!==0x4b) throw new Error("Expected a ZIP archive.");
    await writeFile(`artifacts/datev/reference/${name}`,data);
    await writeFile(`artifacts/datev/reference/${name}.json`,JSON.stringify({url:source,retrievedAt:new Date().toISOString(),sha256:createHash("sha256").update(data).digest("hex")},null,2)+"\n");
    console.log(`Downloaded ${name}: ${data.length} bytes`);
  }
}
