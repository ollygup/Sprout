# Companion navigation fixture

This deterministic local fixture separates eight navigation behaviors that can
look identical when a host has no live-location or navigation feedback:

1. same-document fragment and `history.pushState` routes;
2. full document navigation;
3. a two-hop HTTP redirect;
4. `target=_blank` and `window.open` requests;
5. an auth-shaped redirect, cookie, and return;
6. a custom-protocol handoff;
7. an aborted transport load; and
8. server-side request evidence.

Run the unattended contract check from the repository root:

```powershell
node tools/repro-companion-navigation.mjs
```

Run the fixture for manual inspection:

```powershell
node tools/repro-companion-navigation.mjs --serve
```

The server binds only to `127.0.0.1`; `/events` reports requests received by
the fixture. A fragment change or `pushState` must not add a request. Full
navigations, redirects, popup targets that actually open, auth pages, and the
aborted-load attempt do add requests.

The local server is HTTP by design and is not a saved Companion site: Sprout's
production validation accepts HTTPS only. Native testing must put this exact
fixture behind a trusted HTTPS test origin and use an isolated Sprout app-data
directory. Do not weaken production URL validation or add this HTTP origin to a
real profile for the sake of the investigation.
