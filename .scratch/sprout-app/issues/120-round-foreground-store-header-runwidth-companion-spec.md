# 120 — Round: foreground-on-tap, Store scan, dock header minimal, Run/Stop width, Companion single-site browser (spec)

**What to build:** One round fixing five surfaces as a coherent dock + quick-access move: (1) single-tap on a Launch entry foregrounds its existing window when already running (batch `Start all` keeps skip), at the same Z as normal windows so it appears above a Fixed dock; (2) the app picker surfaces Microsoft Store/MSIX apps via `PackageManager`→`AUMID` alongside Win32 `.lnk`/`.exe` without touching existing validation; (3) the Quick Launch window/dock header drops the `Quick Launch` wordmark and keeps only the `SproutMark` as drag affordance; (4) Run/Stop controls share one `min-width` so the button never jumps on state change (color is the signal); (5) a dock-only, content-gated, single-site Companion WebView directly navigates to a chosen HTTPS site in the lower portion of the dock, with a draggable splitter per monitor and an external-browser escape for unsupported sites. Implemented via tickets 121–125, audited by closing pass. Research `docs/research/0012-companion-webview-feasibility.md` and glossary `docs/CONTEXT.md` Companion entry land with this round.

**Blocked by:** none (round spec; implemented via tickets 121–125)

**Status:** implemented via tickets 121–125; Companion narrow-viewport visual follow-up remains

## Original Problem Statement

Before tickets 121–125, Quick Launch always spawned even when the app was already visible, the picker knew only Win32 `.lnk`/`.exe` apps, the dock header spent scarce width on its wordmark, Run/Stop controls changed width, and there was no Companion surface for a music/video site. This section records the pre-round problem; the implementation status and corrected Companion limits above describe the current product.

## Solution

**Foreground-on-single-tap (a).** Only a single-entry tap foregrounds when already running; `Start all` keeps the ticket 99 skip (`unassigned_app_entries_free_their_slot_at_spawn`, ticket 48 per-desktop idempotency). App entries only — commands always spawn. Match by `EnumWindows`→`GetWindowThreadProcessId`→`QueryFullProcessImageName` basename trimmed, case-insensitive, on the entry's target desktop (`desktop_id` or current per `docs/CONTEXT.md:66` Launch entry, reusing the snapshot seam in `launch.rs:488`). Single hit → `ShowWindow(SW_RESTORE)` + `SetForegroundWindow` at normal Z (covers Fixed dock when overlapping, same as taskbar — Q10), no `HWND_TOPMOST`, no second `ShellExecute`.

**Store/MSIX scan.** Add `Windows.Management.Deployment.PackageManager.FindPackages()` → manifest `Applications/Application@AppUserModelId` enumeration, merge/dedup with existing Win32 candidates, filter framework packages. Stored as `target=shell:AppsFolder\<AUMID>` `kind=app` subtype `uwp`, launched via `IApplicationActivationManager::ActivateApplication`. Additive only — `validate_launch_entry` (`launch.rs:120`) and `quick_actions` unchanged.

**Header minimal (Q6).** Remove `<h1 class="qlw__title">Quick Launch</h1>` when docked, keep `SproutMark size=16` (`+page.svelte:452`) as `data-tauri-drag-region` handle (`src/lib/quickLaunchTitleBar.ts`). Edge arrows + `undock`/`x` stay. Cite deviation from `0005-page-chrome-consistency.md:1` one-header-implementation; `0006` pattern 5 (relocating costs) — mark keeps scent.

**Run/Stop width (Q8).** Measure Run's rendered width, apply as `min-width` token to Stop (`Button.svelte:36` `btn` `gap`+`padding:8px 16px`, variants `primary`/`danger` `Button.svelte:59/79`) so Run→Stop→Stopping (`0004` feedback) never reflows at `340px`; color `accent`→`danger` is the only signal (`0006:6` one reserved accent, `0005:2` one primary).

**Companion single-site browser (b, Q11/12 + your `any web app, just 1, no multi-tab`).** One child WebView2 navigates directly to the chosen HTTPS URL at bounds measured from the dock's content frame. It uses an Android Chrome identity that matches WebView2's Chromium engine and a persistent isolated `companion` data directory. Direct navigation avoids iframe-only `X-Frame-Options` failures but cannot guarantee that every site or authentication flow permits embedded browsers, so a stable failure state offers Retry and Open externally. The splitter defaults to 40%, clamps to 25–60%, persists per monitor, and is absent while floating or when no site is active. Active site and height live in Settings; the separate manager owns saved-site CRUD only. Native history and `_blank` interception from the proposal are not claimed by the shipped Tauri child-WebView surface.

## User Stories

**Foreground**

