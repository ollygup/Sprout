# One source of truth per Windows command

> Status: amended 2026-10-24 — ownership constraint; implementation candidates remain under review.

Each distinct Sprout-owned Windows command or Windows API operation must have exactly one source of truth for its invocation and compatibility knowledge. A Windows update, changed command contract, or removal must require repairing or removing that operation in one owning module, rather than finding copies across install, authoring, Quick Launch, and Quick Actions. This decision constrains the architecture; it does not claim the current code already satisfies it.

The owner keeps the executable or native entry point, arguments and flags, output interpretation, error translation, and required resource lifetime together. Callers supply domain intent and consume outcomes; they do not reconstruct the Windows call. Repeated uses of the same Windows API entry point must converge inside its owning implementation. Different uses retain explicit semantics there: for example, normal launch and elevated worker launch must not silently acquire the same privilege or process-lifetime policy. User-authored commands remain user data, not centrally enumerated Sprout commands.

## Considered options

- **Keep independent calls and share only helpers or constants.** Rejected: sharing an executable name, a flag, or a parser fragment still leaves the operation's compatibility contract distributed across callers.
- **One universal Windows dispatcher with raw arguments.** Rejected: it relocates syntax without hiding knowledge, broadens the interface, and fails the deletion test. Ownership is organized into deep modules around coherent operations, not one catch-all module.
- **One owner per operation, with small internal modules.** Accepted: centralize compatibility changes while keeping distinct domain policies explicit. Large implementations may split into private submodules with restricted visibility; the existing caller-facing interface must not grow merely to expose the split.

## Preservation and verification

- Preserve the validated install flow and the `PlatformEngine` seam (ADR-0001, ADR-0003, ADR-0004): elevation, preparation, detection, version policy, timeout/process-tree termination, exit and reboot handling, env wiring, verification, and outcome honesty remain unchanged (ADR-0009, ADR-0023, ADR-0024).
- Preserve Quick Launch (the informal "Quick Start") and its `LauncherEngine` seam: cap and queue, single-flight behavior, already-open handling, single-tap focus, desktop placement, and honest outcomes (ADR-0010, ADR-0015, ADR-0018).
- Preserve hidden, unelevated Quick Actions, run/stop tracking, logs, and watchdog behavior (ADR-0017). Keep catalog search/show authoring-only and discovery fresh (ADR-0027).
- Preserve passing tests. Before implementation, establish a baseline; after each chosen change, exercise the existing domain interfaces and the affected Windows behavior. An internal seam is justified by a production adapter and an actually used test adapter, not hypothetical future platforms.
- Maintain an operation-to-owner inventory and check call sites when implementing each candidate. Moving files alone does not satisfy ownership. Tests may describe expected Windows contracts but must not introduce alternate production invocations.

The trade-off is deliberate coupling of callers to one compatibility owner, plus the migration cost of consolidating proven behavior. The benefit is locality: one Windows change, one implementation to fix. This extends existing decisions without replacing their execution or product semantics.

## Amendment — 2026-10-24 (clip viewer outside-click hook owner)

The clip viewer needs a low-level mouse hook for its own lifetime (spec 221 §B.6). Its owner is `src-tauri/src/clip_viewer/hook.rs`, a private submodule of the viewer module. The dock/AppBar owner is **not** extended, and no other module may install a hook.

The owned operation covers the whole compatibility contract, not just the entry point: `SetWindowsHookExW(WH_MOUSE_LL)`, the installing thread's message loop, `CallNextHookEx` pass-through (the outside click is never swallowed), `WindowFromPoint`/`GetAncestor(GA_ROOT)` inside classification against the viewer's own HWND and child WebView, the 200 ms post-open grace, and `UnhookWindowsHookEx` on close or destroy. The dedicated pumping thread exists because Windows delivers low-level hook callbacks only to the installing thread while it pumps messages; the app's main thread blocks while building windows and touching SQLite, so hooking there would delay or drop callbacks exactly when the viewer opens. The thread and view are owned by this module; the viewer module's `open`/`close` call its arm/disarm interface rather than reconstructing any of it.

The focus-loss fallback is part of the same operation: a failed install or UIPI unreliability against an elevated foreground window still closes the viewer after the same grace, ignoring focus loss to the viewer's own popups. It lives in the same module so the two dismissal paths share one grace and one open generation. `tools/ownership-gate.mjs` carries the corresponding inventory row.
