// Takes the README screenshots from the mock backend, in a headless Edge or Chrome driven through
// the DevTools protocol (no dependencies). Start the UI first: `cd app && npm run dev`.
//
//   node tools/screenshots.mjs [outDir]          (default: docs/screenshots)
//
// The browser is found in the usual places; set FILEEPOCH_BROWSER to use another one.
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const BASE = "http://127.0.0.1:1420/?shot&lang=en&";
const clickButton = (label) =>
  `(() => { const b = [...document.querySelectorAll("button")].find((b) => (b.title || "").includes(${JSON.stringify(label)}) || b.textContent.trim() === ${JSON.stringify(label)}); b?.click(); return !!b; })()`;

const SHOTS = [
  { name: "main", query: "theme=light&source=2", wait: 3000 },
  { name: "thumbnails", query: "theme=dark&source=1&path=Fotos&view=icons", wait: 4500 },
  { name: "restores", query: "theme=light&source=1&jobs=1", wait: 2500, steps: [{ js: clickButton("Restores"), wait: 5000 }] },
];

const outDir = process.argv[2] ?? join(import.meta.dirname, "..", "docs", "screenshots");
const browser = [
  process.env.FILEEPOCH_BROWSER,
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
].find((p) => p && existsSync(p));
if (!browser) throw new Error("No Edge or Chrome found; set FILEEPOCH_BROWSER.");
try {
  await fetch(BASE);
} catch {
  throw new Error("The UI is not running at http://127.0.0.1:1420 (cd app && npm run dev).");
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const profile = mkdtempSync(join(tmpdir(), "fileepoch-shots-"));
mkdirSync(outDir, { recursive: true });
// Port 0: the browser picks a free port and writes it, with the WebSocket path, to DevToolsActivePort.
const proc = spawn(browser, [
  "--headless=new", "--disable-gpu", "--hide-scrollbars", "--no-first-run",
  "--remote-debugging-port=0", `--user-data-dir=${profile}`, "about:blank",
], { stdio: "ignore" });

let wsUrl;
for (let i = 0; i < 100 && !wsUrl; i++) {
  try {
    const [port, path] = readFileSync(join(profile, "DevToolsActivePort"), "utf-8").split("\n");
    if (port && path) wsUrl = `ws://127.0.0.1:${port.trim()}${path.trim()}`;
  } catch {
    // not written yet
  }
  if (!wsUrl) await sleep(100);
}
if (!wsUrl) throw new Error("The browser did not open its debugging port.");
const ws = new WebSocket(wsUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let nextId = 0;
const pending = new Map();
ws.addEventListener("message", (ev) => {
  const msg = JSON.parse(ev.data);
  const p = msg.id && pending.get(msg.id);
  if (!p) return;
  pending.delete(msg.id);
  msg.error ? p.reject(new Error(JSON.stringify(msg.error))) : p.resolve(msg.result);
});
const send = (method, params = {}, sessionId) =>
  new Promise((resolve, reject) => {
    const id = ++nextId;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params, sessionId }));
  });

try {
  for (const shot of SHOTS) {
    const { targetId } = await send("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
    await send("Emulation.setDeviceMetricsOverride", { width: 1200, height: 760, deviceScaleFactor: 2, mobile: false }, sessionId);
    await send("Page.navigate", { url: BASE + shot.query }, sessionId);
    await sleep(shot.wait);
    for (const step of shot.steps ?? []) {
      const r = await send("Runtime.evaluate", { expression: step.js, returnByValue: true }, sessionId);
      if (r.exceptionDetails || r.result?.value === false) throw new Error(`${shot.name}: a step failed`);
      await sleep(step.wait);
    }
    const { data } = await send("Page.captureScreenshot", { format: "png" }, sessionId);
    writeFileSync(join(outDir, `${shot.name}.png`), Buffer.from(data, "base64"));
    console.log(`${shot.name}.png`);
    await send("Target.closeTarget", { targetId });
  }
} finally {
  const exited = new Promise((r) => proc.once("exit", r));
  await send("Browser.close").catch(() => {});
  ws.close();
  if ((await Promise.race([exited.then(() => true), sleep(5000)])) !== true) {
    // Still running: end it with its helper processes.
    if (process.platform === "win32") spawnSync("taskkill", ["/T", "/F", "/PID", String(proc.pid)], { stdio: "ignore" });
    else proc.kill("SIGKILL");
  }
  try {
    // The browser's helper processes can hold the profile for a moment after it exits.
    rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
  } catch {
    console.warn(`Could not delete the temporary browser profile ${profile}`);
  }
}
