# 18 — Run awareness

**What to build:** Runs stop being invisible the moment the user leaves the Plan page, and consoles stop flashing. All subprocess spawns gain `CREATE_NO_WINDOW` — no cmd/powershell windows appear during a run (the run logs are the record). A run-active query powers a layout-level banner shown on every page while a run is in progress (progress + cancel), replacing page-local polling as the source of truth. On completion, a Windows toast announces the outcome (notification plugin) alongside the in-app notice.

**Blocked by:** 11 — App shell and design foundation; 16 — Honest run outcomes

**Status:** done

- [x] All installer/subprocess spawns use `CREATE_NO_WINDOW`; no console windows flash during any run
- [x] Run-active query exposes whether a run is in progress (and its progress) from anywhere; a layout banner renders on every page while a run is live, with progress and cancel; it survives navigation
- [x] Windows toast on run completion with the outcome (Applied / With notes / Cancelled / Failed); in-app notice also shown; no duplicate toasts on repeat runs; toast text uses the user-facing outcome labels
- [x] Banner and cancel behavior verified while on other pages (Products, Presets, History)
- [x] Backend tests cover the run-active query states; `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok

**Verification notes (2026-08-16):** AC1/AC3 code-reviewed; AC2/AC5 gates green (`cargo test --lib` 159 passed, `svelte-check` 0/0, `cargo check` clean, `npm run build` ok); AC4 smoke-tested in the dev window via WebView2 CDP on the Products page with a planted fake run (`run-1786863143140`): `get_active_run` IPC returned the run, the layout banner rendered "Run in progress — Git — step 1 of 3 (install)" with Cancel, clicking Cancel flipped the banner to "Cancelling after this step…" and the backend wrote the `cancel` marker. Dev-mode limitation: Windows toast visually unverifiable without an installed AUMID shortcut; covered by the in-app banner notice and a swallowed-failure notification path.