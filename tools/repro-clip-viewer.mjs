import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { setTimeout as sleep } from "node:timers/promises";

// Ticket 226 evidence: drives the real app through CDP and checks the clip
// viewer shell end to end — right-click from the main app and the dock, the
// row menu's first item, one window reused across opens, monitor-aware
// placement, remembered size, copy feedback, the missing-Clip empty state,
// Esc/close-control dismissal, focus return, and the native window style
// (visible, not always-on-top). Runs against a throwaway LOCALAPPDATA so it
// never touches the user's Library. Exit: 0 green, 1 red, 2 infra.

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TEMP_ROOT = path.join(os.tmpdir(), "opencode");
const DATA_DIR = path.join(TEMP_ROOT, "sprout-226-appdata");
const WINDOW_HELPER = path.join(ROOT, "tools", "repro-clip-viewer-windows.ps1");

const USAGE = `Usage: node tools/repro-clip-viewer.mjs [options]

Seeds image Clips in a throwaway app-data directory and checks the clip
viewer: main/dock right-click, menu item, reuse, placement, size memory,
copy feedback, missing Clip, Esc/close, focus return, native window style.

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

    const pngA = makePng(64, 48, 40).toString("base64");
    const pngB = makePng(40, 64, 120).toString("base64");
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
