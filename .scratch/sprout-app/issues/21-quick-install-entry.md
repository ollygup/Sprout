# 21 — Quick install entry and rendering

**What to build:** The user-facing half of quick install. "Install now" appears in the product context menu (from ticket 12's menu system). Choosing it routes into the Plan page as a single synthetic requirement — auto-validated, grouped, and run through the standard run stage exactly like a preset selection — then lands in History labeled "Quick install — {product}". No preset is created or required.

**Blocked by:** 17 — Quick install (backend); 19 — Plan page: auto-validate and grouped preview; 20 — Plan page: run stage

**Status:** done

- [x] "Install now" in the product context menu (keyboard-reachable like every menu item)
- [x] It opens the Plan page with the product as a single synthetic requirement — auto-validated and grouped like any selection; "will upgrade" is shown plainly when a newer policy would upgrade an existing install
- [x] The quick install runs through the standard run stage (promise line, check-then-act rows, cancel, results)
- [x] History labels it "Quick install — {product}" and it renders through the same outcome tiers
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok

**Verification notes (2026-08-16):** "Install now" leads the product context menu (first item, focus-first on keyboard open, play icon) and routes to `/plan?quick=<product id>`. The Plan page treats that as a quick-install visit: the new backend command `quick_install_plan` synthesizes the default Requirement from the Library Product (reusing ticket 17's `synthesize_quick_requirement` — latest policy, winget step, default env wiring) and composes it as a single-entry Plan named "Quick install — {product}", so the standard grouped rendering (Ready to apply / Already good / Needs attention, "will upgrade" badge from detection) shows it exactly like a preset selection. The run goes through the unchanged stage 3 — promise line, live check-then-act rows, cancel confirmation, grouped results — because `startRun` receives the composition's `preset_names` ("Quick install — {product}"), which History already renders through the same outcome tiers; no preset is created. URL state stays deep-linkable: `syncUrl` preserves `?quick=<id>&stage=…`; the preset picker and "Save as new preset" are hidden for the focused flow; "Check again" re-validates the product; a failed quick install (product gone or no winget step) drops out of quick mode with the backend's clear error and falls back to the normal preset pick. Gates: `svelte-check` 0/0, `cargo test --lib` 159 passed, `cargo check` clean, `npm run build` ok. Not smoke-tested in the dev window (no run was executed); logic mirrors the ticket-20 run path unchanged, verified by code review and gates.