1. As a user tapping one Launch entry whose app is already open on the target desktop, I want its window foregrounded (restored if minimized) with no second process, so tapping feels like the taskbar.
2. As a user clicking `Start all`, I want already-running entries reported as skipped on their desktop (not foregrounded or re-launched), so a batch doesn't steal focus.

**Store**

3. As a user adding a Launch entry, I want Store/MSIX apps to appear in the picker alongside `.lnk`/`.exe`, so Calculator or Store Spotify needs no custom command.
4. As a user launching a Store app, I want it activated via its AUMID, so it opens exactly as from Start.
5. As a user, I want framework/extension packages never listed, so the picker stays app-only.

**Header / tabs / controls**

6. As a dock user, I want the header to show only the Sprout mark plus window controls, so tabs have room at 340px without degrading early.
7. As a user toggling a Quick Action between Run/Stop, I want the button to keep its width and only change color to danger for Stop, so the control doesn't jitter.

**Companion**

8. As a dock user, I want to pick one saved HTTPS site in main-app Settings and see its narrow responsive layout in the lower portion of the dock, with a clear browser escape when the site refuses embedding.
9. As a user, I want the companion pane and its splitter hidden when no URL is set and when the window is floating, so empty chrome never occupies space.
10. As a user, I want the splitter draggable (default 40% clamped 25–60%) remembered per-monitor, so 1080p and 4K both feel right.
11. As a user with no need for a browser, I want the app unchanged — no pane, no extra buttons — until I set a URL.

## Implementation Decisions

- **Foreground seam:** branch `isSingle` through `run_launch_queue_until` (`launch.rs:468`); `Some(single)` snapshots pre-launch windows and foregrounds on hit, `Start all` path keeps `ticket 99` frees-at-spawn for unassigned + dead-desktop frees-at-spawn with note. No new column; `LaunchEntry` unchanged, desktops resolved against engine's live desktop list.
- **Store seam:** new `store::enumerate_uwp()` behind `launch::list_candidates()`, merged before sort; `shell:AppsFolder` target shape already handled by existing icon extraction (`icons.rs`) for Win32 — UWP path adds `ActivateApplication` launcher branch.
- **Header:** conditional `{#if dock.docked}` wordmark removal only; `titleBarDragRegion(dock.docked)` (`+page.svelte:450`) stays on header+mark; floating window keeps wordmark for discoverability (one-header caveat documented).
- **Run width:** measure once in `QuickActionRunControl.svelte` (`run` label `Run`/`Starting…` longest), set `--run-w` token, `Stop`/`Stopping` use `min-width:var(--run-w)`; `Button` geometry untouched.
- **Companion:** single child `Webview` label `companion`, bounds derived from the measured content frame, Android Chrome identity, persistent isolated data directory, and explicit OS-default-browser escape. Settings own `companionUrl` + `companionHeightRatio`; the manager owns the ordered saved URL list.

## Testing Decisions

- Foreground: `FakeLauncher` (`launch.rs:1103`) windows-per-desktop + `handed_off` — new `foregounds_single_on_same_desktop`, `batch_skips_without_foreground`, `unassigned_foregounds_on_current_desktop`, `dead_desktop_frees_without_foreground`.
- Store: enumeration filtered count test, picker merge dedup test, activation branch mocked via `Launcher::activate_uwp` on `FakeLauncher`.
- Header/width: `svelte-check` 0/0 + manual 340px DPI check (`0004` physical-px); no cargo path.
- Companion: backend settings round-trip coverage plus frontend source/geometry regression tests; manual pass covers no URL→no pane, URL→native pane, stable failure actions, splitter persistence, floating→hidden, and fixed-dock restore. Native creation remains an OS-level smoke test.

## Out of Scope

- Multi-tab companion or omnibox in the pane (single-tab only; new spec if requested).
- Desktop HWND `SetParent` embedding of arbitrary `exe` (rejected for CEF/Chromium hosts; see 0012).
- Command blob spillover (`quick_actions.command TEXT` `db.rs:121`) — Q13 left as-is.
- Justified equal-width tabs (hug-left retained `Tabs.svelte:178`).
- Storing companion URLs in Preset exports/backups beyond machine-local `settings` — companion is machine-local like `install_dir` (`ADR-0009` spirit).

## Further Notes

- Evidence base: `0004-progressive-disclosure` rules 2–4, `0005-page-chrome-consistency` rules 1–2, `0006-notion-patterns` 1/5/8/11–12, `0003-appbar` one-auto-hide-per-edge, `0011-natural-edge-reveal` dwell/seam, plus 0012 companion WebView2 vs `SetParent` sources (`v2.tauri.app`, `WebView2 docs`, `electron#10547`/`#26729`, `sweetwisdom`).
- The glossary now defines **Companion** alongside `Quick Launch window` and `Quick Launch dock` without leaking implementation details into domain language.
