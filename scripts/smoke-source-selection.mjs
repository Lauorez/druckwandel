// Run against a locally launched WebView2 with remote debugging enabled.
// Selects and accepts a source without confirming learning. The native workspace
// autosaves the draft; use an isolated debug ERECHNUNG_TEST_ROOT for this test.
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const [port = "9223", sourceText, screenshotPath] = process.argv.slice(2);
if (!sourceText || !/^\d+$/.test(port)) throw new Error("Usage: node scripts/smoke-source-selection.mjs <port> <source-text> [screenshot.png]");
const pages = await fetch(`http://127.0.0.1:${port}/json/list`).then((response) => response.json());
const page = pages.find((candidate) => candidate.type === "page" && candidate.url.includes("tauri.localhost"));
if (!page) throw new Error("No local Tauri page found.");
const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.addEventListener("open", resolve, { once: true }); socket.addEventListener("error", reject, { once: true }); });
let sequence = 0;
const requests = new Map();
socket.addEventListener("message", ({ data }) => {
  const message = JSON.parse(data);
  const request = requests.get(message.id);
  if (!request) return;
  requests.delete(message.id);
  clearTimeout(request.timeout);
  if (message.error) request.reject(new Error(message.error.message));
  else request.resolve(message.result);
});
function call(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    const timeout = setTimeout(() => { requests.delete(id); reject(new Error(`Timeout: ${method}`)); }, 10000);
    requests.set(id, { resolve, reject, timeout });
    socket.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const response = await call("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (response.exceptionDetails) throw new Error(response.exceptionDetails.exception?.description ?? response.exceptionDetails.text);
  return response.result.value;
}
async function waitFor(expression) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const value = await evaluate(expression);
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Condition not reached: ${expression}`);
}
try {
  await waitFor(`document.querySelector('[aria-label="Absender: Name: Im PDF markieren"]') !== null`);
  await evaluate(`document.querySelector('[aria-label="Absender: Name: Im PDF markieren"]').click()`);
  const selector = `.pdf-source-token[aria-label=${JSON.stringify(sourceText)}]`;
  await waitFor(`document.querySelector(${JSON.stringify(selector)}) !== null`);
  await evaluate(`document.querySelector(${JSON.stringify(selector)}).scrollIntoView({ block: "center" })`);
  const bounds = await evaluate(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; })()`);
  await call("Input.dispatchMouseEvent", { type: "mouseMoved", x: bounds.x - 1, y: bounds.y - 1 });
  await call("Input.dispatchMouseEvent", { type: "mousePressed", x: bounds.x - 1, y: bounds.y - 1, button: "left", clickCount: 1 });
  await call("Input.dispatchMouseEvent", { type: "mouseMoved", x: bounds.x + bounds.width + 1, y: bounds.y + bounds.height + 1, button: "left", buttons: 1 });
  await call("Input.dispatchMouseEvent", { type: "mouseReleased", x: bounds.x + bounds.width + 1, y: bounds.y + bounds.height + 1, button: "left", clickCount: 1 });
  const selected = await waitFor(`document.querySelector('[aria-label="Wert aus der Markierung"]')?.value`);
  assert.equal(selected, sourceText, "The drawn rectangle must select only the intended text block.");
  if (screenshotPath) {
    const screenshot = await call("Page.captureScreenshot", { format: "png" });
    const target = resolve(screenshotPath);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, Buffer.from(screenshot.data, "base64"));
  }
  await evaluate(`Array.from(document.querySelectorAll('.source-picker button')).find(button => button.textContent === 'Übernehmen').click()`);
  await waitFor(`document.querySelector('.source-picker') === null`);
  const assigned = await evaluate(`document.querySelector('[aria-label="Absender: Name: Im PDF markieren"]').closest('.source-field').querySelector('input').value`);
  assert.equal(assigned, sourceText);
  console.log("PASS: native WebView2 rectangle selection, preview and assignment. No learning was confirmed.");
} finally {
  socket.close();
}
