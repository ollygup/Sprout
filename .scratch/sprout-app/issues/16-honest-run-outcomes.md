# 16 — Honest run outcomes

**What to build:** A run never reports success for requirements that didn't actually succeed. A "Needs attention" outcome tier covers unmanaged requirements (installed outside Sprout's control) — rendered as a visible issue, never tucked into a green "clean". winget failures, including "cannot find product" (wrong/stale ID), map to Failed with a clear user-facing message and appear in run results and logs — never as success. Outcome labels across the app become Applied / With notes / Cancelled / Failed, with status colors from the Notion semantic set (light + dark).

**Blocked by:** 11 — App shell and design foundation

**Status:** done

- [x] Unmanaged skips produce an attention state in plan and run output, visibly distinct from success; run outcome "clean" only when nothing needed attention
- [x] winget "no package found" and install errors map to Failed with a clear message ("can't find this app in the winget registry — check its ID" style), shown in run results and written to the run log
- [x] Outcome labels: Applied / With notes / Cancelled / Failed with Notion status colors in both themes; color never the only channel (text labels carry meaning)
- [x] Backend tests: install failure → Failed; no-package-found → Failed; unmanaged → attention; run outcome derivation covers all tiers
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok