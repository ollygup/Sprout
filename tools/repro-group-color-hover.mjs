import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

// Ticket 240/241 evidence: drives the real app through CDP, seeds a colored
// and a neutral group in all three collections, and verifies the rendered
// anatomy in both themes - one full-height 3 px bar, the Oklab per-card tint on
// a plain page surface, no colored outline, the hue-carrying row hover, and the
// AA text rule on the tinted hover. It then restarts docked and repeats the checks in the Quick
// Launch strip, where the same anatomy renders in its flush variant. Runs
// against a throwaway LOCALAPPDATA so it never touches the user's Library.
// Exit: 0 green, 1 red, 2 infra.

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TEMP_ROOT = path.join(os.tmpdir(), "opencode");
const DATA_DIR = path.join(TEMP_ROOT, "sprout-240-appdata");

const HUE = {
  "Work": "#417ed4",
  "Deploy": "#1c9139",
  "Snippets": "#e03a8d",
};

const PAGES = [
  { href: "/", marker: ".launch", name: "launch", interactive: false },
  { href: "/quick-actions", marker: ".qa", name: "actions", interactive: true },
  { href: "/clips", marker: ".clips", name: "clips", interactive: true },
];

const USAGE = `Usage: node tools/repro-group-color-hover.mjs [options]

Seeds colored and neutral groups in a throwaway app-data directory and checks
the group anatomy and row hover in light and dark.

Options:
  --mode dev|exe   app under test (default dev)
  --port P         CDP port (default: first free of 9222,9333,9444,9555,9666)
  --shots DIR      save PNGs (default: %TEMP%\\opencode\\evidence-240)
  --keep           leave the app running after the verdict
  --help`;

function parseArgs(argv) {
  const args = {
    mode: "dev",
    port: null,
    shots: path.join(TEMP_ROOT, "evidence-240"),
    keep: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--help": console.log(USAGE); process.exit(0);
      case "--keep": args.keep = true; break;
      case "--shots": args.shots = argv[++i]; break;
      case "--mode": args.mode = argv[++i]; break;
      case "--port": args.port = Number(argv[++i]); break;
      default: throw new Error(`unknown option: ${a}\n${USAGE}`);
    }
  }
  return args;
}

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
      this.ws.addEventListener("error", () => reject(new Error("CDP websocket connect failed")), { once: true });
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
        "eval threw: " + (r.exceptionDetails.exception?.description ?? r.exceptionDetails.text),
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

// The dev server owns :1420. A restart must wait until the killed tree releases
// it: vite either fails outright or picks another port, and the app's fixed
// devUrl then waits forever for a window that never appears.
async function waitPortFree(port, timeoutMs = 25_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      await fetch(`http://127.0.0.1:${port}/`, { signal: AbortSignal.timeout(600) });
    } catch {
      return true;
    }
    await sleep(400);
  }
  return false;
}

