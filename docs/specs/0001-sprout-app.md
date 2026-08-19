---
title: Sprout — Preset-Driven Windows Installer Desktop App
status: ready
labels: [ready-for-agent]
---

# Sprout — Preset-Driven Windows Installer Desktop App

> Spec synthesized from the grilling session (Rust + Tauri, domain-modeling glossary in `docs/CONTEXT.md`, decisions in `docs/adr/`).
> Local copy awaiting publish to the issue tracker — no tracker configured in this environment at spec time.

## Problem Statement

Configuring a new machine today means the PowerShell package (`Setup.bat`, `runner.ps1`, `select-apps.ps1`, `catalog.yaml`): a console-menu picker, manual YAML editing to change version behavior, and no way to share a known-good setup — "VSCode must be newest, OpenJDK must be v21, DBeaver newest" — with another machine except copying scripts around. There is no notion of version pinning beyond `latest`/`present-only`, no verification that the install actually works, and the UX is a terminal window. (The package was kept in `legacy/` as the parity reference and removed after the v1 parity gate passed.)

## Solution

A Windows desktop application, **Sprout** (Rust + Tauri 2 + Svelte 5, single exe, target <10 MB), that replaces the scripts. The user composes a **Preset** from a blank-canvas **Product** library (empty on first run — ADR-0008; entries appear only after the user adds them from the live winget registry search), declares each **Requirement** with a **VersionPolicy** (`latest` | `pinned` | `present`), optional **Env wiring** and optional **verify** commands, then runs it against the machine: a read-only **Plan** preview (install / upgrade / skip / conflict), a single UAC prompt, and an elevated worker installs on the user's behalf with live progress. Presets export to a single self-contained `.sprout.json` that any other Sprout user imports (including by double-click) and applies identically. The engine is a trait-based strategy (`PlatformEngine`), so a future platform (macOS) is a new implementation, not a fork. Runs and logs are stored locally (`%LOCALAPPDATA%\Sprout`, created lazily) and browsable in-app.

## User Stories

**Product library**
1. As a user, I want the library to start empty and to add products by searching the live winget registry, so that I can compose a preset without typing winget IDs from memory.
2. As a user, I want to add a custom product (name + winget ID + install-location hint + default env suggestions), so that I can include software outside the registry search.
3. As a user, I want to edit any product's metadata, so that I can correct names or hints without deleting and recreating.
4. As a user, I want to delete products I never use, so that the library stays mine.
5. As a user, I want the product list to be searchable, so that composing a large preset stays fast.

**Preset authoring**
6. As a user, I want to create a named preset with a description, so that I can recognize it later and tell others what it contains.
7. As a user, I want to add a Requirement to a preset by choosing a product, so that the preset declares what the machine must have.
8. As a user, I want to set a Requirement's VersionPolicy to `latest`, so that the app installs/upgrades to the newest available version.
9. As a user, I want to set a Requirement's VersionPolicy to a pinned version, so that e.g. OpenJDK is exactly v21.
10. As a user, I want to set a Requirement's VersionPolicy to `present`, so that an already-installed app is left alone (never upgraded).
11. As a user, I want to declare dependencies between Requirements, so that installs happen in dependency order.
12. As a user, I want to set a per-Requirement timeout, so that a hung installer is killed rather than blocking forever.
13. As a user, I want to attach Env wiring (`set` / `prepend`, with the `<InstallLocation>` placeholder) to a Requirement, so that JAVA_HOME and PATH are configured as part of the install.
14. As a user, I want to attach verify commands to a Requirement (e.g. `java -version` must report 21), so that success means "it actually works", not just "winget exited 0".
15. As a user, I want preset validation on save (duplicate product, unknown dependency, malformed policy or env entry) with clear messages, so that broken presets never reach another machine.
16. As a user, I want to duplicate (fork) a preset, so that I can edit an imported preset without touching the original.

**Preset export / import**
17. As a user, I want to export a preset to a single `.sprout.json` file that embeds its product definitions, so that the file is self-contained and versioned.
18. As a user, I want to import a `.sprout.json` by double-clicking it, so that applying a teammate's setup starts with one gesture.
19. As a user, I want to import a `.sprout.json` from inside the app, so that I don't depend on file association.
20. As a user, I want imported presets stored in my local Library exactly as authored (immutable), so that I can always see the original.
21. As a user, I want a clear rejection message when a preset's schema version is too new or too old, so that nothing silently misinterprets my config.
22. As a user, I want a warning when a preset targets a platform other than mine, so that I don't try to apply a macOS preset on Windows.
23. As a user, I want an error when a preset contains the same product twice, so that policy ambiguity is caught at import, not at run time.

