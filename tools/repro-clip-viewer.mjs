import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { setTimeout as sleep } from "node:timers/promises";

// Clip viewer evidence: drives the real app through CDP and checks the viewer
// end to end — right-click from the main app and the dock, the row menu's
// first item, one window reused across opens, monitor-aware placement,
// remembered size, copy feedback, the missing-Clip empty state,
// Esc/close-control dismissal, focus return, native window style (visible,
// not always-on-top), and the zoom/pan surface (range clamp, fit, 100 %/DPR,
// cursor-anchored Ctrl+wheel, wheel/Shift+wheel and arrow panning, drag-pan,
// double-click toggle, keyboard shortcuts, and reduced motion). Runs against a
// throwaway LOCALAPPDATA so it never touches the user's Library.
// Exit: 0 green, 1 red, 2 infra.

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TEMP_ROOT = path.join(os.tmpdir(), "opencode");
const DATA_DIR = path.join(TEMP_ROOT, "sprout-226-appdata");
const WINDOW_HELPER = path.join(ROOT, "tools", "repro-clip-viewer-windows.ps1");

const USAGE = `Usage: node tools/repro-clip-viewer.mjs [options]

Seeds image Clips in a throwaway app-data directory and checks the clip
viewer: main/dock right-click, menu item, reuse, placement, size memory,
copy feedback, missing Clip, Esc/close, focus return, native window style,
and zoom/pan (range, fit, 100 %/DPR, cursor zoom, wheel/keys/drag, motion).

Options:
  --mode dev|exe   app under test (default dev)
  --port P         CDP port (default: first free of 9222,9333,9444,9555,9666)
  --shots DIR      save PNGs (default: %TEMP%\\opencode\\evidence-226)
  --keep           leave the app running after the verdict
  --help`;

function parseArgs(argv) {
  const args = {
    mode: "dev",
    port: null,
    shots: path.join(TEMP_ROOT, "evidence-226"),
    keep: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--help":
        console.log(USAGE);
        process.exit(0);
        break;
      case "--keep":
        args.keep = true;
        break;
      case "--shots":
        args.shots = argv[++i];
        break;
      case "--mode":
        args.mode = argv[++i];
        break;
      case "--port":
        args.port = Number(argv[++i]);
        break;
      default:
        throw new Error(`unknown option: ${a}\n${USAGE}`);
    }
  }
  return args;
}

// ----------------------------- tiny PNG ------------------------------------

const CRC_TABLE = (() => {
  const table = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c;
  }
  return table;
})();

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
  return Buffer.concat([head, data, crc]);
}

/** A deterministic 8-bit RGBA PNG — the seeded Clips need real bytes the
 *  backend sniffs as PNG, with a visible gradient so the screenshots read. */
function makePng(width, height, hue) {
  const stride = width * 4 + 1;
  const raw = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    raw[y * stride] = 0; // filter: none
    for (let x = 0; x < width; x++) {
      const i = y * stride + 1 + x * 4;
      raw[i] = (hue + x * 2) & 255;
      raw[i + 1] = (hue + y * 2) & 255;
      raw[i + 2] = 180;
      raw[i + 3] = 255;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", zlib.deflateSync(raw)),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

// ----------------------------- CDP plumbing --------------------------------

function killStraySprout() {
  try {
    spawnSync("taskkill", ["/IM", "sprout-windows-desktop.exe", "/F"], { stdio: "ignore" });
  } catch {}
}

async function pickFreePort(preferred) {
  const candidates = preferred ? [preferred, 9333, 9444, 9555, 9666] : [9222, 9333, 9444, 9555, 9666];
  for (const port of candidates) {
    const free = await new Promise((resolve) => {
      const srv = net.createServer();
      srv.once("error", () => resolve(false));
      srv.once("listening", () => srv.close(() => resolve(true)));
      srv.listen(port, "127.0.0.1");
    });
    if (free) return port;
  }
  throw new Error("no free CDP port among " + candidates.join(","));
}

async function fetchJson(url) {
  const res = await fetch(url, { signal: AbortSignal.timeout(2000) });
  if (!res.ok) throw new Error(`HTTP ${res.status} from ${url}`);
  return res.json();
}

class Cdp {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.id = 0;
    this.pending = new Map();
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (ev) => {
      const msg = JSON.parse(ev.data);
      if (!msg.id) return;
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if (msg.error) p.reject(new Error(msg.error.message));
      else p.resolve(msg.result);
    });
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", () => reject(new Error("CDP websocket connect failed")), {
        once: true,
      });
    });
  }

  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  async eval(expression) {
    const r = await this.send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (r.exceptionDetails) {
      throw new Error(
        "eval threw: " + (r.exceptionDetails.exception?.description ?? r.exceptionDetails.text)
      );
    }
    return r.result?.value;
  }

  close() {
    try {
      this.ws.close();
    } catch {}
  }
}

function launch(mode, port) {
  const env = {
    ...process.env,
    LOCALAPPDATA: DATA_DIR,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
  };
  if (mode === "dev") {
    const child = spawn("cmd.exe", ["/c", "npm.cmd run tauri dev"], {
      cwd: ROOT,
      env,
      stdio: ["ignore", "ignore", "pipe"],
    });
    const tail = [];
    child.stderr.on("data", (d) => {
      tail.push(d.toString());
      if (tail.length > 20) tail.shift();
    });
    child.tail = tail;
    return child;
  }
  return spawn(path.join(ROOT, "dist", "Sprout.exe"), [], {
    cwd: ROOT,
    env,
    stdio: "ignore",
  });
}

function killTree(child) {
  try {
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
  } catch {}
}

function pages(list) {
  return list.filter(
    (t) => t.type === "page" && t.url !== "about:blank" && !t.url.startsWith("devtools://")
  );
}

async function targets(port) {
  return pages(await fetchJson(`http://127.0.0.1:${port}/json`));
}

async function attach(target) {
  const cdp = new Cdp(target.webSocketDebuggerUrl);
  await cdp.connect();
  await cdp.send("Runtime.enable");
  await cdp.send("Page.enable");
  return cdp;
}

// The app boots to the tray (ADR-0013), so the first page target is the Quick
// Launch window. Ask it to open the main window, then attach to the target
// that renders the nav rail.
async function connectMainWindow(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let asked = false;
  let lastErr = null;
  while (Date.now() < deadline) {
    try {
      const list = await targets(port);
      for (const target of list) {
        const candidate = new Cdp(target.webSocketDebuggerUrl);
        try {
          await candidate.connect();
          await candidate.send("Runtime.enable");
          const hasRail = await candidate.eval(`!!document.querySelector(".rail")`);
          if (hasRail) return { cdp: candidate, target };
        } catch (e) {
          lastErr = e;
        }
        candidate.close();
      }
      if (list.length > 0 && !asked) {
        asked = true;
        const opener = new Cdp(list[0].webSocketDebuggerUrl);
        try {
          await opener.connect();
          await opener.send("Runtime.enable");
          await opener.eval(`window.__TAURI_INTERNALS__.invoke("open_main_window_cmd")`);
        } catch (e) {
          lastErr = e;
        }
        opener.close();
      }
    } catch (e) {
      lastErr = e;
    }
    await sleep(500);
  }
  throw new Error(`main window never appeared (${lastErr ?? "no page with a nav rail"})`);
}