// A restart also needs the previous WebView2 to let go of the debugging port,
// or the new window cannot expose a target at all.
async function waitDevServerFree(cdpPort) {
  const dev = await waitPortFree(1420);
  const debug = await waitPortFree(cdpPort, 10_000);
  return dev && debug;
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
      const list = await fetchJson(`http://127.0.0.1:${port}/json`);
      const pages = list.filter(
        (t) => t.type === "page" && t.url !== "about:blank" && !t.url.startsWith("devtools://"),
      );
      for (const target of pages) {
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
      if (pages.length > 0 && !asked) {
        asked = true;
        const opener = new Cdp(pages[0].webSocketDebuggerUrl);
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
      `JSON.stringify({ href: location.href, path: location.pathname, theme: document.documentElement.dataset.theme, text: document.body?.innerText?.slice(0, 200) ?? null })`,
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

// In-page probe. Colors are resolved through a canvas so every CSS color form
// (rgb(), color(srgb ...), oklab()) lands in the same sRGB numbers the gate
// compares against.
const PROBE_HELPERS = `
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 1;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  const resolve = (value) => {
    if (!value) return null;
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = "#010203";
    ctx.fillStyle = value;
    ctx.fillRect(0, 0, 1, 1);
    const d = ctx.getImageData(0, 0, 1, 1).data;
    return { r: d[0], g: d[1], b: d[2], a: Math.round((d[3] / 255) * 1000) / 1000 };
  };
  const root = getComputedStyle(document.documentElement);
`;

const MEASURE = `(() => {
  ${PROBE_HELPERS}
  const groups = [...document.querySelectorAll("section.group")].map((g) => {
    const label = g.querySelector(".disclosure__label");
    const rows = g.querySelector(".group__rows");
    const firstRow = rows?.querySelector(".rack__row");
    const muted = firstRow?.querySelector(".rack__content, .rack__command, .rack__target");
    const bar = getComputedStyle(g, "::before");
    const section = g.getBoundingClientRect();
    const rowRect = firstRow ? firstRow.getBoundingClientRect() : null;
    return {
      name: label?.textContent?.trim() ?? null,
      colored: g.classList.contains("group--colored"),
      barWidth: bar.width,
      barTop: bar.top,
      barBottom: bar.bottom,
      barHeight: bar.height,
      barColor: bar.backgroundColor,
      barBackground: resolve(bar.backgroundColor),
      sectionHeight: Math.round(section.height),
      titleX: Math.round(label.getBoundingClientRect().left),
      body: rows ? getComputedStyle(rows).backgroundColor : null,
      bodyRgb: rows ? resolve(getComputedStyle(rows).backgroundColor) : null,
      shadow: rows ? getComputedStyle(rows).boxShadow : null,
      rowBg: firstRow ? getComputedStyle(firstRow).backgroundColor : null,
      rowBgRgb: firstRow ? resolve(getComputedStyle(firstRow).backgroundColor) : null,
      rowBgVar: getComputedStyle(g).getPropertyValue("--group-row-bg").trim(),
      mutedColor: muted ? getComputedStyle(muted).color : null,
      mutedRgb: muted ? resolve(getComputedStyle(muted).color) : null,
      hoverVar: g.style.getPropertyValue("--group-row-hover").trim() || getComputedStyle(g).getPropertyValue("--group-row-hover").trim(),
      hoverTextVar: g.style.getPropertyValue("--group-row-hover-text").trim() || getComputedStyle(g).getPropertyValue("--group-row-hover-text").trim(),
      rowRect: rowRect ? { x: Math.round(rowRect.x), y: Math.round(rowRect.y), w: Math.round(rowRect.width), h: Math.round(rowRect.height) } : null,
    };
  });
  return {
    theme: document.documentElement.dataset.theme,
    page: getComputedStyle(document.body).backgroundColor,
    pageRgb: resolve(getComputedStyle(document.body).backgroundColor),
    bgHover: resolve(root.getPropertyValue("--bg-hover")),
    text: resolve(root.getPropertyValue("--text")),
    textMuted: resolve(root.getPropertyValue("--text-muted")),
    groups,
  };
})()`;

const HOVER = `(() => {
  ${PROBE_HELPERS}
  const row = document.querySelector(".rack__row:hover");
  if (!row) return { hovered: false };
  const muted = row.querySelector(".rack__content, .rack__command, .rack__target");
  const bg = getComputedStyle(row).backgroundColor;
  return {
    hovered: true,
    bg,
    bgRgb: resolve(bg),
    mutedColor: muted ? getComputedStyle(muted).color : null,
    mutedRgb: muted ? resolve(getComputedStyle(muted).color) : null,
    insideColored: !!row.closest(".group--colored"),
  };
})()`;

// Ticket 241: the same anatomy in the dock's flush strip. Rows are
// QuickLaunchRow cards, the panel is the visible tab, and the strip's lists
// differ (Launch unpadded, Actions/Clips padded).
const DOCK_MEASURE = `(() => {
  ${PROBE_HELPERS}
  const panel = document.querySelector(".tabs__panel:not([hidden])");
  const groups = [...(panel?.querySelectorAll("section.group") ?? [])].map((g) => {
    const label = g.querySelector(".disclosure__label");
    const rows = g.querySelector(".group__rows");
    const firstRow = rows?.querySelector(".dock-row");
    const bar = getComputedStyle(g, "::before");
    const section = g.getBoundingClientRect();
    const rowRect = firstRow?.getBoundingClientRect() ?? null;
    const labelRect = label?.getBoundingClientRect() ?? null;
    return {
      name: label?.textContent?.trim() ?? null,
      colored: g.classList.contains("group--colored"),
      barWidth: bar.width,
      barTop: bar.top,
      barBottom: bar.bottom,
      barHeight: bar.height,
      sectionHeight: Math.round(section.height),
      sectionX: Math.round(section.x),
      sectionRight: Math.round(section.right),
      titleX: labelRect ? Math.round(labelRect.left) : null,
      rowX: rowRect ? Math.round(rowRect.x) : null,
      rowRight: rowRect ? Math.round(rowRect.right) : null,
      bodyRgb: rows ? resolve(getComputedStyle(rows).backgroundColor) : null,
      shadow: rows ? getComputedStyle(rows).boxShadow : null,
      rowBgRgb: firstRow ? resolve(getComputedStyle(firstRow).backgroundColor) : null,
      rowBgVar: getComputedStyle(g).getPropertyValue("--group-row-bg").trim(),
      hoverVar: getComputedStyle(g).getPropertyValue("--group-row-hover").trim(),
      hoverTextVar: getComputedStyle(g).getPropertyValue("--group-row-hover-text").trim(),
      hasMenu: !!g.querySelector('[aria-haspopup="menu"]'),
    };
  });
  return {
    theme: document.documentElement.dataset.theme,
    groups,
    panelWidth: Math.round(panel?.getBoundingClientRect().width ?? 0),
    pageRgb: resolve(getComputedStyle(document.body).backgroundColor),
    bgHover: resolve(root.getPropertyValue("--bg-hover")),
    text: resolve(root.getPropertyValue("--text")),
    textMuted: resolve(root.getPropertyValue("--text-muted")),
  };
})()`;

const DOCK_HOVER = `(() => {
  ${PROBE_HELPERS}
  const row = document.querySelector(".tabs__panel:not([hidden]) .dock-row:hover");
  if (!row) return { hovered: false };
  const secondary = row.querySelector(".qlw__clip-excerpt, .qlw__entry-starting");
  const bg = getComputedStyle(row).backgroundColor;
  return {
    hovered: true,
    bg,
    bgRgb: resolve(bg),
    secondaryColor: secondary ? getComputedStyle(secondary).color : null,
    secondaryRgb: secondary ? resolve(getComputedStyle(secondary).color) : null,
    insideColored: !!row.closest(".group--colored"),
  };
})()`;

// ---- color math (mirrors tools/contrast-check.mjs) ----
const hexToRgb = (h) => [1, 3, 5].map((i) => parseInt(h.replace("#", "").slice(i - 1, i + 1), 16));
const linear = (v) => {
  const c = v / 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
};
const delinear = (c) => Math.max(0, Math.min(255, Math.round((c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055) * 255)));
function rgbToOklab([r, g, b]) {
  const R = linear(r), G = linear(g), B = linear(b);
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}
function oklabToRgb([L, a, b]) {
  const l = Math.pow(L + 0.3963377774 * a + 0.2158037573 * b, 3);
  const m = Math.pow(L - 0.1055613458 * a - 0.0638541728 * b, 3);
  const s = Math.pow(L - 0.0894841775 * a - 1.291485548 * b, 3);
  return [
    delinear(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    delinear(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    delinear(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
  ];
}
const mixOklab = (a, pct, b) => {
  const A = rgbToOklab(hexToRgb(a));
  const B = rgbToOklab(hexToRgb(b));
  const t = pct / 100;
  return oklabToRgb(A.map((v, i) => v * t + B[i] * (1 - t)));
};
const lightness = (rgb) => rgbToOklab(rgb)[0] * 100;
const luminance = (rgb) => 0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2]);
const contrast = (a, b) => {
  const x = luminance(a), y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};
const over = (fg, bg) => {
  const a = fg.a ?? 1;
  const f = [fg.r, fg.g, fg.b];
  const b = [bg.r, bg.g, bg.b];
  return f.map((c, i) => Math.round(c * a + b[i] * (1 - a)));
};
const near = (rgb, expected, tol = 2) => {
  if (!rgb || !expected) return false;
  const e = Array.isArray(expected) ? expected : [expected.r, expected.g, expected.b];
  return [rgb.r, rgb.g, rgb.b].every((c, i) => Math.abs(c - e[i]) <= tol);
};
const rgbText = (c) => (c ? `rgb(${c.r}, ${c.g}, ${c.b})` : "null");

async function moveMouse(cdp, x, y) {
  await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "none" });
}

// ---- targets: the dock/Quick Launch window is a second page target ----
async function listTargets(port) {
  const list = await fetchJson(`http://127.0.0.1:${port}/json`);
  return list.filter(
    (t) => t.type === "page" && t.url !== "about:blank" && !t.url.startsWith("devtools://"),
  );
}

async function attach(target) {
  const cdp = new Cdp(target.webSocketDebuggerUrl);
  await cdp.connect();
  await cdp.send("Runtime.enable");
  await cdp.send("Page.enable");
  return cdp;
}

async function waitForTarget(port, predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const found = (await listTargets(port)).find(predicate);
      if (found) return found;
    } catch {}
    await sleep(400);
  }
  throw new Error(`${label} never appeared`);
}