**Running**
24. As a user, I want a Plan preview before anything is installed — per Requirement: will install / will upgrade / already OK / satisfies-by-newer / conflict — so that I can verify the outcome before it happens.
25. As a user, I want to toggle any Requirement in or out for a run, so that I can skip e.g. Docker this time without editing the preset.
26. As a user, I want to select multiple presets for a single run (union of Requirements), so that I can combine A+B from one preset with C+D from another.
27. As a user, I want overlapping products from different selected presets surfaced as explicit conflicts to resolve (pick a policy or opt out), so that composition is never silent about ambiguity.
28. As a user, I want to save a composed multi-preset run as a new preset, so that a combined config becomes shareable.
29. As a user, I want one UAC prompt at run start (via the app relaunching itself elevated), so that I don't answer a prompt per app.
30. As a user, I want live per-Requirement progress during the run, so that I can see what is happening and what's next.
31. As a user, I want a run summary: per-Requirement outcomes (installed / upgraded / already OK / satisfied-by-newer / failed / reboot required), so that I know exactly what happened.
32. As a user, I want the app to never downgrade an installed product — pinned-but-newer present is reported as "satisfied by newer", so that a preset never destroys a working install.
33. As a user, I want timeboxed steps that are killed and recorded as timed-out, with re-running skipping already-completed Requirements, so that a hung installer never wedges the machine.
34. As a user, I want winget bootstrapped automatically if it is missing, so that the app works on freshly imaged machines.
35. As a user, I want Env wiring applied only after a successful install, only to User scope, and never overwriting values I already set, so that the machine is never degraded.
36. As a user, I want verify commands executed after install and a failed verification to fail that Requirement loudly, so that "replicate without fail" is actually checked.
37. As a user, I want to know when a run ends with "reboot required", so that I can restart and re-run to finish.
38. As a user, I want to cancel a running plan, so that I stay in control of my machine.

**History and logs**
39. As a user, I want a Runs list (timestamp, preset(s), overall outcome) so that I can see what my machine received and when.
40. As a user, I want to reopen any past run's detail with its per-Requirement results, so that I can investigate a failure weeks later.
41. As a user, I want every run's full log on disk, so that support staff can read the raw output.
42. As a user, I want a Logs tab that shows where log files live, their sizes, and an open-folder action, so that I always know where to find them.

**Non-functional**
43. As a user, I want the installed app to stay under ~10 MB, so that it fits on the smallest installers and USB sticks.
44. As a user, I want to browse, compose, and preview plans without admin rights — only the run phase elevates, so that the app feels safe to open.
45. As a user, I want all app data (SQLite DB, logs) created lazily on first run under `%LOCALAPPDATA%\Sprout`, so that the installer is clean and nothing ships stale state.
46. As a maintainer, I want the install engine behind a platform strategy trait with a Windows implementation, so that macOS can be added as a new implementation without touching the app.
47. As a maintainer, I want step types dispatched through a registry, so that new step kinds (e.g. download-and-run installers) are additive, not invasive.
48. As a maintainer, I want the legacy PowerShell package preserved in `legacy/` during development, so that behavior parity is checkable until release — done: parity passed at v1 (ticket 10), package removed.

## Implementation Decisions

1. **Stack**: Rust (Tauri 2) backend + Svelte 5 / TypeScript / Vite frontend, `rusqlite` for storage, `winreg` for registry heuristics. Single `Sprout.exe`; NSIS installer; `.sprout.json` file association registered by the installer (double-clicking a preset file opens Sprout and imports it; a second instance forwards the file via the single-instance plugin). Per-user install to `%LOCALAPPDATA%\Programs\Sprout` (vendored NSIS template, see ADR-0006) — the exe never lands inside the `%LOCALAPPDATA%\Sprout` data dir. Release target ≈4.6 MB exe / ≈1.9 MB installer (well under the 10 MB gate). Signing deferred.
2. **Engine strategy (Java-interface equivalent)**: `trait PlatformEngine` defining detection, install, upgrade, verify, and env-wiring operations; `WindowsWingetEngine` is the v1 implementation; held as `Arc<dyn PlatformEngine>` in Tauri managed state so commands receive it via `State`. Future macOS = new impl swapped at startup; the rest of the code is unchanged.
3. **Step model and registry**: a Requirement's execution is a `Step`, which is data — `winget { id, version, scope, extra switches }` or `command { exe, args, success codes }`. Execution dispatches through `HashMap<StepType, Box<dyn StepExecutor>>` (the plugin-registry pattern), so the run loop never hardcodes step logic.
4. **VersionPolicy**: `latest` | `pinned { version }` | `present`. Never-downgrade rule is a core invariant: pinned vs newer-installed ⇒ reported "satisfied by newer", never downgraded.
5. **Plan computation**: read-only (winget list + uninstall-registry checks + bootstrap check for missing winget); produces per-Requirement expected actions; conflicts only arise from multi-preset composition and are resolved explicitly by the user at the preview screen.
6. **Elevation**: main process runs non-elevated; the run phase self-relaunches `Sprout.exe --worker` via UAC; the worker executes the plan, appends progress as JSON-lines to a per-run status file the UI tails, and persists results to SQLite. No cross-elevation IPC.
7. **Run semantics** (ported from the legacy runner): dependency-first ordering, per-Requirement timebox, exit-code whitelist (including 3010 / 1641 / `0x8A1500xx` reboot and already-installed/up-to-date families), registry fallback detection for non-winget installs, "present-only skips", reboot-required summary, re-run skips completed Requirements.
8. **Env wiring** (ported verbatim): User scope only; `set` never overwrites an existing User/Machine value; `prepend` only when absent from both scopes; `<InstallLocation>` (optionally `<InstallLocation:hint>`) resolved from uninstall registry keys at apply time.
9. **Verify commands**: optional per-Requirement commands run after install; non-matching output or non-zero exit fails the Requirement.
10. **Persistence**: SQLite at `%LOCALAPPDATA%\Sprout\sprout.db`, created lazily on first run (never shipped by the installer); tables for products, presets, requirements, runs, and run results. Logs as files under `%LOCALAPPDATA%\Sprout\logs\`; the Logs tab shows locations/sizes with an open-folder action (no live viewer).
11. **Preset file schema** (`schemaVersion: 1`), self-contained and serde round-trippable:

```jsonc
{
  "schemaVersion": 1,
  "platform": "windows",
  "name": "Backend dev box",
  "description": "Java 21, VSCode, DBeaver",
  "author": "User A",
  "version": "3",
  "requirements": [
    {
      "product": { "id": "openjdk21", "name": "Eclipse Temurin OpenJDK 21 (LTS)", "wingetId": "EclipseAdoptium.Temurin.21.JDK" },
      "step": { "type": "winget", "id": "EclipseAdoptium.Temurin.21.JDK", "scope": "machine" },
      "versionPolicy": { "kind": "pinned", "version": "21.0.5" },
      "dependsOn": [],
      "timeoutMinutes": 10,
      "env": [ { "action": "set", "name": "JAVA_HOME", "value": "<InstallLocation:Eclipse Temurin>" },
               { "action": "prepend", "name": "PATH", "value": "<InstallLocation:Eclipse Temurin>\\bin" } ],
      "verify": [ { "command": "java", "args": ["-version"], "match": "21" } ]
    }
  ]
}
```

12. **Library model**: presets are immutable once imported; editing always forks; multi-preset runs are the composition mechanism with explicit conflict resolution; a composed run can be saved as a new preset and exported.
13. **Empty first-run library**: nothing is seeded (ADR-0008) — fresh installs open to an empty Library, and Products exist only after the user adds them from the live winget registry search, so every entry is deliberate and current.
14. **Extensibility of steps**: winget's manifest system already covers exe/msi/msix/inno/nullsoft/wix/zip-portable installs, silent switches, and success codes, so v1 ships `winget` + `command`; a future `download-run` step type is one executor registration.

## Testing Decisions

- **One seam**: the engine boundary (`PlatformEngine` + `StepExecutor`). A good test describes external behavior: given a machine-state snapshot and a preset, assert the produced Plan; given fake executor outcomes, assert run ordering, results, and reboot flags. Tests must not assert internal dispatch details.
- **What is tested**: preset parsing/validation (schema, duplicates, dependencies, env grammar, policy matrix), plan computation (every VersionPolicy × installed-state combination, including satisfy-by-newer), exit-code classification (the whitelist table), env-wiring rules (no-overwrite, both scopes, placeholder resolution), run orchestration (ordering, timeouts, toggles, multi-preset conflicts) — all against fake detection and fake executors.
- **Prior art**: none in-repo (the legacy scripts shipped no tests); this establishes the first harness. The release parity gate passed at v1 (ticket 10) — device-based same-state comparison, recorded with caveats in `docs/release/parity-checklist.md`; compare tooling: `tools/parity-compare.mjs`.
- **UI**: manual + the accessibility checklist from the Web Interface Guidelines; no UI unit tests in v1.

## Out of Scope

- macOS/Linux support (architecture is ready; implementation is not — their preset files carry a platform tag and are rejected with a clear message on the wrong OS).
- Download-and-run installer step type (v2, additive via the registry).
- App self-update and code signing.
- Portable/USB data mode (app is installed; data is per-user).
- In-app log viewer (location tab only).
- Uninstalling products, remote/multi-machine management, preset auto-merge.

## Further Notes

- Size budget: Rust + Tauri exe ≈ 4–6 MB; frontend is hand-rolled Svelte (no heavy component library), so <10 MB is comfortable.
- Docs already in-repo: `docs/CONTEXT.md` (glossary), `docs/adr/` (eight ADRs), `docs/research/0001-step-types-and-extensibility.md` (Q13 findings), `docs/release/parity-checklist.md` (release gate procedure).
- The existing `scripts/`, `Setup.bat`, and `config/catalog.yaml` were moved to `legacy/` and kept runnable as the parity reference until the ticket-10 parity gate passed (2026-08-15), then deleted. Legacy's observable outcomes for the gate machine state are archived in `docs/release/clean-legacy-setup.log`; the catalog semantics live on as `tools/parity-preset.sprout.json`.