async function waitFor(cdp, expression, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      if (await cdp.eval(expression)) return;
    } catch {}
    await sleep(100);
  }
  let state = "unavailable";
  try {
    state = await cdp.eval(
      `JSON.stringify({ href: location.href, theme: document.documentElement.dataset.theme, text: document.body?.innerText?.slice(0, 240) ?? null })`
    );
  } catch {}
  throw new Error(`${label} never appeared; page state=${state}`);
}

async function capture(cdp, dir, name) {
  const shot = await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true });
  mkdirSync(dir, { recursive: true });
  const file = path.join(dir, name);
  writeFileSync(file, Buffer.from(shot.data, "base64"));
  console.log(`shot: ${file}`);
}

async function pressKey(cdp, key, code, vk, modifiers = 0) {
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key,
    code,
    windowsVirtualKeyCode: vk,
    nativeVirtualKeyCode: vk,
    modifiers,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key,
    code,
    windowsVirtualKeyCode: vk,
    nativeVirtualKeyCode: vk,
    modifiers,
  });
}

/** A real right-click over an element, so the page's own contextmenu path
 *  (button === 2) runs — not a synthetic dispatchEvent. */
async function rightClick(cdp, expression, label) {
  const point = await cdp.eval(`(() => {
    const el = ${expression};
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  })()`);
  if (!point) throw new Error(`${label}: target element not found`);
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: point.x,
    y: point.y,
    button: "right",
    buttons: 2,
    clickCount: 1,
  });
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: point.x,
    y: point.y,
    button: "right",
    buttons: 0,
    clickCount: 1,
  });
}

async function wheelAt(cdp, { x, y, deltaY, modifiers = 0 }) {
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseWheel",
    x,
    y,
    deltaX: 0,
    deltaY,
    modifiers,
  });
}

/** A real double-click: Chromium emits `dblclick` on the second release. */
async function doubleClickAt(cdp, x, y) {
  for (const clickCount of [1, 2]) {
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mousePressed",
      x,
      y,
      button: "left",
      buttons: 1,
      clickCount,
    });
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseReleased",
      x,
      y,
      button: "left",
      buttons: 0,
      clickCount,
    });
  }
}

/** A real pointer drag through Chromium's input pipeline, so the page's
 *  pointer capture and pointermove path runs instead of a synthetic event. */
async function dragMouse(cdp, from, to) {
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: from.x,
    y: from.y,
    button: "left",
    buttons: 1,
    clickCount: 1,
  });
  const steps = 4;
  for (let i = 1; i <= steps; i++) {
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: from.x + ((to.x - from.x) * i) / steps,
      y: from.y + ((to.y - from.y) * i) / steps,
      button: "left",
      buttons: 1,
    });
  }
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: to.x,
    y: to.y,
    button: "left",
    buttons: 0,
    clickCount: 1,
  });
}

async function click(cdp, expression, label) {
  const clicked = await cdp.eval(`(() => {
    const el = ${expression};
    if (!el) return false;
    el.click();
    return true;
  })()`);
  if (!clicked) throw new Error(`${label}: target element not found`);
}

// ----------------------------- app probes ----------------------------------

const SEED = (pngA, pngB) => `(async () => {
  const inv = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
  const first = await inv("create_clip_image", { name: "Viewer probe", dataBase64: ${JSON.stringify(pngA)} });
  const second = await inv("create_clip_image", { name: "Dock probe", dataBase64: ${JSON.stringify(pngB)} });
  const text = await inv("create_clip", { clip: { name: "Text probe", content: "just words", show_in_dock: true } });
  return { first: first.id, second: second.id, text: text.id };
})()`;

const ZOOM_PROBE = `(() => {
  const img = document.querySelector("img.viewer__image");
  const r = img?.getBoundingClientRect();
  const readout = document.querySelector('button[aria-label^="Zoom level"] output');
  return {
    percent: readout?.textContent?.trim() ?? null,
    live: readout?.getAttribute("aria-live") ?? null,
    labels: [...document.querySelectorAll("button")].map((b) => b.getAttribute("aria-label")),
    image: r ? { left: r.left, top: r.top, width: r.width, height: r.height } : null,
    dpr: devicePixelRatio,
    inner: { w: innerWidth, h: innerHeight },
  };
})()`;

const VIEWER_PROBE = `(() => {
  const buttons = [...document.querySelectorAll("button")];
  const copy = buttons.find((b) => (b.getAttribute("aria-label") ?? "").startsWith("Copy "));
  const close = buttons.find((b) => b.getAttribute("aria-label") === "Close viewer");
  const img = document.querySelector("img.viewer__image");
  return {
    title: document.querySelector("h1")?.textContent?.trim() ?? null,
    meta: document.querySelector(".viewer__meta")?.textContent?.trim() ?? null,
    imageSrc: img?.getAttribute("src")?.slice(0, 22) ?? null,
    imageAlt: img?.getAttribute("alt") ?? null,
    copyLabel: copy?.getAttribute("aria-label") ?? null,
    copyText: copy?.textContent?.trim() ?? null,
    closeLabel: close ? "Close viewer" : null,
    emptyTitle: document.querySelector(".empty__title")?.textContent?.trim() ?? null,
    emptyBody: document.querySelector(".empty__body")?.textContent?.trim() ?? null,
    live: document.querySelector('[role="status"]')?.textContent?.trim() ?? null,
    dragRegion: !!document.querySelector("[data-tauri-drag-region]"),
    theme: document.documentElement.dataset.theme,
    inner: { w: window.innerWidth, h: window.innerHeight, dpr: window.devicePixelRatio },
    focused: document.hasFocus(),
  };
})()`;

/** The OS foreground window: DOM focus alone cannot prove the source window
 *  got activation back, and vice versa. */
function foregroundWindow() {
  const out = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", WINDOW_HELPER, "-Foreground"],
    { encoding: "utf8" }
  );
  const text = (out.stdout ?? "").trim();
  return text ? JSON.parse(text) : { title: null, hwnd: 0, pid: 0 };
}

