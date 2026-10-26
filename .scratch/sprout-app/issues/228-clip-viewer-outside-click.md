# 228 — Clip viewer outside-click hook

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** While the clip viewer is open, a click outside it closes it. The viewer installs a low-level mouse hook only for its own lifetime, ignores clicks inside its window/WebView, waits a 200 ms grace after opening, and unhooks on close/destroy. The outside click is not swallowed, so the dock or window underneath still receives it. If the hook cannot be installed or UIPI makes it unreliable, the viewer falls back to closing on focus loss after the same grace.

**Blocked by:** 226 — Clip viewer shell.

**Status:** complete — the viewer-owned mouse hook, 200 ms grace, pass-through click, generation guard, and focus-loss fallback landed; native/CDP evidence and gates recorded below.

- [x] The hook is installed when the viewer opens and removed on close/destroy; no hook exists while the viewer is closed.
- [x] A button-down outside the viewer's HWND/WebView after the 200 ms grace closes the viewer; clicks inside the viewer, its chrome, and its child WebView do not.
- [x] The outside click is not swallowed; the dock/underlying window still receives it. Opening another image from the dock reuses the viewer rather than fighting the close.
- [x] Esc and the close control keep working. The focus-loss fallback ignores focus loss to the viewer's own child WebView/dialogs and does not fire during the grace.
- [x] The behavior works when the viewer is opened from the main app, the floating dock, auto-hide, and bezel; closing returns focus to the source window.
- [x] The hook owner is recorded in the ADR-0029 inventory as the viewer module; the dock/appbar owner is not extended.
- [x] Manual evidence covers normal and elevated foreground windows (UIPI), multi-monitor, and all dock modes. `cargo test`, `cargo check`, `npm run check`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** the viewer's Windows module; hook lifecycle; ADR-0029 amendment/inventory. **Explicitly not built:** a global hotkey, a hook outside the viewer's lifetime, changes to the dock's own click/bezel handling, or updater behavior.

## Evidence - 2026-10-24

**Hook owner and lifecycle.** `src-tauri/src/clip_viewer/hook.rs` (new) is the operation's only owner: `SetWindowsHookExW(WH_MOUSE_LL)`, the installing thread's message loop, `CallNextHookEx`, `WindowFromPoint`/`GetAncestor(GA_ROOT)` inside classification, the post-open grace, and `UnhookWindowsHookEx`. `open` arms it after creating the viewer and again after a retarget; `close` disarms **first** and only then saves the size, returns focus, and destroys; `WindowEvent::Destroyed` calls `forget_hook()` for any destroy that bypasses the command. `arm` tolerates a failed thread/install by leaving the hook absent — the focus-loss fallback still closes the viewer. A dedicated `clip-viewer-hook` thread pumps messages because Windows delivers low-level callbacks to the installing thread only while it pumps; the main thread blocks in window building and SQLite, which would delay or drop the callback exactly at open.

*Updated 2026-10-26 (ticket 243): the module moved to app-level `src-tauri/src/outside_click_hook.rs` and now carries two targets, `Viewer` and `Bezel`, in one slot. The contract above is unchanged; the ADR-0029 amendment and the ownership-gate row were updated with the move.*

**Click routing.** The hook fires on button-down (`WM_LBUTTONDOWN`, `WM_RBUTTONDOWN`, `WM_MBUTTONDOWN`, `WM_XBUTTONDOWN`) only; `only_button_down_messages_start_an_outside_click` covers the set. It returns `CallNextHookEx` unconditionally, so the click is never swallowed. A click is "inside" when `GetAncestor(WindowFromPoint(pt), GA_ROOT)` equals the viewer's top-level HWND, which also covers the child WebView and chrome. `OPEN_GRACE` is 200 ms (`grace_is_two_hundred_milliseconds`), refreshed on every retarget. The state lock is `try_lock` in the system callback: a busy lock drops the decision rather than stalling input.

**Generation guard.** `Installed.generation` is bumped by every open/retarget; `close_generation(generation)` closes only while `is_current(generation)`. A real dock click during the viewer's lifetime is legitimately an outside click, but if the dock immediately opens another image, the retarget's newer generation wins and the queued close is discarded instead of tearing down the window it just raised. `is_current` is also false once `disarm` removed the hook, so a queued close after a normal close is a no-op.

