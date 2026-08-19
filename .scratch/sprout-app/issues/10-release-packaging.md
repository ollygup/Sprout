# 10 — Release: installer, file association, size gate, parity, legacy removal

**What to build:** Shipping Sprout: an NSIS installer that places the exe and registers the `.sprout.json` file association (double-clicking a preset opens Sprout and imports it), a verified release build under ~10 MB, a parity smoke test against the legacy package on a clean machine, and finally removal of the legacy package. This ticket makes "install Sprout, double-click a preset from a teammate, apply it" work end to end.

**Blocked by:** 06 — Elevated run worker with live progress and cancel; 07 — Env wiring and verify commands; 08 — Command steps, winget bootstrap, unmanaged installs; 09 — History, Logs, and Settings screens

**Status:** done

- [x] NSIS installer installs the exe and registers the `.sprout.json` file association; double-clicking a preset opens Sprout and imports it — verified end-to-end: silent install → `%LOCALAPPDATA%\Programs\Sprout`, association command `...sprout.exe "%1"` in HKCU, installed exe launched with a `.sprout.json` argument imported it (presets 0 → 1). Vendored NSIS template fixes Tauri's install-dir/data-dir collision (see ADR-0006).
- [x] Release build is under ~10 MB — `sprout.exe` 4.62 MB, installer 1.85 MB (release profile: lto, opt-level s, strip, panic abort). Artifacts in `dist/`.
- [x] Parity smoke test: legacy run and Sprout run produce equivalent per-Requirement outcomes — VERDICT PASS on 2026-08-15 (device-based same-state comparison, accepted with caveat since no clean VM was available): git ↔ skipped_unmanaged, openjdk21 ↔ already_ok, dbeaver ↔ already_ok, `parity-compare.mjs` exit 0. Caveats + shared-heuristic finding (`vs_githubprotocolhandlermsi` false-positive) recorded in `docs/release/parity-checklist.md`; baseline log archived at `docs/release/clean-legacy-setup.log`.
- [x] `legacy/` is removed after parity passes; docs (CONTEXT.md, ADRs, spec) finalized and consistent — `legacy/` deleted 2026-08-15; CONTEXT/ADR-0006/ADR-0001/spec/README updated; catalog semantics preserved as `tools/parity-preset.sprout.json`; parity tooling kept (`tools/parity-compare.mjs`).