function nativeWindows(titleLike) {
  const out = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      WINDOW_HELPER,
      "-TitleLike",
      titleLike,
    ],
    { encoding: "utf8" }
  );
  const text = (out.stdout ?? "").trim();
  if (!text) return [];
  const parsed = JSON.parse(text);
  return Array.isArray(parsed) ? parsed : [parsed];
}

function resizeNativeWindow(width, height) {
  spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      WINDOW_HELPER,
      "-TitleLike",
      "Clip viewer",
      "-Width",
      String(width),
      "-Height",
      String(height),
    ],
    { encoding: "utf8" }
  );
}

/** Real left click at an absolute physical screen point. A low-level mouse
 *  hook only sees input that entered the system queue, so CDP cannot stand in
 *  for the outside-click check. */
function clickAt(x, y) {
  const out = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      WINDOW_HELPER,
      "-ClickX",
      String(x),
      "-ClickY",
      String(y),
    ],
    { encoding: "utf8" }
  );
  const text = (out.stdout ?? "").trim();
  return text ? JSON.parse(text) : { clicked: false };
}

/** What a click at this physical point would hit, reduced to its root window.
 *  Proves the "inside" click is aimed at the viewer and is not blocked by a
 *  window stacked above it. */
function hitTest(x, y) {
  const out = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      WINDOW_HELPER,
      "-HitX",
      String(x),
      "-HitY",
      String(y),
    ],
    { encoding: "utf8" }
  );
  const text = (out.stdout ?? "").trim();
  return text ? JSON.parse(text) : null;
}

/** Give a window the foreground without sending any mouse input: the path a
 *  click on an elevated window takes when UIPI blocks the hook (ticket 228).
 *  `exact` is needed to single out the main window ("Sprout") from the dock. */
function activateWindow(title, exact = false) {
  const args = [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    WINDOW_HELPER,
    "-Activate",
    "-TitleLike",
    title,
  ];
  if (exact) args.push("-ExactTitle");
  const out = spawnSync("powershell.exe", args, { encoding: "utf8" });
  const text = (out.stdout ?? "").trim();
  if (!text) return null;
  const parsed = JSON.parse(text);
  return Array.isArray(parsed) ? parsed[0] : parsed;
}

async function waitForViewerTarget(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const list = await targets(port);
    const viewer = list.find((t) => t.url.includes("clip-viewer"));
    if (viewer) return viewer;
    await sleep(200);
  }
  throw new Error("clip-viewer target never appeared");
}

async function viewerCount(port) {
  return (await targets(port)).filter((t) => t.url.includes("clip-viewer")).length;
}

async function waitForNoViewer(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await viewerCount(port)) === 0) return;
    await sleep(150);
  }
  throw new Error("clip-viewer target never closed");
}

function evaluate(checks, report) {
  return checks.map((c) => {
    report.push(c);
    console.log(`${c.ok ? "ok  " : "RED "} ${c.label}${c.detail ? ` — ${c.detail}` : ""}`);
    return c.ok;
  });
}

