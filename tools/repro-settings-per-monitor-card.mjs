import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import net from "node:net";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function killStraySprout() {
  try {
    spawnSync("taskkill", ["/IM", "sprout-windows-desktop.exe", "/F"], { stdio: "ignore" });
  } catch {}
}

const USAGE = `Usage: node tools/repro-settings-per-monitor-card.mjs [options]

Drives the Sprout app via CDP and asserts the Settings per-monitor dock card
never overflows horizontally once its controls stack vertically. Covers
1/2 displays, 100/150/200 % DPI, light/dark, and a 340 px card content width.
Verdict RED when any card row or control extends past the card's content box,
the card scrolls horizontally, or the per-monitor cards are missing when two
displays resolve. Exit code: 0=green 1=red 2=infra.

Options:
  --mode dev|exe   app under test (default dev; dev spawns npm run tauri dev
                   and enables the ?preview-per-monitor=1 two-display preview)
  --viewport W     viewport width in CSS px for the in-layout pass (default 900)
  --card W         constrained card width in CSS px (default 340)
  --dsf a,b,c      device scale factors, i.e. DPI (default 1,1.5,2)
  --port P         CDP port (default: first free of 9222,9333,9444,9555,9666)
  --keep           leave the app running after the verdict (debugging only)
  --legacy-row     inject the pre-fix row layout as a probe-sensitivity check
  --shots DIR      save light/dark PNGs of the 340 px card (two-display pass)
  --help`;

function parseArgs(argv) {
  const args = {
    mode: "dev",
    viewport: 900,
    card: 340,
    dsf: [1, 1.5, 2],
    port: null,
    keep: false,
    legacyRow: false,
    shots: null,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--help": console.log(USAGE); process.exit(0);
      case "--keep": args.keep = true; break;
      case "--legacy-row": args.legacyRow = true; break;
      case "--shots": args.shots = argv[++i]; break;
      case "--mode": args.mode = argv[++i]; break;
      case "--viewport": args.viewport = Number(argv[++i]); break;
      case "--card": args.card = Number(argv[++i]); break;
      case "--dsf": args.dsf = argv[++i].split(",").map(Number); break;
      case "--port": args.port = Number(argv[++i]); break;
      default: throw new Error(`unknown option: ${a}\n${USAGE}`);
    }
  }
  return args;
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
// Launch window. Ask it to open the main window through the same command the
// dock mark uses, then attach to the target that actually renders the nav rail.
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
      `JSON.stringify({ href: location.href, path: location.pathname, text: document.body?.innerText?.slice(0, 160) ?? null })`,
    );
  } catch {}
  throw new Error(`${label} never appeared; page state=${state}`);
}

// Runs in the page: the card's content box is its border box minus padding,
// so a control or row past that edge is real horizontal overflow.
const MEASURE = `(() => {
  const round = (n) => Math.round(n * 10) / 10;
  const describe = (el) => (el.id || el.className || el.tagName).toString().trim();
  const measureCard = (card) => {
    const cr = card.getBoundingClientRect();
    const cs = getComputedStyle(card);
    const padLeft = parseFloat(cs.paddingLeft);
    const padRight = parseFloat(cs.paddingRight);
    const contentLeft = cr.left + padLeft;
    const contentRight = cr.right - padRight;
    const rows = [...card.children].map((el) => {
      const r = el.getBoundingClientRect();
      return { name: describe(el), left: round(r.left), right: round(r.right), width: round(r.width) };
    });
    const controls = [...card.querySelectorAll("button, input[type=range], label")].map((el) => {
      const r = el.getBoundingClientRect();
      return { name: describe(el), left: round(r.left), right: round(r.right), width: round(r.width) };
    });
    const escaping = [...rows, ...controls].filter(
      (r) => r.right > contentRight + 0.5 || r.left < contentLeft - 0.5,
    );
    return {
      width: round(cr.width),
      contentWidth: round(contentRight - contentLeft),
      scrollWidth: card.scrollWidth,
      clientWidth: card.clientWidth,
      direction: cs.flexDirection,
      rows,
      escaping,
      overflow: card.scrollWidth > card.clientWidth + 1 || escaping.length > 0,
    };
  };
  const cards = [...document.querySelectorAll(".per-monitor article.knob")];
  const globalCard = document.querySelector("#dock-width")?.closest(".knob") ?? null;
  return {
    displays: document.querySelectorAll(".per-monitor article.knob").length,
    theme: document.documentElement.dataset.theme ?? "unknown",
    dpr: window.devicePixelRatio,
    cards: cards.map(measureCard),
    global: globalCard ? measureCard(globalCard) : null,
  };
})()`;