// Scrolls the named dock group's first row into view and returns a point in it.
async function hoverDockRow(cdp, groupName) {
  return cdp.eval(`(() => {
    const g = [...document.querySelectorAll(".tabs__panel:not([hidden]) section.group")].find(
      (s) => s.querySelector(".disclosure__label")?.textContent?.trim() === ${JSON.stringify(groupName)}
    );
    const row = g?.querySelector(".dock-row");
    if (!row) return null;
    row.scrollIntoView({ block: "center" });
    const r = row.getBoundingClientRect();
    return { x: Math.round(r.x + Math.min(40, r.width / 2)), y: Math.round(r.y + r.height / 2) };
  })()`);
}

// Scrolls the named group's first row into view and returns a point inside it.
async function hoverGroupRow(cdp, groupName) {
  return cdp.eval(`(() => {
    const g = [...document.querySelectorAll("section.group")].find(
      (s) => s.querySelector(".disclosure__label")?.textContent?.trim() === ${JSON.stringify(groupName)}
    );
    const row = g?.querySelector(".rack__row");
    if (!row) return null;
    row.scrollIntoView({ block: "center" });
    const r = row.getBoundingClientRect();
    return { x: Math.round(r.x + Math.min(60, r.width / 2)), y: Math.round(r.y + r.height / 2) };
  })()`);
}