// ------------------------------- the run -----------------------------------

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const port = await pickFreePort(args.port);
  const bootTimeout = args.mode === "dev" ? 600_000 : 30_000;
  const report = [];
  const checks = [];

  console.log(`repro-clip-viewer mode=${args.mode} port=${port} data=${DATA_DIR} shots=${args.shots}`);

  killStraySprout();
  rmSync(DATA_DIR, { recursive: true, force: true });
  await sleep(1500);

  let child = launch(args.mode, port);
  let cdp = null;
  let viewer = null;
  let infraError = null;

  try {
    const tBoot = Date.now();
    const { cdp: mainCdp } = await connectMainWindow(port, bootTimeout);
    console.log(`boot: ${((Date.now() - tBoot) / 1000).toFixed(1)}s`);
    cdp = mainCdp;
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width: 1180,
      height: 820,
      deviceScaleFactor: 1,
      mobile: false,
    });

    // Large enough that fit lands below 100 % and 100 % overflows the stage,
    // so fit/100 %/panning all have room to move.
    const pngA = makePng(2400, 1800, 40).toString("base64");
    const pngB = makePng(1800, 2400, 120).toString("base64");
    const seeded = await cdp.eval(SEED(pngA, pngB));
    console.log(`seeded: ${JSON.stringify(seeded)}`);

    const displays = await cdp.eval(`window.__TAURI_INTERNALS__.invoke("list_displays")`);
    console.log(`displays: ${JSON.stringify(displays)}`);
    checks.push({
      ok: Array.isArray(displays) && displays.length >= 1,
      label: "display inventory read",
      detail: `${displays?.length ?? 0} display(s)`,
    });

    // The Clips page owns the rows under test.
    await click(cdp, `document.querySelector('a[href="/clips"]')`, "Clips rail link");
    await waitFor(cdp, `!!document.querySelector(".rack__row")`, 20_000, "clips rows");
    await sleep(300);

    const rowExpression = (name) =>
      `[...document.querySelectorAll(".rack__row")].find((li) => li.textContent.includes(${JSON.stringify(
        name
      )}))`;

    // 1. A text Clip's right-click keeps the existing (no-viewer) behavior.
    await rightClick(cdp, rowExpression("Text probe"), "text row");
    await sleep(800);
    // The text path leaves the browser's own menu alone; dismiss it if the
    // host drew one so it cannot swallow the next keystroke.
    await pressKey(cdp, "Escape", "Escape", 27);
    await sleep(300);
    checks.push({
      ok: (await viewerCount(port)) === 0,
      label: "text Clip right-click opens no viewer",
      detail: `${await viewerCount(port)} viewer target(s)`,
    });

    // 2. Shift+F10 on an image row opens the row menu, never the viewer, and
    //    "Open image" is its first item. WebView2 may keep the browser's own
    //    Shift+F10 handling, so fall back to the same contextmenu event with
    //    keyboard semantics (button 0, no cursor position).
    await cdp.eval(`(() => {
      const row = ${rowExpression("Viewer probe")};
      row?.querySelector(".rack__main")?.focus();
      return true;
    })()`);
    await pressKey(cdp, "F10", "F10", 121, 8); // Shift+F10
    let keyboardPath = "real Shift+F10";
    try {
      await waitFor(cdp, `!!document.querySelector(".ctx-menu")`, 2500, "row menu via Shift+F10");
    } catch {
      keyboardPath = "synthetic keyboard contextmenu";
      await cdp.eval(`(() => {
        const row = ${rowExpression("Viewer probe")};
        const button = row?.querySelector(".rack__main");
        button?.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 0 }));
        return true;
      })()`);
      await waitFor(cdp, `!!document.querySelector(".ctx-menu")`, 5000, "row menu via keyboard contextmenu");
    }
    const menu = await cdp.eval(`(() => {
      const menu = document.querySelector(".ctx-menu");
      const items = [...menu.querySelectorAll(".ctx-item")].map((b) => b.textContent.trim());
      return { items, first: items[0] ?? null, label: menu.getAttribute("aria-label") };
    })()`);
    checks.push({
      ok: (await viewerCount(port)) === 0 && menu.first === "Open image",
      label: "Shift+F10 opens the row menu with Open image first, never the viewer",
      detail: `${keyboardPath}; first=${menu.first} viewers=${await viewerCount(port)}`,
    });
    await pressKey(cdp, "Escape", "Escape", 27);
    await sleep(200);

    // 3. Mouse right-click on the image row opens the viewer with the Clip.
    await rightClick(cdp, rowExpression("Viewer probe"), "image row");
    const viewerTarget = await waitForViewerTarget(port, 20_000);
    viewer = await attach(viewerTarget);
    await waitFor(
      viewer,
      `!!document.querySelector("img.viewer__image") || !!document.querySelector(".empty")`,
      20_000,
      "viewer content"
    );
    // The harness process can own the OS foreground, leaving the viewer
    // stacked behind the terminal; rendering and input routing must be tested
    // with the viewer actually in front, as it is for a user.
    activateWindow("Clip viewer");
    await sleep(400);
    const probe = await viewer.eval(VIEWER_PROBE);
    console.log(`viewer: ${JSON.stringify(probe)}`);
    await capture(viewer, args.shots, "viewer-main.png");
    await capture(cdp, args.shots, "main-row.png");
    checks.push({
      ok: probe.title === "Viewer probe" && probe.imageSrc === "data:image/png;base64," && !!probe.meta,
      label: "main right-click opens the viewer with the image, name, and summary",
      detail: `${probe.title} | ${probe.meta} | ${probe.imageSrc}`,
    });
    checks.push({
      ok:
        probe.copyLabel === "Copy Viewer probe to the clipboard" &&
        probe.copyText === "Copy" &&
        probe.closeLabel === "Close viewer",
      label: "Copy and close controls are labeled",
      detail: `copy=${probe.copyLabel} text=${probe.copyText} close=${probe.closeLabel}`,
    });
    checks.push({
      ok: probe.dragRegion && probe.imageAlt === "Preview of Viewer probe",
      label: "frameless chrome carries a drag region and image alt text",
      detail: `drag=${probe.dragRegion} alt=${probe.imageAlt}`,
    });

    // 3b. Zoom and pan on the real window.
    const zoomBase = await viewer.eval(ZOOM_PROBE);
    const zoomLabels = zoomBase.labels ?? [];
    checks.push({
      ok:
        ["Zoom out", "Zoom in", "Fit", "Actual size (100%)", "Reset zoom"].every(
          (label) => zoomLabels.includes(label)
        ) &&
        zoomBase.live === "polite" &&
        /^\d+%$/.test(zoomBase.percent ?? ""),
      label: "zoom controls are labeled buttons with an announced percentage",
      detail: `readout=${zoomBase.percent} live=${zoomBase.live} labels=${zoomLabels.filter(Boolean).join("|")}`,
    });
    const fitPercent = Number((zoomBase.percent ?? "").replace("%", ""));
    checks.push({
      ok: fitPercent > 25 && fitPercent < 100,
      label: "opens fitted inside the 25–800 % range",
      detail: `${zoomBase.percent} in ${zoomBase.inner.w}x${zoomBase.inner.h} CSS at dpr=${zoomBase.dpr}`,
    });

    await pressKey(viewer, "1", "Digit1", 49);
    await sleep(600);
    let zoomState = await viewer.eval(ZOOM_PROBE);
    checks.push({
      ok: zoomState.percent === "100%",
      label: "1 goes to 100 %",
      detail: `${zoomBase.percent} → ${zoomState.percent}`,
    });
    checks.push({
      ok: Math.abs(zoomState.image.width * zoomState.dpr - 2400) <= 2,
      label: "100 % is one image pixel per physical pixel at the real DPI",
      detail: `${zoomState.image.width.toFixed(1)} CSS × ${zoomState.dpr} = ${(zoomState.image.width * zoomState.dpr).toFixed(1)} physical for a 2400 px image`,
    });

    // Ctrl+wheel at an off-centre cursor: the image point under it stays put.
    const anchor = {
      x: Math.round(zoomState.inner.w * 0.35),
      y: Math.round(zoomState.inner.h * 0.45),
    };
    const fractionUnder = (rect) => (anchor.x - rect.left) / rect.width;
    const beforeFraction = fractionUnder(zoomState.image);
    await wheelAt(viewer, { ...anchor, deltaY: -120, modifiers: 2 });
    await sleep(600);
    zoomState = await viewer.eval(ZOOM_PROBE);
    const wheeledPercent = Number((zoomState.percent ?? "").replace("%", ""));
    checks.push({
      ok:
        wheeledPercent > 100 &&
        Math.abs(fractionUnder(zoomState.image) - beforeFraction) < 0.02,
      label: "Ctrl+wheel zooms in at the cursor and holds the anchored pixel",
      detail: `${wheeledPercent}% anchored fraction ${beforeFraction.toFixed(3)} → ${fractionUnder(zoomState.image).toFixed(3)}`,
    });

    // A trackpad pinch arrives as fine-grained ctrl+wheel deltas.
    await pressKey(viewer, "0", "Digit0", 48);
    await sleep(300);
    const fineBase = Number(
      ((await viewer.eval(ZOOM_PROBE)).percent ?? "").replace("%", "")
    );
    await wheelAt(viewer, { ...anchor, deltaY: -20, modifiers: 2 });
    await sleep(300);
    const fineAfter = Number(
      ((await viewer.eval(ZOOM_PROBE)).percent ?? "").replace("%", "")
    );
    checks.push({
      ok: fineAfter > fineBase && fineAfter - fineBase <= 5,
      label: "fine-grained ctrl+wheel (trackpad pinch) makes a small zoom step",
      detail: `${fineBase}% → ${fineAfter}% for deltaY −20`,
    });

    // Wheel panning at 100 %, where the image overflows both axes.
    await pressKey(viewer, "1", "Digit1", 49);
    await sleep(300);
    let panBase = await viewer.eval(ZOOM_PROBE);
    const wheelPoint = {
      x: Math.round(panBase.inner.w / 2),
      y: Math.round(panBase.inner.h * 0.45),
    };
    await wheelAt(viewer, { ...wheelPoint, deltaY: 120 });
    await sleep(300);
    let panned = await viewer.eval(ZOOM_PROBE);
    checks.push({
      ok: Math.abs(panned.image.top - (panBase.image.top - 120)) <= 2,
      label: "plain wheel pans vertically",
      detail: `top ${panBase.image.top.toFixed(1)} → ${panned.image.top.toFixed(1)}`,
    });

    panBase = panned;
    await wheelAt(viewer, { ...wheelPoint, deltaY: 120, modifiers: 8 });
    await sleep(300);
    panned = await viewer.eval(ZOOM_PROBE);
    checks.push({
      ok: Math.abs(panned.image.left - (panBase.image.left - 120)) <= 2,
      label: "Shift+wheel pans horizontally",
      detail: `left ${panBase.image.left.toFixed(1)} → ${panned.image.left.toFixed(1)}`,
    });

    // Arrow-key panning.
    panBase = panned;
    await pressKey(viewer, "ArrowLeft", "ArrowLeft", 37);
    await sleep(300);
    panned = await viewer.eval(ZOOM_PROBE);
    checks.push({
      ok: Math.abs(panned.image.left - (panBase.image.left - 48)) <= 2,
      label: "arrow keys pan a zoomed image",
      detail: `left ${panBase.image.left.toFixed(1)} → ${panned.image.left.toFixed(1)}`,
    });

    // Drag-pan through real pointer events.
    panBase = panned;
    await dragMouse(
      viewer,
      { x: wheelPoint.x, y: wheelPoint.y },
      { x: wheelPoint.x - 60, y: wheelPoint.y - 40 }
    );
    await sleep(300);
    panned = await viewer.eval(ZOOM_PROBE);
    checks.push({
      ok:
        Math.abs(panned.image.left - (panBase.image.left - 60)) <= 3 &&
        Math.abs(panned.image.top - (panBase.image.top - 40)) <= 3,
      label: "drag-pan moves the image with the pointer",
      detail: `left ${panBase.image.left.toFixed(1)} → ${panned.image.left.toFixed(1)}, top ${panBase.image.top.toFixed(1)} → ${panned.image.top.toFixed(1)}`,
    });

    // Ctrl+0 resets to fit and centre.
    await pressKey(viewer, "0", "Digit0", 48, 2);
    await sleep(350);
    const reset = await viewer.eval(ZOOM_PROBE);
    checks.push({
      ok:
        reset.percent === zoomBase.percent &&
        Math.abs(reset.image.left + reset.image.width / 2 - reset.inner.w / 2) <= 2,
      label: "Ctrl+0 resets to fit and centres",
      detail: `${reset.percent}, centre ${(reset.image.left + reset.image.width / 2).toFixed(1)} vs ${(reset.inner.w / 2).toFixed(1)}`,
    });

    // Double-click toggles fit ↔ 100 % at the cursor.
    const togglePoint = {
      x: Math.round(reset.inner.w / 2),
      y: Math.round(reset.inner.h * 0.45),
    };
    await doubleClickAt(viewer, togglePoint.x, togglePoint.y);
    await sleep(350);
    const toggledIn = await viewer.eval(ZOOM_PROBE);
    await doubleClickAt(viewer, togglePoint.x, togglePoint.y);
    await sleep(350);
    const toggledOut = await viewer.eval(ZOOM_PROBE);
    checks.push({
      ok: toggledIn.percent === "100%" && toggledOut.percent === zoomBase.percent,
      label: "double-click toggles fit ↔ 100 %",
      detail: `${zoomBase.percent} → ${toggledIn.percent} → ${toggledOut.percent}`,
    });

    // The released range clamps at both ends.
    await viewer.eval(`(() => {
      const out = [...document.querySelectorAll("button")].find((b) => b.getAttribute("aria-label") === "Zoom out");
      for (let i = 0; i < 30; i++) out.click();
      return true;
    })()`);
    await sleep(400);
    const floor = (await viewer.eval(ZOOM_PROBE)).percent;
    await viewer.eval(`(() => {
      const inBtn = [...document.querySelectorAll("button")].find((b) => b.getAttribute("aria-label") === "Zoom in");
      for (let i = 0; i < 60; i++) inBtn.click();
      return true;
    })()`);
    await sleep(400);
    const ceiling = (await viewer.eval(ZOOM_PROBE)).percent;
    checks.push({
      ok: floor === "25%" && ceiling === "800%",
      label: "zoom clamps at 25 % and 800 %",
      detail: `${floor} … ${ceiling}`,
    });
    await pressKey(viewer, "0", "Digit0", 48);
    await sleep(300);

    // Reduced motion removes the zoom transition but not the controls.
    await viewer.send("Emulation.setEmulatedMedia", {
      features: [{ name: "prefers-reduced-motion", value: "reduce" }],
    });
    await sleep(250);
    const motionBefore = await viewer.eval(ZOOM_PROBE);
    const motion = await viewer.eval(`(() => {
      const img = document.querySelector("img.viewer__image");
      return getComputedStyle(img).transitionDuration;
    })()`);
    // Chrome serializes tiny durations in seconds ("1e-05s" for 0.01ms), so
    // compare the parsed value against "not perceptible" rather than a string.
    const durationSeconds = (value) =>
      Math.max(
        ...String(value)
          .split(",")
          .map((part) => {
            const m = part.trim().match(/^([\d.eE+-]+)(ms|s)$/);
            return m ? parseFloat(m[1]) * (m[2] === "ms" ? 1e-3 : 1) : NaN;
          })
      );
    await viewer.eval(`(() => {
      [...document.querySelectorAll("button")].find((b) => b.getAttribute("aria-label") === "Zoom in").click();
      return true;
    })()`);
    await sleep(300);
    const motionAfter = await viewer.eval(ZOOM_PROBE);
    checks.push({
      ok:
        durationSeconds(motion) <= 0.001 &&
        motionAfter.percent !== motionBefore.percent,
      label: "reduced motion drops the zoom animation but keeps the controls",
      detail: `transition=${motion} (${(durationSeconds(motion) * 1000).toFixed(3)}ms) ${motionBefore.percent} → ${motionAfter.percent}`,
    });
    await viewer.send("Emulation.setEmulatedMedia", { features: [] });
    await pressKey(viewer, "0", "Digit0", 48);
    await sleep(250);

    // 100 % stays one image pixel per physical pixel across 100/150/200 % DPI.
    // A real scaling change keeps the window's physical size and changes its
    // CSS size, so the page sees a resize *and* a new devicePixelRatio — the
    // pair the viewer re-measures. Emulating only deviceScaleFactor at a fixed
    // CSS size never fires that resize, so it would test nothing.
    const dprViewport = await viewer.eval(
      `({ w: innerWidth, h: innerHeight, dpr: devicePixelRatio })`
    );
    const physical = {
      w: Math.round(dprViewport.w * dprViewport.dpr),
      h: Math.round(dprViewport.h * dprViewport.dpr),
    };
    const physicalByDpr = {};
    for (const dpr of [1, 1.5, 2]) {
      await viewer.send("Emulation.setDeviceMetricsOverride", {
        width: Math.round(physical.w / dpr),
        height: Math.round(physical.h / dpr),
        deviceScaleFactor: dpr,
        mobile: false,
      });
      await sleep(350);
      await pressKey(viewer, "1", "Digit1", 49);
      await sleep(300);
      const atDpr = await viewer.eval(ZOOM_PROBE);
      physicalByDpr[dpr] = {
        physical: atDpr.image.width * atDpr.dpr,
        css: atDpr.image.width,
        stage: `${atDpr.inner.w}x${atDpr.inner.h}`,
      };
    }
    await viewer.send("Emulation.clearDeviceMetricsOverride");
    await sleep(300);
    checks.push({
      ok: [1, 1.5, 2].every(
        (dpr) => Math.abs(physicalByDpr[dpr].physical - 2400) <= 3
      ),
      label: "100 % stays one image pixel per physical pixel at 100/150/200 % DPI",
      detail: [1, 1.5, 2]
        .map(
          (dpr) =>
            `${dpr * 100}%=${physicalByDpr[dpr].physical.toFixed(1)}px ` +
            `(${physicalByDpr[dpr].css.toFixed(0)} CSS @ ${physicalByDpr[dpr].stage})`
        )
        .join(" "),
    });
    await pressKey(viewer, "0", "Digit0", 48);
    await sleep(300);

    // The window floor is 480x360 physical — 320 CSS wide at 150 % scaling,
    // which is narrower than one row of labeled controls. Nothing may clip.
    await viewer.send("Emulation.setDeviceMetricsOverride", {
      width: 320,
      height: 240,
      deviceScaleFactor: 1.5,
      mobile: false,
    });
    await sleep(400);
    const narrow = await viewer.eval(`(() => {
      const foot = document.querySelector(".viewer__foot");
      const stage = document.querySelector(".viewer__stage");
      const actions = document.querySelector(".viewer__actions");
      const stepper = document.querySelector(".viewer__stepper");
      const presets = document.querySelector(".viewer__presets");
      const buttons = [...foot.querySelectorAll("button")];
      const zoomButtons = [
        ...foot.querySelectorAll(".viewer__stepper button, .viewer__presets button"),
      ];
      const vw = innerWidth;
      const w = (el) => (el ? Math.round(el.getBoundingClientRect().width) : 0);
      const tops = zoomButtons
        .map((b) => b.getBoundingClientRect().top)
        .sort((a, b) => a - b);
      // Buttons in one row differ by a pixel or two (33px text buttons vs
      // 30px icon buttons, both centred); rows are >10px apart.
      const rows = tops.filter((t, i) => i === 0 || t - tops[i - 1] > 10).length;
      const clipped = buttons
        .filter((b) => {
          const r = b.getBoundingClientRect();
          return r.left < -0.5 || r.right > vw + 0.5;
        })
        .map((b) => b.getAttribute("aria-label") || b.textContent.trim());
      return {
        vw,
        rows,
        clipped,
        overflow: foot.scrollWidth - foot.clientWidth,
        foot: Math.round(foot.getBoundingClientRect().height),
        stage: Math.round(stage.getBoundingClientRect().height),
        widths: {
          actions: w(actions),
          stepper: w(stepper),
          presets: w(presets),
          copy: w(buttons.at(-1)),
        },
      };
    })()`);
    await capture(viewer, args.shots, "viewer-narrow.png");
    await viewer.send("Emulation.clearDeviceMetricsOverride");
    await sleep(300);
    checks.push({
      ok:
        narrow.clipped.length === 0 &&
        narrow.overflow <= 1 &&
        narrow.rows <= 2 &&
        narrow.stage >= 80,
      label: "narrow window folds the toolbar to two rows and keeps the image",
      detail:
        `${narrow.vw} CSS, rows=${narrow.rows}, overflow=${narrow.overflow}, ` +
        `clipped=${narrow.clipped.join(", ") || "none"}, ` +
        `foot=${narrow.foot}px, stage=${narrow.stage}px, ` +
        `stepper=${narrow.widths.stepper}px presets=${narrow.widths.presets}px ` +
        `copy=${narrow.widths.copy}px actions=${narrow.widths.actions}px`,
    });

    // 4. Native window style: visible, not always-on-top.
    const native = nativeWindows("Sprout");
    const viewerWindow = native.find((w) => (w.title ?? "").includes("Clip viewer"));
    checks.push({
      ok: !!viewerWindow && viewerWindow.topmost === false,
      label: "viewer window is visible and not always-on-top",
      detail: viewerWindow
        ? `title=${viewerWindow.title} topmost=${viewerWindow.topmost} rect=${viewerWindow.left},${viewerWindow.top} ${viewerWindow.width}x${viewerWindow.height}`
        : "viewer window not found",
    });

    // 5. Placement: centered on the monitor that owns the invoking window,
    //    inside that monitor's bounds.
    const mainWindow = native.find((w) => w.title === "Sprout");
    if (viewerWindow && mainWindow) {
      const mainCx = (mainWindow.left + mainWindow.right) / 2;
      const mainCy = (mainWindow.top + mainWindow.bottom) / 2;
      const owner =
        displays.find(
          (d) =>
            mainCx >= d.x &&
            mainCx <= d.x + d.width &&
            mainCy >= d.y &&
            mainCy <= d.y + d.height
        ) ?? displays[0];
      const viewerCx = (viewerWindow.left + viewerWindow.right) / 2;
      const viewerCy = (viewerWindow.top + viewerWindow.bottom) / 2;
      const displayCx = owner.x + owner.width / 2;
      const displayCy = owner.y + owner.height / 2;
      const inside =
        viewerWindow.left >= owner.x &&
        viewerWindow.top >= owner.y &&
        viewerWindow.right <= owner.x + owner.width &&
        viewerWindow.bottom <= owner.y + owner.height;
      checks.push({
        ok: inside && Math.abs(viewerCx - displayCx) <= 2 && Math.abs(viewerCy - displayCy) <= 80,
        label: "viewer is centered inside the invoking window's monitor",
        detail: `owner=${owner.label} display-center=${displayCx},${displayCy} viewer-center=${viewerCx},${viewerCy} inside=${inside}`,
      });
    } else {
      checks.push({ ok: false, label: "placement probe found both windows", detail: "missing window" });
    }

    // 6. Copy shows the row's Copied feedback and the live region announces.
    await click(viewer, `[...document.querySelectorAll("button")].find((b) => (b.getAttribute("aria-label") ?? "").startsWith("Copy "))`, "viewer Copy");
    await waitFor(viewer, `!!document.querySelector('[role="status"]')?.textContent?.trim()`, 10_000, "copied live region");
    await sleep(200);
    const copied = await viewer.eval(VIEWER_PROBE);
    await capture(viewer, args.shots, "viewer-copied.png");
    checks.push({
      ok: copied.copyText === "Copied" && copied.live === "Viewer probe copied.",
      label: "Copy flashes Copied and announces it like the row",
      detail: `button=${copied.copyText} live=${copied.live}`,
    });

    // 7. Size memory: resize the native window, close with Esc, reopen from
    //    the row menu, and read the restored physical size back.
    resizeNativeWindow(700, 500);
    await sleep(700);
    const resized = await viewer.eval(`({ w: innerWidth * devicePixelRatio, h: innerHeight * devicePixelRatio })`);
    await pressKey(viewer, "Escape", "Escape", 27);
    await waitForNoViewer(port, 10_000);
    await sleep(400);
    checks.push({
      ok: true,
      label: "Esc closes the viewer",
      detail: `resized to ${resized.w}x${resized.h} physical before closing`,
    });

    await click(cdp, `[...document.querySelectorAll(".rack__row")].find((li) => li.textContent.includes("Viewer probe"))?.querySelector("[data-ctx-trigger]")`, "row menu trigger");
    await waitFor(cdp, `!!document.querySelector(".ctx-menu")`, 5000, "row menu");
    await click(cdp, `[...document.querySelectorAll(".ctx-menu .ctx-item")][0]`, "Open image item");
    const reopened = await attach(await waitForViewerTarget(port, 20_000));
    viewer?.close();
    viewer = reopened;
    await waitFor(viewer, `!!document.querySelector("img.viewer__image")`, 20_000, "reopened viewer");
    await sleep(300);
    const restored = await viewer.eval(`({ w: innerWidth * devicePixelRatio, h: innerHeight * devicePixelRatio })`);
    checks.push({
      ok: Math.abs(restored.w - resized.w) <= 2 && Math.abs(restored.h - resized.h) <= 2,
      label: "row menu's Open image reopens at the remembered size",
      detail: `before=${resized.w}x${resized.h} after=${restored.w}x${restored.h}`,
    });

    // 8. Focus returns to the source window after the close control — both at
    //    the OS level (it is the foreground window) and inside the page (its
    //    WebView2 child holds the keyboard, so the next keystroke lands).
    await click(viewer, `document.querySelector('button[aria-label="Close viewer"]')`, "close control");
    await waitForNoViewer(port, 10_000);
    await sleep(500);
    const mainFocused = await cdp.eval(`document.hasFocus()`);
    const foreground = foregroundWindow();
    checks.push({
      ok: mainFocused && foreground.title === "Sprout",
      label: "close control closes and returns focus to the main window",
      detail: `main document.hasFocus()=${mainFocused} foreground=${foreground.title}`,
    });

    // 9. The dock's image row opens the same one viewer (retarget), and the
    //    second Clip replaces the first without a second window. The dock is
    //    tray-resident and only restores at boot when docked (ADR-0013), so
    //    persist "docked", restart the app, then undock back to floating — the
    //    row is driven in its ordinary floating state. (Opening the dock from
    //    an IPC command instead would deadlock WebView2 creation on the event
    //    thread, a pre-existing bug outside this ticket.)
    await cdp.eval(`(async () => {
      const inv = window.__TAURI_INTERNALS__.invoke;
      const settings = await inv("get_settings");
      await inv("update_settings", { settings: { ...settings, dock_state: "docked" } });
      return true;
    })()`);
    killTree(child);
    cdp.close();
    await sleep(1500);
    child = launch(args.mode, port);
    const restarted = await connectMainWindow(port, bootTimeout);
    cdp = restarted.cdp;
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width: 1180,
      height: 820,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await click(cdp, `document.querySelector('a[href="/clips"]')`, "Clips rail link");
    await waitFor(cdp, `!!document.querySelector(".rack__row")`, 20_000, "clips rows after restart");
    const quickTarget = (await targets(port)).find((t) => t.url.includes("quick-launch-window"));
    if (!quickTarget) throw new Error("Quick Launch target not found after restart");
    const dock = await attach(quickTarget);
    await waitFor(dock, `!!document.querySelector("#tab-clips")`, 20_000, "dock page");
    await dock.eval(
      `window.__TAURI_INTERNALS__.invoke("toggle_quick_launch_dock").then(() => true)`
    );
    await sleep(800);
    await click(dock, `document.querySelector("#tab-clips")`, "dock Clips tab");
    await waitFor(dock, `!!document.querySelector(".dock-row")`, 10_000, "dock clip rows");
    await sleep(300);
    await rightClick(
      dock,
      `[...document.querySelectorAll(".dock-row")].find((li) => li.textContent.includes("Dock probe"))`,
      "dock image row"
    );
    const dockViewer = await attach(await waitForViewerTarget(port, 20_000));
    viewer?.close();
    viewer = dockViewer;
    await waitFor(viewer, `document.querySelector("h1")?.textContent?.includes("Dock probe")`, 20_000, "dock retarget");
    await sleep(300);
    const dockProbe = await viewer.eval(VIEWER_PROBE);
    await capture(viewer, args.shots, "viewer-dock.png");
    await capture(dock, args.shots, "dock-row.png");
    checks.push({
      ok: (await viewerCount(port)) === 1 && dockProbe.title === "Dock probe",
      label: "dock right-click retargets the one viewer to the dock's Clip",
      detail: `viewers=${await viewerCount(port)} title=${dockProbe.title}`,
    });

    // 10. A Clip deleted while the viewer shows it becomes the empty state.
    await cdp.eval(`window.__TAURI_INTERNALS__.invoke("delete_clip", { id: ${seeded.second} })`);
    await viewer.eval(`window.dispatchEvent(new Event("focus")); true`);
    await waitFor(viewer, `!!document.querySelector(".empty__title")`, 10_000, "missing Clip empty state");
    const missing = await viewer.eval(VIEWER_PROBE);
    await capture(viewer, args.shots, "viewer-missing.png");
    checks.push({
      ok: missing.emptyTitle === "This clip is gone" && (missing.emptyBody ?? "").includes("Press Esc"),
      label: "deleted Clip shows the friendly empty state",
      detail: `${missing.emptyTitle} | ${missing.emptyBody}`,
    });
    await pressKey(viewer, "Escape", "Escape", 27);
    await waitForNoViewer(port, 10_000);
    checks.push({ ok: true, label: "Esc closes the empty state", detail: "viewer gone" });

    // 11. Outside-click hook (ticket 228). CDP's Input domain never enters the
    //     system queue, so these checks send real mouse_event input at absolute
    //     screen coordinates and read native rects; opening by command keeps
    //     the page state irrelevant. Every wait skips the 200 ms open grace.
    //     The floating dock is always-on-top and can cover the viewer, so the
    //     checks close it first: they test the viewer's input routing, not how
    //     two always-on-top windows stack.
    await cdp
      .eval(
        `window.__TAURI_INTERNALS__.invoke("close_quick_launch_window").catch(() => "already closed")`
      )
      .catch(() => {});
    await sleep(900);
    const openFromMain = async () => {
      await cdp.eval(
        `window.__TAURI_INTERNALS__.invoke("open_clip_viewer", { id: ${seeded.first}, source: "main" })`
      );
      const target = await attach(await waitForViewerTarget(port, 20_000));
      viewer?.close();
      viewer = target;
      await waitFor(viewer, `!!document.querySelector("img.viewer__image")`, 20_000, "viewer for hook");
      activateWindow("Clip viewer");
      await sleep(650);
    };
    await cdp.eval(`(() => {
      window.__outsideClicks = 0;
      document.addEventListener("pointerdown", () => { window.__outsideClicks += 1; }, true);
      return true;
    })()`);

    await openFromMain();
    const hookWindows = nativeWindows("Sprout");
    const hookViewer = hookWindows.find((w) => (w.title ?? "").includes("Clip viewer"));
    const hookMain = hookWindows.find((w) => w.title === "Sprout");
    if (!hookViewer || !hookMain) {
      throw new Error("outside-click probe could not find the viewer and main windows");
    }
    // The viewer is centred over the main window; only a strip of the main
    // window is exposed. Try the rail edges, then the top strip above it, and
    // pick the first point whose root window really is the main window.
    const outsideCandidates = [
      { x: hookMain.left + 24, y: Math.round((hookMain.top + hookMain.bottom) / 2) },
      { x: hookMain.right - 24, y: Math.round((hookMain.top + hookMain.bottom) / 2) },
      { x: Math.round((hookMain.left + hookMain.right) / 2), y: hookMain.top + 10 },
    ].filter(
      (p) =>
        !(p.x >= hookViewer.left && p.x <= hookViewer.right && p.y >= hookViewer.top && p.y <= hookViewer.bottom)
    );
    let outside = null;
    let outsideHit = null;
    for (const point of outsideCandidates) {
      const hit = hitTest(point.x, point.y);
      if (hit?.root === hookMain.hwnd) {
        outside = point;
        outsideHit = hit;
        break;
      }
    }
    if (!outside) throw new Error("no exposed main-window point outside the viewer");
    await cdp.eval(`window.__outsideClicks = 0; true`);
    const outsideClick = clickAt(outside.x, outside.y);
    let outsideClosed = false;
    try {
      await waitForNoViewer(port, 8000);
      outsideClosed = true;
    } catch {}
    await sleep(500);
    const outsideForeground = foregroundWindow();
    const outsideDelivered = await cdp.eval(`window.__outsideClicks ?? 0`);
    checks.push({
      ok: outsideClosed && outsideDelivered >= 1 && outsideForeground.title === "Sprout",
      label: "a real click outside closes the viewer and still reaches the window underneath",
      detail: `click=${outsideClick.clicked} at ${outside.x},${outside.y} target=${outsideHit?.rootTitle}/${outsideHit?.root} main=${hookMain.hwnd} closed=${outsideClosed} main-pointerdown=${outsideDelivered} foreground=${outsideForeground.title}`,
    });

    await openFromMain();
    const insideWindows = nativeWindows("Sprout");
    const insideViewer = insideWindows.find((w) => (w.title ?? "").includes("Clip viewer"));
    if (!insideViewer) throw new Error("inside-click probe could not find the viewer window");
    // The floating dock is always-on-top and can overlap the viewer's centre,
    // so pick a point whose root window really is the viewer instead of
    // assuming the centre is visible.
    const insideCandidates = [
      { fx: 0.08, fy: 0.45 },
      { fx: 0.15, fy: 0.45 },
      { fx: 0.3, fy: 0.45 },
      { fx: 0.5, fy: 0.45 },
      { fx: 0.25, fy: 0.6 },
      { fx: 0.5, fy: 0.6 },
    ].map(({ fx, fy }) => ({
      x: Math.round(insideViewer.left + (insideViewer.right - insideViewer.left) * fx),
      y: Math.round(insideViewer.top + (insideViewer.bottom - insideViewer.top) * fy),
    }));
    let insidePoint = null;
    let insideHit = null;
    for (const point of insideCandidates) {
      const hit = hitTest(point.x, point.y);
      if (hit?.root === insideViewer.hwnd) {
        insidePoint = point;
        insideHit = hit;
        break;
      }
    }
    if (!insidePoint) {
      const seen = insideCandidates
        .map((p) => `${p.x},${p.y}=${hitTest(p.x, p.y)?.rootTitle ?? "?"}`)
        .join(" ");
      throw new Error(`no visible point inside the viewer to click (${seen})`);
    }
    const insideForeground = foregroundWindow();
    const insideClick = clickAt(insidePoint.x, insidePoint.y);
    await sleep(900);
    checks.push({
      ok: (await viewerCount(port)) === 1,
      label: "a real click inside the viewer leaves it open",
      detail: `click=${insideClick.clicked} at ${insidePoint.x},${insidePoint.y} root=${insideHit.rootTitle}/${insideHit.root} viewer=${insideViewer.hwnd} foreground=${insideForeground.title} viewers=${await viewerCount(port)}`,
    });

    // Focus-loss fallback: activation without any mouse input is the same path
    // a click on an elevated (UIPI-blocked) foreground window takes.
    const activation = activateWindow("Sprout", true);
    let fallbackClosed = false;
    try {
      await waitForNoViewer(port, 8000);
      fallbackClosed = true;
    } catch {}
    await sleep(300);
    checks.push({
      ok: fallbackClosed,
      label: "losing the foreground to another window closes the viewer (UIPI fallback path)",
      detail: `activated=${activation?.activated ?? "?"} foregroundAfter=${activation?.foregroundAfter ?? "?"} title=${activation?.title ?? "?"} closed=${fallbackClosed}`,
    });
    dock.close();
  } catch (e) {
    infraError = e;
  } finally {
    const red = evaluate(checks, report).some((ok) => !ok);
    if (infraError) {
      console.error(`INFRA ERROR: ${infraError.message}`);
      if (child.tail) console.error(child.tail.slice(-10).join(""));
      process.exitCode = 2;
    } else {
      mkdirSync(args.shots, { recursive: true });
      writeFileSync(path.join(args.shots, "report.json"), JSON.stringify(report, null, 2));
      console.log(red ? "VERDICT: RED" : "VERDICT: GREEN");
      process.exitCode = red ? 1 : 0;
    }

    viewer?.close();
    cdp?.close();
    if (!args.keep) {
      killTree(child);
      await sleep(500);
    }
    await sleep(100);
    process.exit(process.exitCode ?? 0);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(2);
});