// The page remembers collapsed Settings sections; a collapsed section would
// hide the knobs under test (the theme picker lives in General, the dock
// knobs in Dock). Open every closed section and return the ids opened so the
// run can put them back afterwards.
async function openClosedGroups(cdp) {
  return cdp.eval(`(() => {
    const opened = [];
    for (const b of document.querySelectorAll('.group button.disclosure[aria-expanded="false"]')) {
      const controls = b.getAttribute("aria-controls");
      b.click();
      if (controls) opened.push(controls);
    }
    return opened;
  })()`);
}

async function restoreGroups(cdp, ids) {
  for (const id of ids) {
    await cdp.eval(`(() => {
      const b = document.querySelector('button.disclosure[aria-controls=' + ${JSON.stringify(JSON.stringify(id))} + '][aria-expanded="true"]');
      if (b) b.click();
      return true;
    })()`);
  }
}

function injectLegacyRow(cdp, on) {
  const css = on
    ? `.per-monitor article.knob {
        flex-direction: row !important;
        align-items: center !important;
        justify-content: space-between !important;
        gap: var(--space-5) !important;
      }
      .per-monitor article.knob > .knob__body { flex: 1 1 auto !important; }`
    : "";
  return cdp.eval(`(() => {
    let style = document.getElementById("legacy-row-probe");
    if (!style) {
      style = document.createElement("style");
      style.id = "legacy-row-probe";
      document.head.appendChild(style);
    }
    style.textContent = ${JSON.stringify(css)};
    return true;
  })()`);
}

async function capture(cdp, dir, name) {
  await cdp.eval(`document.querySelector(".per-monitor")?.scrollIntoView({ block: "center" })`);
  await sleep(250);
  const shot = await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true });
  mkdirSync(dir, { recursive: true });
  writeFileSync(path.join(dir, name), Buffer.from(shot.data, "base64"));
  console.log(`shot: ${path.join(dir, name)}`);
}

async function activeTheme(cdp) {
  return cdp.eval(`(() => {
    const active = document.querySelector(".theme-picker__option--active");
    return active ? active.textContent.trim() : null;
  })()`);
}

function setTheme(cdp, label) {
  return cdp.eval(`(() => {
    const buttons = [...document.querySelectorAll(".theme-picker button")];
    const target = buttons.find((b) => b.textContent.trim() === ${JSON.stringify(label)});
    if (!target) return false;
    target.click();
    return true;
  })()`);
}

