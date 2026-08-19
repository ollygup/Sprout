# 26 — Tab-navigation freeze (effect self-loop → wedged router)

**What to build:** Fix the reported freeze where clicking a nav tab sometimes hangs for a long time and then opens, and subsequent tab clicks stop responding entirely. Root cause was found and fixed: the mount-time `$effect(() => { load() })` on the four data pages (Presets, Settings, History, Logs) synchronously ran the loading-phrase rotator, which read **and** wrote the `usedPhrases` signal inside the effect's tracked scope — an infinite self-reschedule that Svelte 5 aborts with `effect_update_depth_exceeded` (~1000 iterations, ~4 s of main-thread work). The uncaught throw during the navigation commit leaves the SvelteKit router permanently wedged: hover still repaints, clicks never land. The effect was replaced with `onMount` (untracked by design) on the four pages; a CDP feedback loop (`tools/repro-tab-freeze.mjs`) was built as the permanent regression harness.

**Blocked by:** — (bug found in final-audit follow-up; backend not involved)

**Status:** done

- [x] Feedback loop `tools/repro-tab-freeze.mjs` (raw CDP, zero deps) drives real mouse input over the nav rail, asserts every click lands within budget, flags main-thread stalls > 1 s, captures console/exceptions; RED verdict = user's exact symptom
- [x] Reproduced RED on every data page (`/presets`, `/settings`, `/history`, `/logs`): ~3.5–4.3 s stall + `effect_update_depth_exceeded` + permanent router death; `/plan` and `/` (products) clean
- [x] Falsified alternatives: IPC round trips 8–14 ms (`--ipc-probe`); router wedge persists after 10 s wait (permanent, not transient)
- [x] Fix: `$effect(() => { load(); })` → `onMount(() => { load(); })` in `presets/+page.svelte`, `settings/+page.svelte`, `history/+page.svelte`, `logs/+page.svelte`
- [x] Regression: full 6-tab sweep ×2 green in exe mode (12 clicks, 117–256 ms, no stalls, no exceptions); full sweep green in dev mode
- [x] `npm run check` 0 errors; release exe + setup rebuilt, copied to `dist\`; working copy synced to the share (add/update robocopy, `/L` shows Copied: 0)

## Diagnosis record (ticket 26, 2026-08-16)

### Symptom (user)

In both `npm run tauri dev` and the installed exe: clicking a nav tab sometimes hangs for a long time and then opens; subsequent tab clicks stop responding entirely while hover highlight still repaints. No run/UAC involved — plain tab browsing.

### Root cause

Svelte 5 `$effect` tracks signal reads made anywhere in its synchronous execution, including nested function calls. On the four data pages:

```js
$effect(() => { load(); });          // tracked scope
async function load() {
  ...
  nextLoadingPhrase();               // reads usedPhrases, then writes a NEW array to it
  ...
}
```

`nextLoadingPhrase()` reads `usedPhrases` (tracked) then writes `usedPhrases = [...usedPhrases, phrase]` — a fresh array, always different by `Object.is`, so the write re-schedules the very effect that is currently running, within the same flush. The flush loop re-runs the effect (~4 ms per iteration: interval churn + array copies + an IPC post per pass) until Svelte's `EFFECT_UPDATE_DEPTH_EXCEEDED` guard throws after ~1000 iterations ≈ the observed 3.5–4.3 s. The throw happens mid-navigation-commit; the uncaught exception leaves SvelteKit's router permanently pending — every later `goto` no-ops, which is the "clicks dead, hover alive" state. The rotator resets `usedPhrases = []` once the pool is exhausted, so the loop never settles on its own — only the depth guard stops it.

Why only some tabs: the products page (`/`) defers `load` into `setTimeout` inside its effect (reads only `query`), and the Plan page has no phrase rotator — neither ever read-and-writes a signal inside a tracked scope.

### Evidence chain

- Loop RED 4/4 data pages; stall sampler caught contiguous main-thread blocks of 3.2–4.0 s; one `effect_update_depth_exceeded` per failing navigation; every subsequent click never landed (5 s budget)
- H3 falsified: direct IPC timings 8–14 ms (`list_products`, `list_presets`, `list_runs`, `get_settings`, `list_logs`)
- H2 falsified: after 10 s pause the router still never lands a click (permanent wedge)
- Post-fix: 12/12 clicks land in 117–256 ms, zero exceptions — exe and dev mode

### Permanent regression harness

`node tools/repro-tab-freeze.mjs` — zero-dependency Node 24 script (built-in fetch + WebSocket). Launches the app with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<free port>`, drives `Input.dispatchMouseEvent` at the real nav-rail anchors (Products → Presets → Plan → History → Logs → Settings), asserts each navigation lands within budget, samples `performance.now()` every 100 ms to flag stalls, captures `consoleAPICalled`/`exceptionThrown`/`Log.entryAdded`. Options: `--mode exe|dev`, `--reps`, `--budget`, `--stall`, `--delay`, `--target /path`, `--targets a,b`, `--ipc-probe`. Exit 0 green / 1 red / 2 infra. ~15 s/run.