import assert from "node:assert/strict";
import { createFixtureServer } from "./companion-navigation-fixture/server.mjs";

const SESSION_COOKIE = "companion_fixture_session=present";
const serve = process.argv.includes("--serve");
const portArgument = process.argv.find((value) => value.startsWith("--port="));
const port = portArgument ? Number(portArgument.slice("--port=".length)) : serve ? 43171 : 0;

if (!Number.isInteger(port) || port < 0 || port > 65535) {
  throw new Error("--port must be an integer from 0 through 65535");
}

const fixture = createFixtureServer({ port });
const baseUrl = await fixture.start();

if (serve) {
  process.stdout.write(`Companion navigation fixture: ${baseUrl}/\n`);
  process.stdout.write(`Request evidence: ${baseUrl}/events\n`);
  process.stdout.write("Stop with Ctrl+C. This HTTP origin is for fixture inspection; Sprout accepts saved HTTPS origins only.\n");
  const stop = async () => {
    await fixture.stop();
    process.exit(0);
  };
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
  await new Promise(() => {});
}

const passes = [];
const pass = (name) => {
  passes.push(name);
  process.stdout.write(`PASS ${name}\n`);
};

try {
  const root = await fetch(`${baseUrl}/`);
  const rootBody = await root.text();
  assert.equal(root.status, 200);

  assert.match(rootBody, /id="fragment-link" href="#fragment-target"/);
  assert.match(rootBody, /id="history-route"/);
  assert.match(rootBody, /history\.pushState\(\{ fixture: "history" \}, "", "\/history-state"\)/);
  pass("same-document fragment and history contracts");

  const full = await fetch(`${baseUrl}/full`);
  assert.equal(full.status, 200);
  assert.match(await full.text(), /id="full-marker">full-document/);
  pass("full navigation contract");

  const redirectStart = await fetch(`${baseUrl}/redirect/start`, { redirect: "manual" });
  assert.equal(redirectStart.status, 302);
  assert.equal(redirectStart.headers.get("location"), "/redirect/middle");
  const redirectMiddle = await fetch(`${baseUrl}/redirect/middle`, { redirect: "manual" });
  assert.equal(redirectMiddle.status, 307);
  assert.equal(redirectMiddle.headers.get("location"), "/redirect/final?via=two-hop");
  const redirectFinal = await fetch(`${baseUrl}/redirect/final?via=two-hop`);
  assert.match(await redirectFinal.text(), /id="redirect-marker">two-hop/);
  pass("redirect chain contract");

  assert.match(rootBody, /id="blank-popup"[^>]*target="_blank"/);
  assert.match(rootBody, /window\.open\("\/popup-target\?source=window-open", "fixture-popup"\)/);
  for (const source of ["_blank", "window-open"]) {
    const popup = await fetch(`${baseUrl}/popup-target?source=${encodeURIComponent(source)}`);
    assert.match(await popup.text(), new RegExp(`id="popup-marker">${source.replace("_", "_")}`));
  }
  const injection = '<img src=x onerror="fixture-injection">';
  const popupInjection = await fetch(`${baseUrl}/popup-target?source=${encodeURIComponent(injection)}`);
  const popupInjectionBody = await popupInjection.text();
  assert.doesNotMatch(popupInjectionBody, /<img src=x/);
  assert.match(popupInjectionBody, /&lt;img src=x onerror=&quot;fixture-injection&quot;&gt;/);
  const redirectInjection = await fetch(`${baseUrl}/redirect/final?via=${encodeURIComponent(injection)}`);
  const redirectInjectionBody = await redirectInjection.text();
  assert.doesNotMatch(redirectInjectionBody, /<img src=x/);
  assert.match(redirectInjectionBody, /&lt;img src=x onerror=&quot;fixture-injection&quot;&gt;/);
  pass("target=_blank and window.open contracts");

  const authStart = await fetch(`${baseUrl}/auth/start`, { redirect: "manual" });
  assert.equal(authStart.status, 302);
  assert.equal(authStart.headers.get("location"), "/auth/login?return=%2Fauth%2Freturn");
  const cookie = authStart.headers.get("set-cookie");
  assert.ok(cookie?.includes("companion_fixture_session=present"));
  const authLogin = await fetch(`${baseUrl}/auth/login?return=%2Fauth%2Freturn`);
  assert.match(await authLogin.text(), /id="auth-return"/);
  const authReturn = await fetch(`${baseUrl}/auth/return?state=fixture-state&code=fixture-code`, {
    headers: { cookie: SESSION_COOKIE },
  });
  assert.match(await authReturn.text(), /id="auth-marker">authenticated/);
  const externalProfileReturn = await fetch(`${baseUrl}/auth/return?state=fixture-state&code=fixture-code`);
  assert.match(await externalProfileReturn.text(), /id="auth-marker">profile-or-state-mismatch/);
  pass("auth return and profile separation contracts");

  assert.match(rootBody, /href="sprout-fixture:\/\/handoff\?case=custom-protocol"/);
  pass("custom-protocol handoff contract");

  await assert.rejects(fetch(`${baseUrl}/failure/abort`));
  pass("aborted transport failure contract");

  const events = await (await fetch(`${baseUrl}/events`)).json();
  const eventPaths = events.map((event) => event.path);
  for (const expected of [
    "/full",
    "/redirect/start",
    "/redirect/middle",
    "/redirect/final",
    "/popup-target",
    "/auth/start",
    "/auth/login",
    "/auth/return",
    "/failure/abort",
  ]) {
    assert.ok(eventPaths.includes(expected), `missing request evidence for ${expected}`);
  }
  assert.ok(!eventPaths.includes("/history-state"), "pushState unexpectedly became a server request");
  pass("request-evidence contract");

  process.stdout.write(`RESULT ${passes.length}/${passes.length} controlled contracts passed\n`);
} finally {
  await fixture.stop();
}
