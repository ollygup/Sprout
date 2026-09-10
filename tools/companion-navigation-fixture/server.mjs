import { createServer } from "node:http";

const SESSION_COOKIE = "companion_fixture_session=present";

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function page(title, body, script = "") {
  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>${title}</title>
    <style>
      body { font: 16px/1.45 system-ui, sans-serif; margin: 24px; max-width: 48rem; }
      nav { display: grid; gap: 12px; }
      button, a { width: fit-content; }
      output { display: block; margin-block: 16px; padding: 8px; background: #eef3f8; }
      #fragment-target { margin-top: 70vh; }
    </style>
  </head>
  <body>
    <h1>${title}</h1>
    ${body}
    ${script ? `<script>${script}</script>` : ""}
  </body>
</html>`;
}

const rootPage = page(
  "Companion navigation fixture",
  `<p id="fixture-version">fixture-version: 1</p>
  <output id="location-readout"></output>
  <nav aria-label="Controlled navigation cases">
    <a id="fragment-link" href="#fragment-target">Same-document fragment</a>
    <button id="history-route" type="button">History pushState route</button>
    <a id="full-navigation" href="/full">Full document navigation</a>
    <a id="redirect-navigation" href="/redirect/start">Two-hop redirect</a>
    <a id="blank-popup" href="/popup-target?source=_blank" target="_blank">target=_blank popup</a>
    <button id="window-open-popup" type="button">window.open popup</button>
    <a id="auth-start" href="/auth/start">Auth-style redirect and return</a>
    <a id="protocol-handoff" href="sprout-fixture://handoff?case=custom-protocol">Custom-protocol handoff</a>
    <a id="load-failure" href="/failure/abort">Aborted transport load</a>
  </nav>
  <h2 id="fragment-target">Fragment target</h2>`,
  `const readout = document.querySelector("#location-readout");
  const update = (kind) => { readout.textContent = kind + ": " + location.href; };
  addEventListener("hashchange", () => update("hashchange"));
  addEventListener("popstate", () => update("popstate"));
  document.querySelector("#history-route").addEventListener("click", () => {
    history.pushState({ fixture: "history" }, "", "/history-state");
    update("pushState");
  });
  document.querySelector("#window-open-popup").addEventListener("click", () => {
    window.open("/popup-target?source=window-open", "fixture-popup");
  });
  update("load");`,
);

function send(res, status, body, headers = {}) {
  res.writeHead(status, {
    "cache-control": "no-store",
    "content-type": "text/html; charset=utf-8",
    ...headers,
  });
  res.end(body);
}

export function createFixtureServer({ host = "127.0.0.1", port = 0 } = {}) {
  const events = [];
  const server = createServer((req, res) => {
    const origin = `http://${req.headers.host ?? `${host}:${port}`}`;
    const url = new URL(req.url ?? "/", origin);

    if (url.pathname === "/events") {
      res.writeHead(200, {
        "cache-control": "no-store",
        "content-type": "application/json; charset=utf-8",
      });
      res.end(JSON.stringify(events, null, 2));
      return;
    }

    events.push({ method: req.method ?? "GET", path: url.pathname, search: url.search });

    if (url.pathname === "/failure/abort") {
      req.socket.destroy();
      return;
    }

    if (url.pathname === "/") {
      send(res, 200, rootPage);
      return;
    }

    if (url.pathname === "/full") {
      send(res, 200, page("Full navigation complete", '<p id="full-marker">full-document</p><a href="/">Return</a>'));
      return;
    }

    if (url.pathname === "/redirect/start") {
      send(res, 302, "redirect-start", { location: "/redirect/middle" });
      return;
    }

    if (url.pathname === "/redirect/middle") {
      send(res, 307, "redirect-middle", { location: "/redirect/final?via=two-hop" });
      return;
    }

    if (url.pathname === "/redirect/final") {
      const via = escapeHtml(url.searchParams.get("via") ?? "missing");
      send(res, 200, page("Redirect complete", `<p id="redirect-marker">${via}</p><a href="/">Return</a>`));
      return;
    }

    if (url.pathname === "/popup-target") {
      const source = escapeHtml(url.searchParams.get("source") ?? "missing");
      send(res, 200, page("Popup target", `<p id="popup-marker">${source}</p>`));
      return;
    }

    if (url.pathname === "/auth/start") {
      send(res, 302, "auth-start", {
        location: "/auth/login?return=%2Fauth%2Freturn",
        "set-cookie": `${SESSION_COOKIE}; Path=/; HttpOnly; SameSite=Lax`,
      });
      return;
    }

    if (url.pathname === "/auth/login") {
      send(
        res,
        200,
        page(
          "Fixture sign-in",
          `<form method="get" action="/auth/return">
            <input type="hidden" name="state" value="fixture-state">
            <input type="hidden" name="code" value="fixture-code">
            <button id="auth-return" type="submit">Complete sign-in</button>
          </form>`,
        ),
      );
      return;
    }

    if (url.pathname === "/auth/return") {
      const hasSession = (req.headers.cookie ?? "").includes(SESSION_COOKIE);
      const validState = url.searchParams.get("state") === "fixture-state";
      const validCode = url.searchParams.get("code") === "fixture-code";
      const result = hasSession && validState && validCode ? "authenticated" : "profile-or-state-mismatch";
      send(res, 200, page("Auth return", `<p id="auth-marker">${result}</p><a href="/">Return</a>`));
      return;
    }

    send(res, 404, page("Not found", `<p id="not-found-marker">${escapeHtml(url.pathname)}</p>`));
  });

  return {
    events,
    async start() {
      await new Promise((resolve, reject) => {
        server.once("error", reject);
        server.listen(port, host, resolve);
      });
      const address = server.address();
      if (!address || typeof address === "string") throw new Error("Fixture server did not expose a TCP address");
      return `http://${host}:${address.port}`;
    },
    async stop() {
      if (!server.listening) return;
      await new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
    },
  };
}
