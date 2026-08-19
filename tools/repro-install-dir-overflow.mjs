import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import net from "node:net";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function killStraySprout() {
  try {
    spawnSync("taskkill", ["/IM", "sprout.exe", "/F"], { stdio: "ignore" });
  } catch {}
}

const USAGE = `Usage: node tools/repro-install-dir-overflow.mjs [options]

Drives the Sprout app via CDP, forces viewport widths down to the window
minimum (900px), and asserts the install-directory text field on /settings
never extends past the application width. Verdict RED when the field's
right edge exceeds the viewport (the reported bug). Exit code: 0=green
1=red 2=infra.

Options:
  --mode exe|dev   app under test (default dev; dev spawns npm run tauri dev)
  --widths a,b,c   viewport widths to sweep (default 1200,1050,950,900,880,840)
  --inject css     inject a <style> rule before measuring (hypothesis probe)
  --port P         CDP port (default: first free of 9222,9333,9444,9555,9666)
  --keep           leave the app running after the verdict (debugging only)
  --help`;

function parseArgs(argv) {
  const args = { mode: "dev", widths: [1200, 1050, 950, 900, 880, 840], port: null, keep: false, inject: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--help": console.log(USAGE); process.exit(0);
      case "--keep": args.keep = true; break;
      case "--mode": args.mode = argv[++i]; break;
      case "--widths": args.widths = argv[++i].split(",").map(Number); break;
      case "--inject": args.inject = argv[++i]; break;
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

async function waitForPageTarget(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastErr = null;
  while (Date.now() < deadline) {
    try {
      const list = await fetchJson(`http://127.0.0.1:${port}/json`);
      const target = list.find(
        (t) => t.type === "page" && t.url !== "about:blank" && !t.url.startsWith("devtools://"),
      );
      if (target) return target;
    } catch (e) {
      lastErr = e;
    }
    await sleep(250);
  }
  throw new Error(`no CDP page target on :${port} within ${timeoutMs}ms (${lastErr ?? "no endpoint"})`);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const port = await pickFreePort(args.port);
  const bootTimeout = args.mode === "dev" ? 600_000 : 30_000;

  console.log(`repro-install-dir-overflow mode=${args.mode} port=${port} widths=${args.widths.join(",")}`);

  // A running Sprout instance shares the WebView2 user-data-folder browser
  // process, which swallows the CDP port argument (observed). Kill first so
  // the probe gets a clean browser process tree.
  killStraySprout();
  await sleep(1500);

  const child = launch(args.mode, port);
  let cdp = null;
  let infraError = null;
  const report = [];

  try {
    const tBoot = Date.now();
    const target = await waitForPageTarget(port, bootTimeout);
    console.log(`boot: ${((Date.now() - tBoot) / 1000).toFixed(1)}s target=${target.url}`);

    cdp = new Cdp(target.webSocketDebuggerUrl);
    await cdp.connect();
    await cdp.send("Runtime.enable");
    await cdp.send("Page.enable");

    const tRail = Date.now();
    while (Date.now() - tRail < 20_000) {
      try {
        if (await cdp.eval(`!!document.querySelector('.rail')`)) break;
      } catch {}
      await sleep(100);
    }
    if (!(await cdp.eval(`!!document.querySelector('.rail')`))) {
      throw new Error("nav rail never appeared");
    }
    console.log("app ready");

    const nav = await cdp.eval(`(() => {
      const a = document.querySelector('a.rail__item[href="/settings"]');
      if (!a) return null;
      const r = a.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    })()`);
    if (!nav) throw new Error("settings nav anchor not found");

    await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: nav.x, y: nav.y });
    await sleep(30);
    await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: nav.x, y: nav.y, button: "left", buttons: 1, clickCount: 1 });
    await sleep(30);
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: nav.x, y: nav.y, button: "left", buttons: 0, clickCount: 1 });

    let gotInput = false;
    const tIn = Date.now();
    while (Date.now() - tIn < 20_000) {
      try {
        if (await cdp.eval(`!!document.querySelector('#install-dir')`)) {
          gotInput = true;
          break;
        }
      } catch {}
      await sleep(100);
    }
    if (!gotInput) {
      const state = await cdp
        .eval(`(() => ({
          path: location.pathname,
          active: document.querySelector('.rail__item.active')?.getAttribute('href') ?? null,
          mainText: document.querySelector('#main')?.innerText.slice(0, 200) ?? null,
          settingsSection: !!document.querySelector('.settings'),
        }))()`)
        .catch(() => null);
      throw new Error(`install-dir input never appeared (settings load failed?) state=${JSON.stringify(state)}`);
    }
    if (args.inject) {
      await cdp.eval(`(() => {
        const s = document.createElement('style');
        s.id = 'hypothesis-probe';
        s.textContent = ${JSON.stringify(args.inject)};
        document.head.appendChild(s);
        return true;
      })()`);
      await sleep(150);
    }
    await sleep(500);

    const measure = `(() => {
      const input = document.querySelector('#install-dir');
      const ir = input.getBoundingClientRect();
      const main = document.querySelector('#main');
      const mr = main.getBoundingClientRect();
      const row = input.closest('.knob__input--wide');
      const rr = row.getBoundingClientRect();
      return {
        innerWidth: window.innerWidth,
        inputLeft: Math.round(ir.left * 10) / 10,
        inputRight: Math.round(ir.right * 10) / 10,
        inputWidth: Math.round(ir.width * 10) / 10,
        mainRight: Math.round(mr.right * 10) / 10,
        rowLeft: Math.round(rr.left * 10) / 10,
        rowRight: Math.round(rr.right * 10) / 10,
        rowWidth: Math.round(rr.width * 10) / 10,
        rowFlex: getComputedStyle(row).flex,
        knobWidth: Math.round(row.closest('.knob').getBoundingClientRect().width * 10) / 10,
        knobLeft: Math.round(row.closest('.knob').getBoundingClientRect().left * 10) / 10,
        knobRight: Math.round(row.closest('.knob').getBoundingClientRect().right * 10) / 10,
        bodyLeft: Math.round(row.closest('.knob').querySelector('.knob__body').getBoundingClientRect().left * 10) / 10,
        bodyRight: Math.round(row.closest('.knob').querySelector('.knob__body').getBoundingClientRect().right * 10) / 10,
        clearVisible: !!document.querySelector('.knob__input--wide .btn--ghost'),
        mainOverflowX: document.querySelector('#main').scrollWidth > document.querySelector('#main').clientWidth,
        inputFlex: getComputedStyle(input).flex,
        inputMinWidth: getComputedStyle(input).minWidth,
        inputWidthCss: getComputedStyle(input).width,
        buttons: [...document.querySelectorAll('.knob__input--wide .btn')].map((b) => {
          const r = b.getBoundingClientRect();
          return { text: b.textContent.trim(), left: Math.round(r.left * 10) / 10, right: Math.round(r.right * 10) / 10 };
        }),
      };
    })()`;

    for (const width of args.widths) {
      await cdp.send("Emulation.setDeviceMetricsOverride", {
        width,
        height: 620,
        deviceScaleFactor: 1,
        mobile: false,
      });
      await sleep(150);

      const empty = await cdp.eval(measure);

      const set = await cdp.eval(`(() => {
        const el = document.querySelector('#install-dir');
        const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
        setter.call(el, 'D:\\\\Apps\\\\Tooling');
        el.dispatchEvent(new Event('input', { bubbles: true }));
        return true;
      })()`);
      await sleep(150);
      const filled = await cdp.eval(measure);

      const exceeds = (m) => m.inputRight > m.innerWidth + 0.5 || m.inputRight > m.mainRight + 0.5;
      report.push({ width, empty, filled, red: exceeds(empty) || exceeds(filled) });

      const fmt = (m) =>
        `input[${m.inputWidth}px @${m.inputLeft}-${m.inputRight}] css(w=${m.inputWidthCss} min=${m.inputMinWidth} flex=${m.inputFlex})` +
        ` row[${m.rowWidth}px @${m.rowLeft}-${m.rowRight} flex=${m.rowFlex}] knob=${m.knobWidth}px@${m.knobLeft}-${m.knobRight} body=${m.bodyRight - m.bodyLeft}px@${m.bodyLeft}-${m.bodyRight}` +
        ` btns=[${m.buttons.map((b) => `${b.text}@${b.left}-${b.right}`).join(" ")}]` +
        ` main→${m.mainRight} innerW=${m.innerWidth} overflowX=${m.mainOverflowX ? "YES" : "no"} clear=${m.clearVisible ? "yes" : "no"}`;
      console.log(
        `width=${width}  empty: ${fmt(empty)}  filled: ${fmt(filled)}  ${report[report.length - 1].red ? "EXCEEDS" : "ok"}`,
      );

      await cdp.eval(`(() => {
        const el = document.querySelector('#install-dir');
        const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
        setter.call(el, '');
        el.dispatchEvent(new Event('input', { bubbles: true }));
        return true;
      })()`);
    }
    await cdp.send("Emulation.clearDeviceMetricsOverride");
  } catch (e) {
    infraError = e;
  } finally {
    if (infraError) {
      console.error(`INFRA ERROR: ${infraError.message}`);
      if (child.tail) console.error(child.tail.slice(-10).join(""));
      process.exitCode = 2;
    } else {
      const red = report.some((r) => r.red);
      for (const r of report) {
        console.log(
          `width=${r.width} ${r.red ? "RED — field exceeds application width" : "green"}`,
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