async function goTo(cdp, page) {
  await moveMouse(cdp, 4, 4);
  const clicked = await cdp.eval(`(() => {
    const a = document.querySelector('a.rail__item[href="${page.href}"]');
    if (a) { a.click(); return true; }
    location.href = "${page.href}";
    return false;
  })()`);
  await waitFor(cdp, `!!document.querySelector("${page.marker}")`, 20_000, `${page.name} page`);
  await cdp.eval(`window.scrollTo(0, 0); true`);
  await sleep(500);
  return clicked;
}

function evaluate(checks, report) {
  return checks.map((c) => {
    report.push(c);
    console.log(`${c.ok ? "ok  " : "RED "} ${c.label}${c.detail ? ` — ${c.detail}` : ""}`);
    return c.ok;
  });
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const port = await pickFreePort(args.port);
  const bootTimeout = args.mode === "dev" ? 600_000 : 30_000;
  const report = [];
  const checks = [];

  console.log(`repro-group-color-hover mode=${args.mode} port=${port} data=${DATA_DIR} shots=${args.shots}`);

  killStraySprout();
  rmSync(DATA_DIR, { recursive: true, force: true });
  await waitDevServerFree(port);
  await sleep(600);

  let child = launch(args.mode, port);
  let cdp = null;
  let infraError = null;
  try {
    const tBoot = Date.now();
    const { cdp: mainCdp } = await connectMainWindow(port, bootTimeout);
    console.log(`boot: ${((Date.now() - tBoot) / 1000).toFixed(1)}s`);
    cdp = mainCdp;
    await cdp.send("Page.enable");
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width: 1180,
      height: 820,
      deviceScaleFactor: 1,
      mobile: false,
    });

    // The hue is part of the seed payload below.
    const seeded = await cdp.eval(
      `(async () => {
        const inv = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
        const defs = [
          { collection: "launch", color: "Work", hue: ${JSON.stringify(HUE.Work)}, neutral: "Neutral launch" },
          { collection: "action", color: "Deploy", hue: ${JSON.stringify(HUE.Deploy)}, neutral: "Neutral actions" },
          { collection: "clip", color: "Snippets", hue: ${JSON.stringify(HUE.Snippets)}, neutral: "Neutral clips" },
        ];
        const make = {
          launch: (name) => inv("create_launch_entry", { entry: { name, kind: "command", target: "echo " + name, shell: "powershell", show_window: false, desktop_id: null, show_in_dock: true } }),
          action: (name) => inv("create_quick_action", { action: { name, shell: "powershell", command: "echo " + name, cwd: null, stoppable: false, stop_command: null, auto_run: false, show_in_dock: true } }),
          clip: (name) => inv("create_clip", { clip: { name, content: "content for " + name, show_in_dock: true } }),
        };
        const made = [];
        for (const d of defs) {
          await inv("update_groups_enabled", { collection: d.collection, enabled: true });
          const colored = await inv("create_group", { collection: d.collection, name: d.color });
          await inv("set_group_color", { id: colored.id, color: d.hue });
          for (const suffix of [" one", " two"]) {
            const item = await make[d.collection](d.color + suffix);
            await inv("assign_to_group", { collection: d.collection, itemId: item.id, groupId: colored.id });
          }
          const neutral = await inv("create_group", { collection: d.collection, name: d.neutral });
          const item = await make[d.collection](d.neutral + " one");
          await inv("assign_to_group", { collection: d.collection, itemId: item.id, groupId: neutral.id });
          made.push({ collection: d.collection, color: d.color, hue: d.hue, neutral: d.neutral });
        }
        return made;
      })()`,
    );
    console.log(`seeded: ${JSON.stringify(seeded)}`);
    const hueFor = Object.fromEntries(seeded.map((s) => [s.color, s.hue]));

    for (const theme of ["light", "dark"]) {
      await cdp.eval(`(async () => {
        await window.__TAURI_INTERNALS__.invoke("update_theme", { theme: ${JSON.stringify(theme)} });
        localStorage.setItem("sprout.theme", ${JSON.stringify(theme)});
        return true;
      })()`);
      try {
        await cdp.eval(`location.reload(); true`);
      } catch {}
      await waitFor(
        cdp,
        `document.documentElement.dataset.theme === ${JSON.stringify(theme)} && !!document.querySelector("section.group")`,
        30_000,
        `${theme} grouped library`,
      );
      await sleep(500);

      for (const page of PAGES) {
        const clicked = await goTo(cdp, page);
        console.log(`[${theme}] ${page.name}${clicked ? "" : " (full reload)"}`);
        const m = await cdp.eval(MEASURE);
        const colored = m.groups.find((g) => g.colored);
        const neutral = m.groups.find((g) => !g.colored);
        const hue = colored ? hueFor[colored.name] : null;
        const tintPct = theme === "dark" ? 14 : 8;
        const expectedTint = hue ? mixOklab(hue, tintPct, theme === "dark" ? "#191919" : "#ffffff") : null;
        const expectedHover = hue ? mixOklab(hue, 14, theme === "dark" ? "#2e2e2e" : "#f1f1ef") : null;
        const tag = `[${theme}] ${page.name}`;

        checks.push({
          ok: m.theme === theme && !!colored && !!neutral,
          label: `${tag} colored + neutral group present`,
          detail: `theme=${m.theme} colored=${colored?.name} neutral=${neutral?.name}`,
        });
        if (!colored || !neutral) continue;

        checks.push({
          ok: colored.barWidth === "3px" && colored.barTop === "0px" && colored.barBottom === "0px",
          label: `${tag} 3 px bar anchored to the section's full height`,
          detail: `width=${colored.barWidth} top=${colored.barTop} bottom=${colored.barBottom}`,
        });
        checks.push({
          ok: Math.abs(Number(colored.barHeight.replace("px", "")) - colored.sectionHeight) <= 1,
          label: `${tag} bar spans header + body (no header stub)`,
          detail: `bar=${colored.barHeight} section=${colored.sectionHeight}px`,
        });
        checks.push({
          ok: colored.bodyRgb?.a === 0 && neutral.bodyRgb?.a === 0,
          label: `${tag} group body stays on the page surface`,
          detail: `colored=${rgbText(colored.bodyRgb)} neutral=${rgbText(neutral.bodyRgb)}`,
        });
        checks.push({
          ok: near(colored.rowBgRgb, expectedTint, 2) && (colored.rowBgRgb?.a ?? 0) === 1 &&
            neutral.rowBgRgb?.a === 0,
          label: `${tag} card carries the ${tintPct} % Oklab tint`,
          detail: `got ${rgbText(colored.rowBgRgb)} expected rgb(${expectedTint?.join(", ")}); neutral=${rgbText(neutral.rowBgRgb)}`,
        });
        checks.push({
          ok: !!colored.rowBgVar && /color-mix|oklab|rgb/.test(colored.rowBgVar) &&
            (!neutral.rowBgVar || !/color-mix/.test(neutral.rowBgVar)),
          label: `${tag} only the colored group publishes a row tint`,
          detail: `colored="${colored.rowBgVar}" neutral="${neutral.rowBgVar}"`,
        });
        checks.push({
          ok: colored.shadow === "none",
          label: `${tag} colored outline removed`,
          detail: colored.shadow,
        });
        checks.push({
          ok: Math.abs(colored.titleX - (colored.rowRect?.x ?? colored.titleX)) <= 1 &&
            Math.abs(neutral.titleX - (neutral.rowRect?.x ?? neutral.titleX)) <= 1,
          label: `${tag} header title and rows share one start in both groups`,
          detail: `colored title=${colored.titleX} row=${colored.rowRect?.x} neutral title=${neutral.titleX} row=${neutral.rowRect?.x}`,
        });
        checks.push({
          ok: !!colored.hoverVar && /color-mix|oklab|rgb/.test(colored.hoverVar) &&
            (!neutral.hoverVar || !/color-mix/.test(neutral.hoverVar)),
          label: `${tag} only the colored group publishes a hover surface`,
          detail: `colored="${colored.hoverVar}" neutral="${neutral.hoverVar}"`,
        });

        // Rest screenshot, then hover the colored group's first row.
        await capture(cdp, args.shots, `${page.name}-${theme}-rest.png`);
        if (!page.interactive) {
          const point = await hoverGroupRow(cdp, colored.name);
          if (point) {
            await moveMouse(cdp, point.x, point.y);
            await sleep(300);
            const h = await cdp.eval(HOVER);
            checks.push({
              ok: h.hovered && near(h.bgRgb, colored.rowBgRgb, 2),
              label: `${tag} display-only rows keep their card tint (no hover state)`,
              detail: `rest=${rgbText(colored.rowBgRgb)} hovered=${h.bg}`,
            });
          }
          continue;
        }

        const point = await hoverGroupRow(cdp, colored.name);
        await moveMouse(cdp, point.x, point.y);
        await sleep(350);
        const hover = await cdp.eval(HOVER);
        const hoverText = hover.mutedRgb;
        const hoverContrast = hoverText && hover.bgRgb
          ? contrast(over(hoverText, hover.bgRgb), [hover.bgRgb.r, hover.bgRgb.g, hover.bgRgb.b])
          : 0;
        const step = hover.bgRgb ? Math.abs(lightness([hover.bgRgb.r, hover.bgRgb.g, hover.bgRgb.b]) - lightness([colored.rowBgRgb.r, colored.rowBgRgb.g, colored.rowBgRgb.b])) : 0;
        checks.push({
          ok: hover.hovered && hover.insideColored && near(hover.bgRgb, expectedHover, 2),
          label: `${tag} hovered colored row carries the hue`,
          detail: `got ${rgbText(hover.bgRgb)} expected rgb(${expectedHover?.join(", ")})`,
        });
        checks.push({
          ok: near(hoverText, m.text, 2),
          label: `${tag} hovered colored row promotes secondary text to --text`,
          detail: `${hover.mutedColor} vs ${rgbText(m.text)}`,
        });
        checks.push({
          ok: hoverContrast >= 4.5,
          label: `${tag} hovered text clears AA on the tinted surface`,
          detail: `${hoverContrast.toFixed(2)}:1`,
        });
        checks.push({
          ok: step >= 3,
          label: `${tag} hover is a visible lightness step from the card tint`,
          detail: `dL=${step.toFixed(2)}`,
        });
        await capture(cdp, args.shots, `${page.name}-${theme}-hover.png`);

        // Neutral group: neutral hover surface, secondary text unchanged.
        const neutralPoint = await hoverGroupRow(cdp, neutral.name);
        if (neutralPoint) {
          await moveMouse(cdp, neutralPoint.x, neutralPoint.y);
          await sleep(350);
          const nh = await cdp.eval(HOVER);
          const neutralText = nh.mutedRgb;
          const neutralContrast = neutralText && nh.bgRgb
            ? contrast(over(neutralText, nh.bgRgb), [nh.bgRgb.r, nh.bgRgb.g, nh.bgRgb.b])
            : 0;
          checks.push({
            ok: nh.hovered && !nh.insideColored && near(nh.bgRgb, [m.bgHover.r, m.bgHover.g, m.bgHover.b], 2),
            label: `${tag} neutral hover stays on --bg-hover`,
            detail: `got ${rgbText(nh.bgRgb)} expected ${rgbText(m.bgHover)}`,
          });
          checks.push({
            ok: near(neutralText, m.textMuted, 2) && neutralContrast >= 4.5,
            label: `${tag} neutral hover keeps --text-muted and clears AA`,
            detail: `${nh.mutedColor} ${neutralContrast.toFixed(2)}:1`,
          });
          await capture(cdp, args.shots, `${page.name}-${theme}-neutral-hover.png`);
        }
      }
    }

    // ---- Dock (ticket 241): the same anatomy in the flush strip ----
    // The dock is tray-resident and only restores at boot when docked, so
    // persist the state and restart; the Quick Launch target is the strip.
    await cdp.eval(`(async () => {
      const inv = window.__TAURI_INTERNALS__.invoke;
      const settings = await inv("get_settings");
      await inv("update_settings", { settings: { ...settings, dock_state: "docked" } });
      return true;
    })()`);
    cdp.close();
    killTree(child);
    if (!(await waitDevServerFree(port))) {
      throw new Error("previous dev/WebView2 tree still holds :1420 or the debug port");
    }
    child = launch(args.mode, port);
    const quickTarget = await waitForTarget(
      port,
      (t) => t.url.includes("quick-launch-window"),
      bootTimeout,
      "dock target",
    );
    const dock = await attach(quickTarget);
    await waitFor(dock, `!!document.querySelector("#tab-clips")`, 30_000, "dock tabs");
    console.log(`dock: attached (${quickTarget.url})`);

    const DOCK_TABS = [
      { id: "launch", name: "launch", excerpt: false },
      { id: "actions", name: "actions", excerpt: false },
      { id: "clips", name: "clips", excerpt: true },
    ];

    for (const theme of ["light", "dark"]) {
      await dock.eval(`(async () => {
        await window.__TAURI_INTERNALS__.invoke("update_theme", { theme: ${JSON.stringify(theme)} });
        localStorage.setItem("sprout.theme", ${JSON.stringify(theme)});
        return true;
      })()`);
      try {
        await dock.eval(`location.reload(); true`);
      } catch {}
      await waitFor(
        dock,
        `document.documentElement.dataset.theme === ${JSON.stringify(theme)} && !!document.querySelector("#tab-clips")`,
        30_000,
        `${theme} dock`,
      );
      await sleep(600);

      for (const tab of DOCK_TABS) {
        await dock.eval(`(() => { document.querySelector("#tab-${tab.id}")?.click(); return true; })()`);
        await waitFor(
          dock,
          `!!document.querySelector(".tabs__panel:not([hidden]) section.group")`,
          15_000,
          `${tab.id} dock panel`,
        );
        await sleep(300);

        const m = await dock.eval(DOCK_MEASURE);
        const colored = m.groups.find((g) => g.colored);
        const neutral = m.groups.find((g) => !g.colored);
        const hue = colored ? hueFor[colored.name] : null;
        const tintPct = theme === "dark" ? 14 : 8;
        const expectedTint = hue ? mixOklab(hue, tintPct, theme === "dark" ? "#191919" : "#ffffff") : null;
        const expectedHover = hue ? mixOklab(hue, 14, theme === "dark" ? "#2e2e2e" : "#f1f1ef") : null;
        const tag = `[${theme}] dock/${tab.name}`;

        checks.push({
          ok: m.theme === theme && !!colored && !!neutral,
          label: `${tag} colored + neutral group render`,
          detail: `theme=${m.theme} colored=${colored?.name} neutral=${neutral?.name} panel=${m.panelWidth}px`,
        });
        if (!colored || !neutral) continue;

        checks.push({
          ok:
            colored.barWidth === "3px" &&
            colored.barTop === "0px" &&
            colored.barBottom === "0px" &&
            Math.abs(Number(colored.barHeight.replace("px", "")) - colored.sectionHeight) <= 1,
          label: `${tag} one 3 px bar spans the section`,
          detail: `width=${colored.barWidth} top=${colored.barTop} bottom=${colored.barBottom} bar=${colored.barHeight} section=${colored.sectionHeight}px`,
        });
        checks.push({
          ok: colored.bodyRgb?.a === 0 && neutral.bodyRgb?.a === 0,
          label: `${tag} section body stays on the page surface`,
          detail: `colored=${rgbText(colored.bodyRgb)} neutral=${rgbText(neutral.bodyRgb)}`,
        });
        checks.push({
          ok: near(colored.rowBgRgb, expectedTint, 2) && (colored.rowBgRgb?.a ?? 0) === 1 &&
            neutral.rowBgRgb?.a === 0,
          label: `${tag} dock card carries the ${tintPct} % Oklab tint`,
          detail: `got ${rgbText(colored.rowBgRgb)} expected rgb(${expectedTint?.join(", ")}); neutral=${rgbText(neutral.rowBgRgb)}`,
        });
        checks.push({
          ok: !!colored.rowBgVar && /color-mix|oklab|rgb/.test(colored.rowBgVar) &&
            (!neutral.rowBgVar || !/color-mix/.test(neutral.rowBgVar)),
          label: `${tag} only the colored group publishes a row tint`,
          detail: `colored="${colored.rowBgVar}" neutral="${neutral.rowBgVar}"`,
        });
        checks.push({
          ok: colored.shadow === "none",
          label: `${tag} no coloured outline`,
          detail: colored.shadow,
        });
        checks.push({
          ok: Math.abs(colored.titleX - colored.rowX - (neutral.titleX - neutral.rowX)) <= 1,
          label: `${tag} inset preserves the header-to-row relationship`,
          detail: `colored title-row=${colored.titleX - colored.rowX} neutral=${neutral.titleX - neutral.rowX}`,
        });
        checks.push({
          ok: Math.abs(colored.rowX - neutral.rowX - 8) <= 1 && Math.abs(neutral.rowRight - colored.rowRight) <= 1,
          label: `${tag} flush inset is 8 px and rows keep the strip's right edge`,
          detail: `colored ${colored.rowX}-${colored.rowRight} neutral ${neutral.rowX}-${neutral.rowRight}`,
        });
        checks.push({
          ok: !colored.hasMenu && !neutral.hasMenu,
          label: `${tag} dock groups expose no menu`,
          detail: `colored=${colored.hasMenu} neutral=${neutral.hasMenu}`,
        });

        await capture(dock, args.shots, `dock-${tab.name}-${theme}-rest.png`);

        const point = await hoverDockRow(dock, colored.name);
        if (!point) continue;
        await moveMouse(dock, point.x, point.y);
        await sleep(350);
        const h = await dock.eval(DOCK_HOVER);
        const hoverContrast = h.secondaryRgb && h.bgRgb
          ? contrast(over(h.secondaryRgb, h.bgRgb), [h.bgRgb.r, h.bgRgb.g, h.bgRgb.b])
          : 0;
        checks.push({
          ok: h.hovered && h.insideColored && near(h.bgRgb, expectedHover, 2),
          label: `${tag} hovered dock row carries the hue`,
          detail: `got ${rgbText(h.bgRgb)} expected rgb(${expectedHover?.join(", ")})`,
        });
        if (tab.excerpt) {
          checks.push({
            ok: near(h.secondaryRgb, m.text, 2),
            label: `${tag} hover promotes the excerpt to --text`,
            detail: `${h.secondaryColor} vs ${rgbText(m.text)}`,
          });
          checks.push({
            ok: hoverContrast >= 4.5,
            label: `${tag} excerpt clears AA on the tinted hover`,
            detail: `${hoverContrast.toFixed(2)}:1`,
          });
        }
        await capture(dock, args.shots, `dock-${tab.name}-${theme}-hover.png`);

        const neutralPoint = await hoverDockRow(dock, neutral.name);
        if (neutralPoint) {
          await moveMouse(dock, neutralPoint.x, neutralPoint.y);
          await sleep(350);
          const nh = await dock.eval(DOCK_HOVER);
          const neutralContrast = nh.secondaryRgb && m.pageRgb
            ? contrast(over(nh.secondaryRgb, m.pageRgb), [m.pageRgb.r, m.pageRgb.g, m.pageRgb.b])
            : 0;
          checks.push({
            ok: nh.hovered && !nh.insideColored && nh.bgRgb && nh.bgRgb.a === 0,
            label: `${tag} neutral dock row keeps the clear card`,
            detail: `bg=${nh.bg}`,
          });
          if (nh.secondaryColor) {
            checks.push({
              ok: near(nh.secondaryRgb, m.textMuted, 2),
              label: `${tag} neutral dock row keeps --text-muted`,
              detail: nh.secondaryColor,
            });
            checks.push({
              ok: neutralContrast >= 4.5,
              label: `${tag} neutral dock text clears AA`,
              detail: `${neutralContrast.toFixed(2)}:1`,
            });
          }
          await capture(dock, args.shots, `dock-${tab.name}-${theme}-neutral-hover.png`);
        }
      }
    }
    dock.close();
  } catch (e) {
    infraError = e;
  } finally {
    if (infraError) {
      console.error(`INFRA ERROR: ${infraError.message}`);
      if (child.tail) console.error(child.tail.slice(-10).join(""));
      process.exitCode = 2;
    } else {
      const red = evaluate(checks, report).some((ok) => !ok);
      mkdirSync(args.shots, { recursive: true });
      writeFileSync(path.join(args.shots, "report.json"), JSON.stringify(report, null, 2));
      console.log(red ? "VERDICT: RED" : "VERDICT: GREEN");
      process.exitCode = red ? 1 : 0;
    }

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
