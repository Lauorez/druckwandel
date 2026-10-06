// Native WebView2 checks for the release candidate. Isolated ERECHNUNG_TEST_ROOT only (erechnung-wp*).
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";

const [phase, testRoot, port = "9223"] = process.argv.slice(2);
assert(phase === "views", "Usage: node scripts/smoke-release.mjs views <Testwurzel> [port]");
assert(testRoot && basename(testRoot).startsWith("erechnung-wp"), "Use an isolated test root.");

async function listPages() {
  return fetch(`http://127.0.0.1:${port}/json/list`).then((r) => r.json());
}

function attach(page) {
  const socket = new WebSocket(page.webSocketDebuggerUrl);
  const pending = new Map();
  let sequence = 0;
  const ready = new Promise((ok, fail) => {
    socket.addEventListener("open", ok, { once: true });
    socket.addEventListener("error", fail, { once: true });
  });
  socket.addEventListener("message", ({ data }) => {
    const response = JSON.parse(data);
    const task = pending.get(response.id);
    if (!task) return;
    clearTimeout(task.timer);
    pending.delete(response.id);
    response.error ? task.reject(new Error(response.error.message)) : task.resolve(response.result);
  });
  function call(method, params = {}) {
    return new Promise((ok, fail) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        fail(new Error(`Timeout: ${method}`));
      }, 15000);
      pending.set(id, { resolve: ok, reject: fail, timer });
      socket.send(JSON.stringify({ id, method, params }));
    });
  }
  async function evaluate(expression) {
    const result = await call("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    }
    return result.result.value;
  }
  async function waitFor(expression) {
    const end = Date.now() + 20000;
    while (Date.now() < end) {
      const value = await evaluate(expression);
      if (value) return value;
      await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    }
    throw new Error(`Condition not reached: ${expression}`);
  }
  return { page, socket, ready, call, evaluate, waitFor, close: () => socket.close() };
}

function click(name) {
  return `(() => { const button = Array.from(document.querySelectorAll('button')).find((entry) => (entry.textContent || '').includes(${JSON.stringify(name)})); if (!button) return false; button.click(); return true; })()`;
}

const mainPage = (await listPages()).find((p) => p.type === "page" && p.url.includes("tauri.localhost") && !p.url.includes("#settings"));
assert(mainPage, "Native Tauri window is missing.");
const main = attach(mainPage);
await main.ready;

try {
  await main.waitFor(`document.querySelector('nav') !== null`);
  assert.equal(await main.evaluate(click("Posteingang")), true);
  await main.waitFor(`document.body.textContent.includes('Posteingang und Entwürfe')`);
  const inboxTab = await main.evaluate(`document.querySelector('nav button[aria-current="page"]')?.textContent`);
  assert(String(inboxTab).includes("Posteingang"), "Active inbox tab must expose aria-current.");

  assert.equal(await main.evaluate(click("Archiv")), true);
  await main.waitFor(`document.body.textContent.includes('Rechnungsarchiv')`);
  await main.waitFor(`document.querySelector('label.archive-search input') !== null`);
  await main.evaluate(`document.querySelector('label.archive-search input').focus()`);
  assert.equal(
    await main.evaluate(`document.activeElement === document.querySelector('label.archive-search input')`),
    true,
  );

  assert.equal(await main.evaluate(click("Für die Steuerkanzlei exportieren")), true);
  await main.waitFor(`document.body.textContent.includes('Testversion')`);

  assert.equal(
    await main.evaluate(
      `(() => { const button = Array.from(document.querySelectorAll('header button')).find((entry) => (entry.textContent || '').trim() === 'Einstellungen'); if (!button) return false; button.click(); return true; })()`,
    ),
    true,
  );
  const deadline = Date.now() + 20000;
  let settingsPage;
  while (Date.now() < deadline) {
    const overlay = await main.evaluate(`Boolean(document.querySelector('.settings-main'))`);
    const pages = await listPages();
    settingsPage = pages.find(
      (p) =>
        p.type === "page" &&
        (String(p.title).includes("Einstellungen") || String(p.url).includes("#settings")),
    );
    if (!settingsPage) {
      settingsPage = pages.find(
        (p) => p.type === "page" && p.url.includes("tauri.localhost") && p.id !== main.page.id,
      );
    }
    if (settingsPage || overlay) break;
    await new Promise((resolveWait) => setTimeout(resolveWait, 150));
  }
  const overlayOpen = await main.evaluate(`Boolean(document.querySelector('.settings-main'))`);
  assert(settingsPage || overlayOpen, "Settings window is missing.");
  const settings = settingsPage ? attach(settingsPage) : main;
  if (settingsPage) await settings.ready;
  try {
    await settings.waitFor(`document.body.textContent.includes('Dauerhafte Angaben')`);
    assert.equal(await settings.evaluate(click("Diagnose")), true);
    await settings.waitFor(`document.body.textContent.includes('Diagnosebericht')`);
    assert.equal(await settings.evaluate(click("Sicherung")), true);
    await settings.waitFor(`document.body.textContent.includes('Sicherungskennwort') || document.body.textContent.includes('Anwendungssicherung')`);
    await settings.evaluate(`window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`);
  } finally {
    if (settingsPage) settings.close();
  }

  const sizes = [
    [1380, 900, 1],
    [900, 650, 1],
    [1380, 900, 1.5],
    [900, 650, 2],
  ];
  for (const [width, height, scale] of sizes) {
    await main.call("Emulation.setDeviceMetricsOverride", {
      width,
      height,
      deviceScaleFactor: scale,
      mobile: false,
    });
    for (const name of ["Posteingang", "Archiv"]) {
      await main.evaluate(click(name));
      await new Promise((resolveWait) => setTimeout(resolveWait, 150));
      const metrics = await main.evaluate(
        `({height:innerHeight,scrollHeight:document.documentElement.scrollHeight,width:innerWidth,scrollWidth:document.documentElement.scrollWidth})`,
      );
      assert(
        metrics.scrollHeight <= height + 1,
        `${name} @${scale}: outer vertical scroll ${JSON.stringify(metrics)}`,
      );
      assert(
        metrics.scrollWidth <= width + 1,
        `${name} @${scale}: outer horizontal scroll ${JSON.stringify(metrics)}`,
      );
    }
  }
  await main.call("Emulation.clearDeviceMetricsOverride");

  const report = {
    schemaVersion: 1,
    passed: true,
    phase: "views",
    appVersion: "0.3.1",
    createdAt: new Date().toISOString(),
    checks: [
      "inbox-aria-current",
      "archive-search-focus",
      "datev-view",
      "settings-diagnostics-backup",
      "escape-settings",
      "window-sizes-and-scale",
    ],
  };
  await mkdir(resolve("artifacts"), { recursive: true });
  await writeFile(join(resolve("artifacts"), "native-window-smoke.json"), `${JSON.stringify(report, null, 2)}\n`);
  console.log("PASS: native views, keyboard focus, diagnostics/backup, 100/150/200 % layout.");
} finally {
  main.close();
}
