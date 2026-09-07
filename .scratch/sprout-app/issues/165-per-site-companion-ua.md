# 165 — Per-site Companion Desktop/Mobile identity

**What to build:** Each saved Companion site carries a Mobile/Desktop switch (default Mobile) so desktop-only sites like Teams load instead of serving their mobile block page — with the Chromium token refreshed at build time.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

## Scope

- Site shape grows the preference with tolerant reads (missing = Mobile); create/edit UI exposes the switch; child creation passes the site's UA; refresh the stale Chrome/131 pin, Edge variant preferred since Teams first-classes Edge; Open-externally stays the fallback.
- Glossary update for the site preference rides here.
- Explicitly not built: per-load UA editing, free-text UA, any change to isolation (own profile, no bridge) or docked-only visibility.

## ACs

- [x] Default and legacy sites behave exactly as today (Mobile); a Desktop site loads desktop-only pages.
- [x] The `Windows NT 10.0` token is used for Desktop (covers current Windows releases; Win11 differs only via Client Hints).
- [x] `npm.cmd run check` 0 errors; related settings/companion tests green.

## Verification

- `npm.cmd run check`, settings/companion tests; manual: Teams-type site on Desktop vs Mobile, legacy-site migration.
