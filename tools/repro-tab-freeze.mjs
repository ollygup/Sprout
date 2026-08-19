import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import net from "node:net";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const USAGE = `Usage: node tools/repro-tab-freeze.mjs [options]

Drives the Sprout app via CDP and asserts every nav-tab click lands.
Verdict RED when a click never lands within budget or a main-thread
stall gap exceeds the threshold. Exit code: 0=green 1=red 2=infra.

Options:
  --mode exe|dev   app under test (default exe; dev spawns npm run tauri dev)
  --reps N         full nav sweeps (default 1)
  --budget ms      per-click landing budget (default 5000)
  --stall ms       stall threshold (default 1000)
  --delay ms       settle delay between clicks (default 400)
  --sample ms      stall sampler interval (default 100)
  --target path    click only this tab once (default: full sweep)
  --targets a,b,c  click these tabs in order, once each
  --ipc-probe      time the list IPC round trips on boot, then exit
  --port P         CDP port (default: first free of 9222,9333,9444,9555,9666)
  --keep           leave the app running after the verdict (debugging only)
  --help`;

function parseArgs(argv) {
  const args = { mode: "exe", reps: 1, budget: 5000, stall: 1000, delay: 400, sample: 100, port: null, target: null, targets: null, ipcProbe: false, keep: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--help": console.log(USAGE); process.exit(0);
      case "--keep": args.keep = true; break;
      case "--mode": args.mode = argv[++i]; break;
      case "--reps": args.reps = Number(argv[++i]); break;
      case "--budget": args.budget = Number(argv[++i]); break;
      case "--stall": args.stall = Number(argv[++i]); break;
      case "--delay": args.delay = Number(argv[++i]); break;
      case "--sample": args.sample = Number(argv[++i]); break;
      case "--target": args.target = argv[++i]; break;
      case "--targets": args.targets = argv[++i].split(","); break;
      case "--ipc-probe": args.ipcProbe = true; break;
      case "--port": args.port = Number(argv[++i]); break;
      default: throw new Error(`unknown option: ${a}\n${USAGE}`);
    }
  }
  return args;
}

const SWEEP = ["/presets", "/plan", "/history", "/logs", "/settings", "/"];

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
    this.events = [];
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id) {
        const p = this.pending.get(msg.id);
        if (!p) return;
        this.pending.delete(msg.id);
        if (msg.error) p.reject(new Error(msg.error.message));
        else p.resolve(msg.result);
      } else {
        this.events.push(msg);
      }
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
        "eval threw: " +
          (r.exceptionDetails.exception?.description ?? r.exceptionDetails.text),
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
        (t) =>
          t.type === "page" &&
          t.url !== "about:blank" &&
          !t.url.startsWith("devtools://"),
      );
      if (target) return target;
    } catch (e) {
      lastErr = e;
    }
    await sleep(250);
  }
  throw new Error(`no CDP page target on :${port} within ${timeoutMs}ms (${lastErr ?? "no endpoint"})`);
}

function startSampler(cdp, intervalMs) {
  const samples = [];
  const timer = setInterval(() => {
    const wallSend = Date.now();
    cdp
      .eval("performance.now()")
      .then((perf) => samples.push({ wallSend, wallRecv: Date.now(), perf }))
      .catch(() => {});
  }, intervalMs);
  timer.unref();
  return {
    samples,
    stop() {
      clearInterval(timer);
    },
  };
}

function stallsFrom(samples, thresholdMs) {
  const stalls = [];
  let episode = null;
  for (const s of samples) {
    const latency = s.wallRecv - s.wallSend;
    if (latency > thresholdMs) {
      if (!episode) episode = { from: s.wallSend, to: s.wallRecv, max: latency };
      else {
        episode.to = s.wallRecv;
        episode.max = Math.max(episode.max, latency);
      }
    } else if (episode) {
      stalls.push(episode);
      episode = null;
    }
  }
  if (episode) stalls.push(episode);
  return stalls;
}

