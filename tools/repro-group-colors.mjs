import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

// Ticket 225 evidence: drives the real app through CDP, seeds one colored
// group per hue plus a neutral one, and verifies the rendered anatomy in both
// themes — 3 px accent bar, body tint at or under 12 %, 1 px outline, chrome
// text on tokens, and 3:1 non-text contrast for the bar/dot/outline. It also
// exercises the menu dot, the dialog's ten swatches, focus trap, Escape, and
// the inline invalid-hex error. Runs against a throwaway LOCALAPPDATA so it
// never touches the user's Library. Exit: 0 green, 1 red, 2 infra.

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TEMP_ROOT = path.join(os.tmpdir(), "opencode");
const DATA_DIR = path.join(TEMP_ROOT, "sprout-225-appdata");

const USAGE = `Usage: node tools/repro-group-colors.mjs [options]

Seeds colored groups in a throwaway app-data directory and checks the group
color rendering, menu dot, and dialog in light and dark.

Options:
  --mode dev|exe   app under test (default dev)
  --port P         CDP port (default: first free of 9222,9333,9444,9555,9666)
  --shots DIR      save light/dark PNGs (default: %TEMP%\\opencode\\evidence-225)
  --keep           leave the app running after the verdict
  --help`;

function parseArgs(argv) {
  const args = {
    mode: "dev",
    port: null,
    shots: path.join(TEMP_ROOT, "evidence-225"),
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

// Seeds sequentially: assign_to_group sweeps empty groups, so each group gets
// its member in the same round-trip that creates it.
const SEED = `(async () => {
  const inv = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
  await inv("update_groups_enabled", { collection: "launch", enabled: true });
  const defs = [
    { name: "Work", color: "#417ed4" },
    { name: "Games", color: "#e03a8d" },
    { name: "Reading", color: "#1c9139" },
    { name: "Neutral", color: null },
  ];
  const made = [];
  for (const d of defs) {
    const g = await inv("create_group", { collection: "launch", name: d.name });
    if (d.color) await inv("set_group_color", { id: g.id, color: d.color });
    const e = await inv("create_launch_entry", {
      entry: {
        name: d.name + " entry",
        kind: "command",
        target: "echo " + d.name,
        shell: "powershell",
        show_window: false,
        desktop_id: null,
        show_in_dock: false,
      },
    });
    await inv("assign_to_group", { collection: "launch", itemId: e.id, groupId: g.id });
    made.push({ id: g.id, name: d.name, color: d.color });
  }
  return made;
})()`;

// In-page anatomy probe. Colors are composited over their real backdrop before
// the ratio is taken, so a tinted body never flatters its own outline.
const MEASURE = `(() => {
  const parse = (value) => {
    if (!value) return null;
    // color-mix() computes to the modern color(srgb ...) form in WebView2.
    const srgb = value.match(/color\\(srgb\\s+([\\d.]+)\\s+([\\d.]+)\\s+([\\d.]+)(?:\\s*\\/\\s*([\\d.]+))?\\)/);
    if (srgb) {
      return {
        r: parseFloat(srgb[1]) * 255,
        g: parseFloat(srgb[2]) * 255,
        b: parseFloat(srgb[3]) * 255,
        a: srgb[4] === undefined ? 1 : parseFloat(srgb[4]),
      };
    }
    const m = value.match(/rgba?\\(([^)]+)\\)/);
    if (!m) return null;
    const p = m[1].split(",").map((n) => parseFloat(n.trim()));
    return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
  };
  const over = (fg, bg) => {
    const a = fg.a ?? 1;
    return {
      r: Math.round(fg.r * a + bg.r * (1 - a)),
      g: Math.round(fg.g * a + bg.g * (1 - a)),
      b: Math.round(fg.b * a + bg.b * (1 - a)),
      a: 1,
    };
  };
  const lum = (c) => {
    const ch = (v) => {
      const s = v / 255;
      return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
    };
    return 0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b);
  };
  const ratio = (a, b) => {
    const l1 = lum(a);
    const l2 = lum(b);
    return Math.round(((Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05)) * 100) / 100;
  };
  const page = parse(getComputedStyle(document.body).backgroundColor) ?? { r: 255, g: 255, b: 255, a: 1 };
  const groups = [...document.querySelectorAll("section.group")].map((g) => {
    const head = g.querySelector(".group__head");
    const label = g.querySelector(".disclosure__label");
    const rows = g.querySelector(".group__rows");
    const bar = getComputedStyle(head, "::before");
    const rowsStyle = rows ? getComputedStyle(rows) : null;
    const tint = rowsStyle ? parse(rowsStyle.backgroundColor) : null;
    const tinted = tint ? over(tint, page) : null;
    const barColor = parse(bar.backgroundColor);
    const outline = rowsStyle ? parse(rowsStyle.boxShadow) : null;
    const badge = g.querySelector(".badge");
    return {
      name: label?.textContent?.trim() ?? null,
      colored: g.classList.contains("group--colored"),
      barWidth: bar.width,
      barColor: bar.backgroundColor,
      barContrast: barColor ? ratio(over(barColor, page), page) : null,
      tint: rowsStyle?.backgroundColor ?? null,
      tintAlpha: tint ? tint.a : null,
      tintContrast: outline && tinted ? ratio(over(outline, tinted), tinted) : null,
      outline: rowsStyle?.boxShadow ?? null,
      textColor: label ? getComputedStyle(label).color : null,
      badgeColor: badge ? getComputedStyle(badge).color : null,
      rows: g.querySelectorAll(".rack > li").length,
    };
  });
  return {
    theme: document.documentElement.dataset.theme,
    page: getComputedStyle(document.body).backgroundColor,
    groups,
  };
})()`;

async function openGroupMenu(cdp) {
  const clicked = await cdp.eval(`(() => {
    const g = document.querySelector("section.group--colored");
    const b = g?.querySelector("button[data-ctx-trigger]");
    if (!b) return false;
    b.click();
    return true;
  })()`);
  if (!clicked) throw new Error("group menu trigger not found");
  await waitFor(cdp, `!!document.querySelector(".ctx-menu")`, 5000, "group menu");
}

const MENU_PROBE = `(() => {
  const menu = document.querySelector(".ctx-menu");
  // Locale-independent: the Color row is the one carrying the dot slot.
  const row = menu.querySelector(".ctx-item__swatch")?.closest(".ctx-item") ?? null;
  if (!row) return { found: false, rows: [...menu.querySelectorAll(".ctx-item")].map((b) => b.textContent.trim()) };
  const dot = row.querySelector(".ctx-item__swatch");
  const bg = dot ? getComputedStyle(dot).backgroundColor : null;
  const surface = getComputedStyle(menu).backgroundColor;
  return { found: true, dotBg: bg, dotSize: dot ? dot.getBoundingClientRect().width : null, surface, hollow: dot?.classList.contains("ctx-item__swatch--none") ?? null };
})()`;

async function openColorDialog(cdp) {
  const opened = await cdp.eval(`(() => {
    const menu = document.querySelector(".ctx-menu");
    const row = menu.querySelector(".ctx-item__swatch")?.closest(".ctx-item") ?? null;
    if (!row) return false;
    row.click();
    return true;
  })()`);
  if (!opened) throw new Error("Color menu row not found");
  await waitFor(cdp, `!!document.querySelector("dialog.dialog[open]")`, 5000, "color dialog");
  await sleep(250);
}

const DIALOG_PROBE = `(() => {
  const dialog = document.querySelector("dialog.dialog[open]");
  const swatches = [...dialog.querySelectorAll(".swatch")];
  const active = document.activeElement;
  return {
    title: dialog.querySelector(".dialog__title")?.textContent?.trim() ?? null,
    swatchCount: swatches.length,
    labelled: swatches.every((s) => (s.getAttribute("aria-label") ?? "").length > 0),
    pressed: swatches.filter((s) => s.getAttribute("aria-pressed") === "true").length,
    focusInside: dialog.contains(active),
    focusTag: active?.tagName ?? null,
    swatchLabels: swatches.map((s) => s.getAttribute("aria-label")),
  };
})()`;

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

  console.log(`repro-group-colors mode=${args.mode} port=${port} data=${DATA_DIR} shots=${args.shots}`);

  killStraySprout();
  rmSync(DATA_DIR, { recursive: true, force: true });
  await sleep(1500);

  const child = launch(args.mode, port);
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

    // Seed before the themed reloads so every pass reads persisted state.
    const seeded = await cdp.eval(SEED);
    console.log(`seeded: ${JSON.stringify(seeded)}`);

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
      await cdp.eval(`window.scrollTo(0, 0); true`);
      await sleep(400);

      const m = await cdp.eval(MEASURE);
      console.log(`\n[${theme}] ${JSON.stringify(m)}`);
      await capture(cdp, args.shots, `library-${theme}.png`);

      const colored = m.groups.filter((g) => g.colored);
      const neutral = m.groups.find((g) => !g.colored);
      const textColors = new Set(m.groups.map((g) => g.textColor));

      checks.push({
        ok: m.theme === theme,
        label: `[${theme}] applied theme`,
        detail: m.theme,
      });
      checks.push({
        ok: colored.length === 3 && !!neutral,
        label: `[${theme}] three colored groups plus one neutral`,
        detail: `${colored.length} colored / ${neutral ? "neutral present" : "no neutral"}`,
      });
      checks.push({
        ok: colored.every((g) => g.barWidth === "3px"),
        label: `[${theme}] 3 px accent bar`,
        detail: colored.map((g) => `${g.name}=${g.barWidth}`).join(" "),
      });
      checks.push({
        ok: colored.every((g) => g.tintAlpha !== null && g.tintAlpha > 0 && g.tintAlpha <= 0.12),
        label: `[${theme}] body tint at or under 12 %`,
        detail: colored.map((g) => `${g.name}=${g.tintAlpha}`).join(" "),
      });
      checks.push({
        ok: colored.every((g) => (g.outline ?? "").includes("1px") && (g.outline ?? "").includes("inset")),
        label: `[${theme}] 1 px inset outline`,
        detail: colored.map((g) => g.outline).join(" | "),
      });
      checks.push({
        ok: colored.every((g) => g.barContrast >= 3),
        label: `[${theme}] accent bar ≥ 3:1 on the page`,
        detail: colored.map((g) => `${g.name}=${g.barContrast}:1`).join(" "),
      });
      checks.push({
        ok: colored.every((g) => g.tintContrast >= 3),
        label: `[${theme}] outline ≥ 3:1 on the tinted body`,
        detail: colored.map((g) => `${g.name}=${g.tintContrast}:1`).join(" "),
      });
      checks.push({
        ok: textColors.size === 1,
        label: `[${theme}] group text stays one token color`,
        detail: [...textColors].join(" | "),
      });
      checks.push({
        ok: !!neutral && !neutral.colored && neutral.barWidth !== "3px" && (neutral.tint === "rgba(0, 0, 0, 0)" || neutral.tintAlpha === 0),
        label: `[${theme}] neutral group carries no accent or tint`,
        detail: neutral ? `bar=${neutral.barWidth} tint=${neutral.tint}` : "missing",
      });

      // Menu dot for the colored group, then the dialog.
      const stored = await cdp.eval(`window.__TAURI_INTERNALS__.invoke("list_groups", { collection: "launch" })`);
      const workColor = stored.find((g) => g.id === seeded[0].id)?.color ?? null;
      await openGroupMenu(cdp);
      const menu = await cdp.eval(MENU_PROBE);
      const expected = colored[0]?.barColor ?? null;
      checks.push({
        ok: menu.found && menu.dotBg === expected && menu.dotSize >= 8,
        label: `[${theme}] Color row shows a dot in the group color`,
        detail: `${menu.dotBg} (${menu.dotSize}px) vs bar ${expected}`,
      });
      await capture(cdp, args.shots, `menu-${theme}.png`);

      await openColorDialog(cdp);
      const dialog = await cdp.eval(DIALOG_PROBE);
      checks.push({
        ok: dialog.swatchCount === 10 && dialog.labelled,
        label: `[${theme}] dialog has ten labelled swatches`,
        detail: `${dialog.swatchCount} swatches, labels ${dialog.labelled}`,
      });
      checks.push({
        ok: dialog.focusInside,
        label: `[${theme}] dialog takes focus inside itself`,
        detail: `${dialog.focusTag}`,
      });
      await capture(cdp, args.shots, `dialog-${theme}.png`);

      // Escape cancels: the dialog closes and the stored color is unchanged.
      await pressKey(cdp, "Escape", "Escape", 27);
      await sleep(300);
      const closed = await cdp.eval(`!document.querySelector("dialog.dialog[open]")`);
      const still = await cdp.eval(`window.__TAURI_INTERNALS__.invoke("list_groups", { collection: "launch" })`);
      checks.push({
        ok: closed && still.find((g) => g.id === seeded[0].id)?.color === workColor,
        label: `[${theme}] Escape cancels without writing`,
        detail: `closed=${closed} color=${still.find((g) => g.id === seeded[0].id)?.color}`,
      });

      if (theme === "light") {
        // Invalid hex: inline error, nothing stored.
        await openGroupMenu(cdp);
        await openColorDialog(cdp);
        await cdp.eval(`(() => {
          const input = document.querySelector("dialog.dialog[open] .field__input");
          input.focus();
          input.value = "not-a-color";
          input.dispatchEvent(new Event("input", { bubbles: true }));
          return true;
        })()`);
        await cdp.eval(`document.querySelector("dialog.dialog[open] form").requestSubmit()`);
        await waitFor(cdp, `!!document.querySelector("dialog.dialog[open] .notice")`, 5000, "inline color error");
        const err = await cdp.eval(`(() => {
          const n = document.querySelector("dialog.dialog[open] .notice");
          return { text: n?.textContent?.trim() ?? null, open: !!document.querySelector("dialog.dialog[open]") };
        })()`);
        const after = await cdp.eval(`window.__TAURI_INTERNALS__.invoke("list_groups", { collection: "launch" })`);
        checks.push({
          ok: !!err.text && err.open && after.find((g) => g.id === seeded[0].id)?.color === seeded[0].color,
          label: "[light] invalid hex shows an inline error and stores nothing",
          detail: `${err.text} | stored=${after.find((g) => g.id === seeded[0].id)?.color}`,
        });
        await capture(cdp, args.shots, "dialog-error-light.png");

        // Keyboard path: tab from the last focusable wraps to the first, then
        // a swatch applies immediately and closes the dialog.
        const wrap = await cdp.eval(`(() => {
          const dialog = document.querySelector("dialog.dialog[open]");
          const focusables = [...dialog.querySelectorAll('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])')]
            .filter((el) => !el.hasAttribute("disabled"));
          focusables[focusables.length - 1].focus();
          return { last: document.activeElement?.textContent?.trim() || document.activeElement?.tagName, first: focusables[0]?.tagName };
        })()`);
        await pressKey(cdp, "Tab", "Tab", 9);
        await sleep(150);
        const afterTab = await cdp.eval(`(() => {
          const dialog = document.querySelector("dialog.dialog[open]");
          const focusables = [...dialog.querySelectorAll('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])')]
            .filter((el) => !el.hasAttribute("disabled"));
          return { focusedIsFirst: document.activeElement === focusables[0], tag: document.activeElement?.tagName };
        })()`);
        checks.push({
          ok: afterTab.focusedIsFirst,
          label: "[light] Tab from the last control wraps inside the dialog",
          detail: `last=${wrap.last} → ${afterTab.tag}`,
        });

        const amber = await cdp.eval(`(() => {
          const s = [...document.querySelectorAll("dialog.dialog[open] .swatch")].find(
            (b) => getComputedStyle(b).backgroundColor === "rgb(150, 123, 41)"
          );
          if (!s) return false;
          s.focus();
          s.click();
          return true;
        })()`);
        await sleep(600);
        const applied = await cdp.eval(`window.__TAURI_INTERNALS__.invoke("list_groups", { collection: "launch" })`);
        checks.push({
          ok: amber && applied.find((g) => g.id === seeded[0].id)?.color === "#967b29",
          label: "[light] keyboard swatch applies immediately",
          detail: `clicked=${amber} stored=${applied.find((g) => g.id === seeded[0].id)?.color}`,
        });
      }
    }

    // Put the throwaway data back to a clean look and leave no dialog open.
    await cdp.eval(`document.querySelector("dialog.dialog[open]")?.close?.(); true`);
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
