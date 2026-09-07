// Native WebView2 acceptance test. Run only with a debug build and an isolated
// ERECHNUNG_TEST_ROOT named erechnung-wp7-*. No real user documents are changed.
import assert from "node:assert/strict";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { basename, join, resolve } from "node:path";

const [phase, testRoot, port = "9223"] = process.argv.slice(2);
assert(["prepare","resume","close"].includes(phase));
assert(testRoot && basename(testRoot).startsWith("erechnung-wp7-"),"Use an isolated test root.");
const pages = await fetch(`http://127.0.0.1:${port}/json/list`).then(r => r.json());
const page = pages.find(p => p.type === "page" && p.url.includes("tauri.localhost"));
assert(page,"Native Tauri window is missing.");
const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve,reject) => { socket.addEventListener("open",resolve,{ once: true }); socket.addEventListener("error",reject,{ once: true }); });
let sequence = 0;
const pending = new Map();
socket.addEventListener("message",({ data }) => {
  const response = JSON.parse(data), task = pending.get(response.id);
  if (!task) return;
  clearTimeout(task.timer); pending.delete(response.id);
  response.error ? task.reject(new Error(response.error.message)) : task.resolve(response.result);
});
function call(method,params = {}) {
  return new Promise((resolve,reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timeout: ${method}`)); },15000);
    pending.set(id,{ resolve,reject,timer }); socket.send(JSON.stringify({ id,method,params }));
  });
}
async function evaluate(expression) {
  const result = await call("Runtime.evaluate",{ expression,returnByValue: true,awaitPromise: true });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
}
async function waitFor(expression) {
  const end = Date.now()+20000;
  while (Date.now()<end) {
    const value = await evaluate(expression);
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve,100));
  }
  throw new Error(`Condition not reached: ${expression}`);
}
const native = (command,args = {}) => `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)},${JSON.stringify(args)})`;
const seller = `document.querySelector('[aria-label="Absender: Name: Im PDF markieren"]')?.closest('.source-field').querySelector('input')`;
const setSeller = name => `(() => { const input = ${seller}; Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,${JSON.stringify(name)}); input.dispatchEvent(new Event('input',{ bubbles:true })); })()`;
try {
  await waitFor(`document.querySelector('nav') !== null`);
  if (phase === "prepare") {
    await waitFor(`document.body.textContent.includes('Bereit – neue Rechnungen')`);
    const tree = await call("DOM.getDocument");
    const input = await call("DOM.querySelector",{ nodeId: tree.root.nodeId,selector: 'header input[accept="application/pdf,.pdf"]' });
    await call("DOM.setFileInputFiles",{ nodeId: input.nodeId,files: [resolve("test/corpus/pdf/standard-header-merged.pdf")] });
    await waitFor(`${seller} !== undefined`);
    await evaluate(setSeller("WP7 Wiederaufnahme"));
    await waitFor(`(async () => { const page = await ${native("workspace_list")}; const row = page.entries[0]; if (!row) return false; const detail = await window.__TAURI_INTERNALS__.invoke('workspace_read',{id:row.id}); return detail.snapshot && JSON.parse(detail.snapshot).draft.seller.name === 'WP7 Wiederaufnahme'; })()`);
    const inbox = join(testRoot,"documents","E-Rechnung Druckeingang");
    await mkdir(inbox,{ recursive:true });
    const pdf = await readFile("test/corpus/pdf/legacy-columns.pdf");
    for (let i=0; i<10; i++) {
      const id = randomUUID();
      await writeFile(join(inbox,`${id}.pdf`),pdf);
      await writeFile(join(inbox,`${id}.printjob.json`),JSON.stringify({schemaVersion:1,jobId:id,handedOffAt:new Date().toISOString(),pdfFileName:`${id}.pdf`,documentName:`WP7 Drucktest ${i+1}`,printerName:"E-Rechnung",pages:1}));
    }
    await waitFor(`(async () => (await ${native("workspace_list")}).total === 11)()`);
    assert.equal(await evaluate(`(${seller}).value`),"WP7 Wiederaufnahme");
    assert.equal(await evaluate(native("read_learning_memory")),null,"Autosave must not confirm learning.");
    console.log("PASS: native import, autosave, ten queued print jobs without replacing the active invoice.");
  } else if (phase === "resume") {
    await waitFor(`(${seller})?.value === 'WP7 Wiederaufnahme'`);
    assert.equal((await evaluate(native("workspace_list"))).total,11);
    for (const [width,height] of [[1380,900],[900,650]]) {
      await call("Emulation.setDeviceMetricsOverride",{width,height,deviceScaleFactor:1,mobile:false});
      for (const name of ["Posteingang (11)","Rechnung","Archiv"]) {
        await evaluate(`Array.from(document.querySelectorAll('nav button')).find(b=>b.textContent===${JSON.stringify(name)}).click()`);
        await new Promise(resolve=>setTimeout(resolve,150));
        const metrics = await evaluate(`({height:innerHeight,scrollHeight:document.documentElement.scrollHeight,width:innerWidth,scrollWidth:document.documentElement.scrollWidth})`);
        assert(metrics.scrollHeight<=height+1,`${name}: outer vertical scroll ${JSON.stringify(metrics)}`);
        assert(metrics.scrollWidth<=width+1,`${name}: outer horizontal scroll ${JSON.stringify(metrics)}`);
      }
    }
    await call("Emulation.clearDeviceMetricsOverride");
    await evaluate(`Array.from(document.querySelectorAll('nav button')).find(b=>b.textContent==='Rechnung').click()`);
    console.log("PASS: native restart restored saved input; all three views fit both window sizes.");
  } else {
    await waitFor(`(${seller}) !== undefined`);
    // Change and request close in one turn, before the 450 ms autosave timer.
    const expected = `WP7 Schließen ${randomUUID()}`;
    await evaluate(`${setSeller(expected)}; setTimeout(() => { void ${native("plugin:window|close",{label:"main"})}.catch(e => { document.body.dataset.closeError = String(e); }); },0); true`);
    const deadline = Date.now()+20000;
    let closed = false;
    while (Date.now()<deadline) {
      const remaining = await fetch(`http://127.0.0.1:${port}/json/list`).then(r=>r.json()).catch(()=>[]);
      if (!remaining.some(p=>p.id===page.id)) { closed = true; break; }
      await new Promise(resolve=>setTimeout(resolve,100));
    }
    assert(closed,"Window did not close; inspect the visible save error.");
    const { DatabaseSync } = await import("node:sqlite");
    const db = new DatabaseSync(join(testRoot,"local","workspace","workspace.sqlite3"),{readOnly:true});
    try {
      const row = db.prepare("SELECT snapshot FROM documents WHERE id=(SELECT value FROM settings WHERE key='last_opened')").get();
      assert.equal(JSON.parse(row.snapshot).draft.seller.name,expected);
    } finally { db.close(); }
    console.log("PASS: closing flushed a unique pending input before the autosave timer; native window closed.");
  }
} finally { socket.close(); }