async function clickTab(cdp, href, budgetMs, stallMs, sampleMs, delayMs, report) {
  const rect = await cdp.eval(`(() => {
    const a = document.querySelector('a.rail__item[href=${JSON.stringify(href)}]');
    if (!a) return null;
    const r = a.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  })()`);
  if (!rect) throw new Error(`nav anchor not found for ${href}`);

  const t0 = Date.now();
  const sampler = startSampler(cdp, sampleMs);
  try {
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: rect.x,
      y: rect.y,
    });
    await sleep(30);
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mousePressed",
      x: rect.x,
      y: rect.y,
      button: "left",
      buttons: 1,
      clickCount: 1,
    });
    await sleep(30);
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseReleased",
      x: rect.x,
      y: rect.y,
      button: "left",
      buttons: 0,
      clickCount: 1,
    });

    let last = null;
    while (Date.now() - t0 < budgetMs) {
      try {
        last = await cdp.eval(`(() => ({
          path: location.pathname,
          active: document.querySelector('.rail__item.active')?.getAttribute('href') ?? null
        }))()`);
        if (last && last.path === href && last.active === href) {
          const landMs = Date.now() - t0;
          const stalls = stallsFrom(sampler.samples, stallMs);
          report.clicks.push({ href, start: t0, landMs, ok: true, stalls, state: last });
          return;
        }
      } catch {}
      await sleep(40);
    }
    const landMs = Date.now() - t0;
    const stalls = stallsFrom(sampler.samples, stallMs);
    report.clicks.push({ href, start: t0, landMs, ok: false, stalls, state: last });
  } finally {
    sampler.stop();
  }
  await sleep(delayMs);
}

function extractLog(cdp) {
  const out = [];
  for (const ev of cdp.events) {
    if (ev.method === "Runtime.exceptionThrown") {
      const d = ev.params.exceptionDetails;
      out.push({
        kind: "exception",
        text: d.exception?.description ?? d.text,
        url: d.url,
        line: d.lineNumber,
      });
    } else if (ev.method === "Runtime.consoleAPICalled") {
      const text = ev.params.args
        .map((a) => a.value ?? a.description ?? a.type)
        .join(" ");
      out.push({ kind: "console", type: ev.params.type, text });
    } else if (ev.method === "Log.entryAdded") {
      const e = ev.params.entry;
      out.push({ kind: "log", level: e.level, text: e.text, url: e.url });
    }
  }
  return out;
}

function verdict(report, stallMs, budgetMs) {
  const bad = report.clicks.filter(
    (c) => !c.ok || c.landMs > budgetMs || c.stalls.length > 0,
  );
  return { red: bad.length > 0, bad };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const port = await pickFreePort(args.port);
  const bootTimeout = args.mode === "dev" ? 600_000 : 30_000;

  console.log(
    `repro-tab-freeze mode=${args.mode} port=${port} reps=${args.reps} budget=${args.budget}ms stall=${args.stall}ms delay=${args.delay}ms sample=${args.sample}ms`,
  );

  const child = launch(args.mode, port);
  const report = { clicks: [] };
  let cdp = null;
  let infraError = null;

  try {
    const tBoot = Date.now();
    const target = await waitForPageTarget(port, bootTimeout);
    console.log(`boot: ${((Date.now() - tBoot) / 1000).toFixed(1)}s target=${target.url}`);

    cdp = new Cdp(target.webSocketDebuggerUrl);
    await cdp.connect();
    await cdp.send("Runtime.enable");
    await cdp.send("Page.enable");
    await cdp.send("Log.enable");

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
    const settled = await cdp.eval(
      `new Promise(r => setTimeout(() => r(document.querySelector('.rail__item.active')?.getAttribute('href') ?? null), 1500))`,
    );
    console.log(`app ready, active tab=${settled}`);

    if (args.ipcProbe) {
      const names = ["list_products", "list_presets", "list_runs", "get_settings", "list_logs"];
      for (const name of names) {
        const ms = await cdp.eval(`(async () => {
          const t0 = performance.now();
          await window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)});
          return Math.round(performance.now() - t0);
        })()`);
        console.log(`ipc ${name.padEnd(16)} ${ms}ms`);
      }
    } else {
      const sweep = args.target ? [args.target] : args.targets ? args.targets : SWEEP;
      for (let rep = 0; rep < args.reps; rep++) {
        for (const href of sweep) {
          await clickTab(cdp, href, args.budget, args.stall, args.sample, args.delay, report);
        }
      }
    }
  } catch (e) {
    infraError = e;
  } finally {
    const { red, bad } = verdict(report, args.stall, args.budget);
    for (const c of report.clicks) {
      const stallNote = c.stalls.length
        ? ` STALLS: ${c.stalls.map((s) => `${s.max}ms@+${s.from - c.start}ms`).join(", ")}`
        : "";
      console.log(
        `click ${c.href.padEnd(12)} ${c.ok ? "landed" : "NEVER LANDED"} ${c.landMs}ms${stallNote}`,
      );
    }

    if (infraError) {
      console.error(`INFRA ERROR: ${infraError.message}`);
      if (child.tail) console.error(child.tail.slice(-10).join(""));
      process.exitCode = 2;
    } else {
      const log = extractLog(cdp);
      if (log.length) {
        console.log("--- captured console/errors ---");
        for (const l of log) {
          console.log(`[${l.kind}/${l.type ?? l.level ?? ""}] ${l.text}${l.url ? ` (${l.url}:${l.line ?? ""})` : ""}`);
        }
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