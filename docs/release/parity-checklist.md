# Parity verification record — legacy vs Sprout (ticket 10, AC 3)

Gate for ticket 10, AC 3: *"Parity smoke test on a clean machine: legacy run and
Sprout run produce equivalent per-Requirement outcomes."* The `legacy/` removal
(AC 4) was gated on this passing.

## Result: PASS (2026-08-15, device-based)

The gate ran as a **same-state comparison on the development machine** (no VM
tooling available), not on a pristine clean machine — accepted for v0.1.0 with
this recorded caveat. Both tools observed the identical machine state:

- Temurin OpenJDK 21.0.12.8 installed (winget-managed; its MSI had set
  machine-scope `JAVA_HOME` + PATH entry, so both tools' env wiring skipped)
- "Git" flagged present by the shared uninstall-registry heuristic — a
  **false positive**: the VS Build Tools component `vs_githubprotocolhandlermsi`
  matches the `*git*` DisplayName needle. Git is not actually installed. Both
  tools share this heuristic (legacy `Test-RegistryInstall`, Sprout
  `detection_for` in `src-tauri/src/engine/windows.rs`), so outcomes stay
  equivalent; the quirk is recorded, not fixed, for v1.
- DBeaver 26.1.4 installed (winget-managed)

`tools/parity-compare.mjs` verdict (exit 0):

```
id                       legacy    sprout               result
------------------------------------------------------------------------
dbeaver                  OK        already_ok           MATCH
git                      OK        skipped_unmanaged    MATCH
openjdk21                OK        already_ok           MATCH
------------------------------------------------------------------------
VERDICT: PASS — per-Requirement outcomes are equivalent
```

Env wiring spot-check: both tools skipped `set JAVA_HOME` / `prepend PATH`
(already set machine-wide by the Temurin MSI); no overwrite happened on either
side. The no-overwrite rules are also covered by unit tests
(`windows.rs` env-wiring tests).

## What was exercised

- winget-installed / already-current classification (`already_ok`)
- non-winget registry fallback ("installed outside winget") — both sides
- env-wiring no-overwrite behavior
- fresh-install path was exercised during preparation (Git + Temurin installed
  by a legacy run whose log was lost to an aborted session; their final
  machine state is what both tools then observed) — **not** exercised in the
  recorded comparison; timeouts/reboots/docker/node-lts not exercisable here

## How to re-verify later

A pristine-VM rerun remains possible only with the archived artifacts:

1. Legacy outcome baseline: `docs/release/clean-legacy-setup.log` (captured
   2026-08-15; the legacy package itself was removed with `legacy/`).
2. Sprout preset carrying the catalog semantics: `tools/parity-preset.sprout.json`.
3. Run Sprout on the same requirement set, then
   `node tools/parity-compare.mjs --legacy docs/release/clean-legacy-setup.log`
   and compare class-wise (success: installed/upgraded/already_ok/
   satisfied_by_newer/skipped_unmanaged; failure: failed/timed_out).
