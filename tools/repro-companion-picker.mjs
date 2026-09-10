import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { svelte, vitePreprocess } from "@sveltejs/vite-plugin-svelte";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { chromium } = await import(process.env.SPROUT_PLAYWRIGHT_MODULE || "playwright");
const mockCore = `
export * from '/node_modules/@tauri-apps/api/core.js';
const sites = [
  { name: 'Work', url: 'https://work.example/', ua: 'desktop' },
  { name: 'Music', url: 'https://music.example/', ua: 'mobile' },
];
let active = sites[0].url;
export async function invoke(command, args) {
  switch (command) {
    case 'get_settings': return {
      theme: 'light', companion_url: active, companion_url_list: sites,
      companion_height_ratio: 0.4, dock_density: 'default',
    };
    case 'get_quick_launch_dock_state': return {
      docked: true, edge: 'left', mode: 'fixed', blocked: null,
      left_eligible: true, right_eligible: true, monitor: 'fixture',
    };
    case 'list_displays': return [{ device_name: 'fixture' }];
    case 'get_companion_height_ratio': return null;
    case 'get_companion_audio_state': return { muted: false, playing: false };
    case 'set_companion_url': active = args.url; return;
    default:
      if (command.startsWith('list_')) return [];
      throw new Error('Unexpected fixture command: ' + command);
  }
}
`;
const modules = {
  'fixture:core': mockCore,
  'fixture:event': `export async function listen() { return () => {}; }`,
};
const server = await createServer({
  root,
  configFile: false,
  logLevel: "error",
  plugins: [
    {
      name: "companion-picker-fixture",
      enforce: "pre",
      resolveId(id) {
        if (id === "@tauri-apps/api/core") return "fixture:core";
        if (id === "@tauri-apps/api/event") return "fixture:event";
      },
      load(id) { return modules[id]; },
      configureServer(vite) {
        vite.middlewares.use("/picker-fixture", async (_request, response) => {
          response.setHeader("Content-Type", "text/html");
          response.end(await vite.transformIndexHtml("/picker-fixture", `<!doctype html><html><body><div id="app"></div>
            <script type="module">
              import { mount } from 'svelte';
              import Page from '/src/routes/quick-launch-window/+page.svelte';
              import '/src/lib/styles/tokens.css';
              mount(Page, { target: document.querySelector('#app') });
            </script></body></html>`));
        });
      },
    },
    svelte({ configFile: false, preprocess: vitePreprocess() }),
  ],
  resolve: { alias: { $lib: path.join(root, "src/lib") } },
  server: { host: "127.0.0.1", port: 0 },
});

let browser;
const failures = [];
function check(name, run) {
  try {
    run();
    console.log(`PASS ${name}`);
  } catch (error) {
    failures.push(name);
    console.error(`FAIL ${name}: ${error.message}`);
  }
}

try {
  await server.listen();
  browser = await chromium.launch({ channel: "msedge", headless: true });
  const page = await browser.newPage({ viewport: { width: 420, height: 900 } });
  const pageErrors = [];
  page.on("pageerror", (error) => {
    pageErrors.push(error.message);
    console.error(error.message);
  });
  await page.route("https://*.example/**", (route) => route.fulfill({
    contentType: "text/html",
    body: "<!doctype html><title>Companion fixture</title><p>Saved site content</p>",
  }));
  await page.goto(`${server.resolvedUrls.local[0]}picker-fixture`, { waitUntil: "domcontentloaded" });
  const trigger = page.locator(".qlw__companion-site-trigger");
  const frame = page.locator(".qlw__companion-frame-wrap");
  const menu = page.getByRole("menu", { name: "Saved Companion sites" });
  await trigger.waitFor();
  const settle = () => page.evaluate(() => new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await settle();

  const initial = await frame.boundingBox();
  await trigger.click();
  await menu.waitFor();
  await settle();
  const opened = await frame.boundingBox();
  const position = await menu.evaluate((element) => getComputedStyle(element).position);
  console.log(JSON.stringify({ initial, opened, menuPosition: position }));
  check("opening the picker preserves the site frame", () => assert.deepEqual(opened, initial));
  check("the picker floats above the site layout", () => assert.equal(position, "fixed"));
  await trigger.click();
  await settle();
  const stillOpen = await menu.isVisible();
  check("clicking the trigger again closes the picker", () =>
    assert.equal(stillOpen, false));

  for (const target of [".qlw__companion-site-text", ".qlw__companion-site-chevron svg"]) {
    await page.keyboard.press("Escape");
    await trigger.locator(target).click();
    await settle();
    const isOpen = await menu.isVisible();
    await trigger.locator(target).click();
    await settle();
    const isClosed = !(await menu.isVisible());
    check(`repeated clicks on ${target} toggle once each`, () => {
      assert.equal(isOpen, true);
      assert.equal(isClosed, true);
    });
  }
  check("no browser runtime errors", () => assert.deepEqual(pageErrors, []));
  if (process.env.SPROUT_PICKER_SCREENSHOT) {
    await page.keyboard.press("Escape");
    await trigger.click();
    await settle();
    await page.screenshot({ path: process.env.SPROUT_PICKER_SCREENSHOT });
  }
  assert.equal(failures.length, 0, failures.join("; "));
} finally {
  await browser?.close();
  await server.close();
}