function constrainCard(cdp, width) {
  const css =
    width === null
      ? ""
      : `.per-monitor { width: ${width}px !important; max-width: ${width}px !important; }`;
  return cdp.eval(`(() => {
    let style = document.getElementById("per-monitor-probe");
    if (!style) {
      style = document.createElement("style");
      style.id = "per-monitor-probe";
      document.head.appendChild(style);
    }
    style.textContent = ${JSON.stringify(css)};
    return true;
  })()`);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const port = await pickFreePort(args.port);
  const bootTimeout = args.mode === "dev" ? 600_000 : 30_000;
  const previewSupported = args.mode === "dev";

  console.log(
    `repro-settings-per-monitor-card mode=${args.mode} port=${port} viewport=${args.viewport} card=${args.card} dsf=${args.dsf.join(",")}`,
  );

  killStraySprout();
  await sleep(1500);

  const child = launch(args.mode, port);
  let cdp = null;
  let infraError = null;
  const openedGroups = [];
  const report = [];

  try {
    const tBoot = Date.now();
    const { cdp: mainCdp, target } = await connectMainWindow(port, bootTimeout);
    console.log(`boot: ${((Date.now() - tBoot) / 1000).toFixed(1)}s target=${target.url}`);

    cdp = mainCdp;
    await cdp.send("Page.enable");
    console.log("main window ready");

    const displayModes = previewSupported
      ? [
          { label: "1 display", query: "" },
          { label: "2 displays", query: "?preview-per-monitor=1" },
        ]
      : [{ label: "physical displays", query: "" }];

    for (const displayMode of displayModes) {
      if (displayMode.query) {
        // Dev-only two-display preview: the page reads it on mount, so a
        // reload is required. Navigating the live SPA (rail click) loads the
        // route; the query swaps the resolved display list.
        await cdp.eval(`location.search = ${JSON.stringify(displayMode.query)}`);
      } else if (!(await cdp.eval(`location.pathname === "/settings"`))) {
        const clicked = await cdp.eval(`(() => {
          const a = document.querySelector('a.rail__item[href="/settings"]');
          if (!a) return false;
          a.click();
          return true;
        })()`);
        if (!clicked) throw new Error("settings nav anchor not found");
      }
      await waitFor(cdp, `!!document.querySelector(".settings") && location.pathname === "/settings"`, 60_000, "settings page");
      // A remembered collapsed section would hide the knobs under test;
      // open the closed ones through their own disclosures and restore after.
      for (const id of await openClosedGroups(cdp)) {
        if (!openedGroups.includes(id)) openedGroups.push(id);
      }
      await waitFor(cdp, `!!document.querySelector("#dock-width")`, 20_000, "dock width knob");
      await sleep(500);
      await injectLegacyRow(cdp, args.legacyRow);

      if (displayMode.query) {
        await waitFor(cdp, `!!document.querySelector(".per-monitor article.knob")`, 20_000, "per-monitor cards");
      }

      const baseline = await cdp.eval(MEASURE);
      // Restore the stored mode word ("System"/"Light"/"Dark"), not the
      // resolved theme, so a System preference stays System.
      const restoreTheme = (await activeTheme(cdp)) ?? (baseline.theme === "dark" ? "Dark" : "Light");

      for (const dsf of args.dsf) {
        await cdp.send("Emulation.setDeviceMetricsOverride", {
          width: args.viewport,
          height: 800,
          deviceScaleFactor: dsf,
          mobile: false,
        });
        await sleep(150);

        for (const themeLabel of ["Light", "Dark"]) {
          if (!(await setTheme(cdp, themeLabel))) throw new Error("theme picker not found");
          await sleep(250);

          for (const [cardWidth, passLabel] of [
            [args.card, `card=${args.card}px`],
            [null, `viewport=${args.viewport}px`],
          ]) {
            await constrainCard(cdp, cardWidth);
            await sleep(200);
            const m = await cdp.eval(MEASURE);

            const expected = displayMode.query ? 2 : null;
            const missing = expected !== null && m.cards.length !== expected;
            const overflow = m.cards.some((card) => card.overflow) || missing;
            // A single display has no per-monitor card; the global dock-width
            // card must still sit inside its column at the same width.
            const globalOverflow = cardWidth !== null && m.global ? m.global.overflow : false;
            const red = overflow || globalOverflow;

            report.push({
              displays: displayMode.label,
              dpi: `${Math.round(dsf * 100)}%`,
              theme: themeLabel.toLowerCase(),
              pass: passLabel,
              cardCount: m.cards.length,
              width: m.cards[0]?.width ?? m.global?.width ?? 0,
              red,
            });

            const detail = m.cards.length
              ? m.cards
                  .map(
                    (c) =>
                      `card[${c.width}px content=${c.contentWidth}px dir=${c.direction} scroll=${c.scrollWidth}/${c.clientWidth}` +
                      (c.escaping.length
                        ? ` ESCAPING=${c.escaping.map((e) => `${e.name}@${e.left}-${e.right}`).join(",")}`
                        : ""),
                  )
                  .join(" | ")
              : `no per-monitor card; global[${m.global?.width ?? "?"}px scroll=${m.global?.scrollWidth ?? "?"}/${m.global?.clientWidth ?? "?"}]`;

            console.log(
              `${displayMode.label} dpi=${Math.round(dsf * 100)}% theme=${themeLabel.toLowerCase()} ${passLabel} cards=${m.cards.length} ${detail} ${red ? "RED" : "ok"}`,
            );

            if (
              args.shots &&
              displayMode.query &&
              cardWidth === args.card &&
              dsf === args.dsf[args.dsf.length - 1]
            ) {
              await capture(
                cdp,
                args.shots,
                `per-monitor-${themeLabel.toLowerCase()}-${Math.round(dsf * 100)}pct.png`,
              );
            }
          }
        }

        await constrainCard(cdp, null);
        await setTheme(cdp, restoreTheme);
      }

      // Put the theme back the way the session found it.
      await cdp.send("Emulation.clearDeviceMetricsOverride");
      await setTheme(cdp, restoreTheme);
    }
  } catch (e) {
    infraError = e;
  } finally {
    // Put the Settings sections back the way the session found them.
    if (openedGroups.length > 0 && cdp && !infraError) {
      try {
        await restoreGroups(cdp, openedGroups);
      } catch {}
    }
    if (infraError) {
      console.error(`INFRA ERROR: ${infraError.message}`);
      if (child.tail) console.error(child.tail.slice(-10).join(""));
      process.exitCode = 2;
    } else {
      const red = report.some((r) => r.red);
      for (const r of report) {
        console.log(
          `${r.displays} ${r.dpi} ${r.theme} ${r.pass} cards=${r.cardCount} width=${r.width}px ${r.red ? "RED — horizontal overflow" : "green"}`,
        );
      }
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