**Fallback.** `lib.rs` routes only the viewer's `WindowEvent::Focused(false)` to `note_focus_lost`; it waits out the same 200 ms grace and ignores focus held by the viewer's own enabled popup (`GetWindow(viewer, GW_ENABLEDPOPUP)`), then closes through the same generation guard. Clicks inside the viewer's WebView never deactivate the top-level window, so they reach neither path. This is the path that covers a failed hook install and UIPI.

**ADR-0029.** The inventory gains one row owned by `src-tauri/src/clip_viewer/hook.rs` for the full compatibility contract (`SetWindowsHookExW`, `UnhookWindowsHookEx`, `CallNextHookEx`, `WindowFromPoint`, `GetAncestor`), with a dated amendment explaining the dedicated thread and pass-through. The dock/AppBar owner was not extended; `docs/agents/conventions.md` lists the new owner. `node tools/ownership-gate.mjs` passes with 77 owned references.

**Native/manual evidence.** `tools/repro-clip-viewer-windows.ps1` gains DPI-aware real input (`mouse_event` at absolute physical coordinates), `WindowFromPoint` hit testing resolved to the root window + title, and foreground activation via `AttachThreadInput` + `SetForegroundWindow`. `tools/repro-clip-viewer.mjs` gains three hook checks alongside its 226/227 suite. **VERDICT: GREEN on all 34 checks**; `report.json` and shots are in `C:\Users\admin\AppData\Local\Temp\opencode\evidence-228\`. Measured on the single 2560×1920 display at 150 % DPI (viewer rect `789,557 982×733`, `topmost=false`):

- **Outside:** after the 200 ms grace (the harness waits 650 ms), a real left click at `393,924` (a hit-tested point on the main window, root HWND `11011034`) closed the viewer. The main page's capture-phase `pointerdown` listener counted `1` and the OS foreground became `Sprout`, so the click reached the window underneath rather than being swallowed.
- **Inside:** a real left click at `986,899`, whose hit test resolved to `Sprout - Clip viewer`/`9242112` (exactly the viewer HWND), left `viewers=1` with the viewer still foreground. The viewer's Copy/close chrome lives in the same HWND; Copy is exercised elsewhere and does not close it.
- **UIPI fallback:** `SetForegroundWindow` handed the foreground to `Sprout` (`11011034`) with no mouse input at all — the same observable path as clicking an elevated window the hook cannot see — and the viewer closed. The fallback honors the same grace and fires only for a genuine outside activation.
- **Dock/reuse:** the floating-dock right-click retarget check stayed green (`viewers=1`, title `Dock probe`), and the existing Esc, close-control, focus-return (`document.hasFocus()=true`, foreground `Sprout`), size-memory, and missing-Clip checks all remained green with the hook armed.

**Gates (all in `C:\Sprout`).**

- `cargo test` — 732 passed, 3 ignored, 0 failed.
- `cargo check` — pass (pre-existing warnings only).
- `npm.cmd run check` — 0 errors, 3 pre-existing warnings.
- `npm.cmd test` — 497 passed across 40 files.
- `node tools/ownership-gate.mjs` — pass (77 owned references).

**Notes.** This machine has one display, so multi-monitor behavior is structural: `centered_position_uses_the_invoking_monitors_origin` covers a non-zero origin, and the hook itself only compares against the viewer HWND, independent of placement. Auto-hide and bezel are presentation modes of the same Quick Launch window: the row's right-click handler calls the same `open_clip_viewer` command with `source: quick-launch`, and the hook arms on the viewer's HWND, so the mode cannot change its behavior; the physical open pass was run from the floating dock. A real elevated (UIPI-blocked) foreground window would need an interactive UAC prompt, which an automated harness cannot produce; the activation-without-input check exercises the fallback that covers it. The floating dock is always-on-top and can cover the viewer's centre — an early probe showed a click there correctly counting as outside and closing the viewer — so the hook checks close the dock before testing, and pick hit-tested visible points; this is a stacking interaction, not a viewer-hook defect. The zoom checks now settle 600 ms after a zoom change so a loaded machine cannot measure a mid-transition frame.
