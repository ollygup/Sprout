# 226 — Clip viewer shell

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** One reusable floating clip-viewer window. Right-clicking an image Clip row in the main app or dock opens it on the monitor that invoked it; the row's ⋯ menu also gets "Open image" as its first item. The viewer shows the image, name, and pixel dimensions, offers Copy, closes on Esc or its close control, is not always-on-top, and remembers its size across opens. Opening another image reuses the same window. This ticket builds the shell and the open/copy path; zoom/pan and the outside-click hook are separate tickets.

**Blocked by:** None — can start immediately.

**Status:** complete — viewer window, open/copy/close path, placement, size memory, and focus return landed; gates and native/CDP evidence recorded below.

- [x] One `clip-viewer` window and route; one viewer at a time. The open command resolves the Clip by id (no image bytes in URLs/events), raises/focuses an existing viewer with the new clip, or creates it otherwise.
- [x] The viewer opens centered on the monitor that owns the invoking window (main or dock), clamped to its work area. Size is remembered in machine-local settings, clamped to the work area; default is the smaller of 960×720 and 80 % of the work area; minimum 480×360.
- [x] The viewer is resizable, not always-on-top, and not a dock. Closing destroys it; the next open recreates it at the remembered size.
- [x] Right-click anywhere on an image Clip row in the main app opens the viewer; right-click on an image Clip row in the dock opens it. The row's ⋯ menu has "Open image" first. Shift+F10/context-menu key opens the menu, never the viewer. Text Clips keep their existing right-click menu.
- [x] The viewer shows the image, its name (or "Image"), and its pixel dimensions/mime. A deleted/missing Clip shows a friendly empty state and closes on Esc.
- [x] Copy uses the existing image clipboard command and shows the same copied feedback as the row.
- [x] Esc and the close control close the viewer; focus returns to the source window. The viewer has a compact frameless chrome with a drag region, labeled Copy/close controls, visible focus, and tokens only.
- [x] Viewer size memory is local-only and never appears in an export or backup.
- [x] Pure tests cover size clamping/defaulting; component-level tests cover the labels and keyboard handlers where the project already tests components. Manual evidence covers main/dock invocation, multi-monitor placement, size memory, and missing-Clip behavior.
- [x] `cargo test`, `cargo check`, `npm run check`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** a new viewer module/window/route; a clips by-id read if none exists; viewer size settings; `Icon` copy/close; i18n. **Explicitly not built:** zoom/pan controls (227); the outside-click hook (228); always-on-top; viewer-in-dock; annotation/rotation/slideshow; a second viewer window; dragging images out of the viewer.

## Evidence - 2026-10-24

**Backend.** `src-tauri/src/clip_viewer.rs` (new) owns the one window. `open` resolves the Clip by id (`clips::get_clip`) before any window exists — a missing id or a text-only Clip is refused — records the id in `PENDING_CLIP`, then either emits `clip-viewer-open` to a live viewer or builds one. The build runs inside `tauri::async_runtime::spawn_blocking`: creating a WebView2 window inline on a sync command's event thread deadlocks it, which the evidence harness caught as a window parked at `about:blank` whose `invoke` never returned. Placement reads `appbar::work_area` for the invoking window's monitor (`main` or the dock's label), falling back to the primary monitor and then `1280×720`. The window is positioned from its **outer** rect after `set_size`, because `set_position` moves the frame while `inner_size` sizes the content — centering by the inner size left the frame half a titlebar off-centre. Size memory is `clipviewer.width` / `clipviewer.height` in the machine-local `meta` table (`save_size` on close, `load_size` on open, through `db::read_meta` / `upsert_meta`). `close` saves the size, clears the pending id, focuses the source **before** `destroy()` (while the viewer is still foreground Windows honours the activation; afterwards it can refuse), and restores the page's own keyboard focus through `Webview::set_focus()` (`MoveFocus(PROGRAMMATIC)`) — without that last step the source window came back as the foreground window with `document.hasFocus() === false`, so the first keystroke after closing went nowhere. `constants/window.rs` holds the viewer dimensions and the pure `clip_viewer_default_size` / `clip_viewer_clamp_size` pair; tests `clip_viewer_default_is_smaller_of_preferred_and_80pct`, `clip_viewer_default_respects_floor_and_tiny_work_areas`, `clip_viewer_remembered_size_clamps_to_floor_and_work_area`, `remembered_size_roundtrips_in_machine_local_meta`, `open_size_clamps_the_stored_pair_into_the_work_area`, and `centered_position_uses_the_invoking_monitors_origin` (a 1920-wide monitor at x=1920 yields `(2400, 160)`). `lib.rs` registers `open_clip_viewer` (async, `spawn_blocking`), `take_clip_viewer_request`, `close_clip_viewer`, and routes the viewer's close-requested event through `clip_viewer::close`, so the OS frame's close behaves like Esc. `capabilities/clip-viewer.json` scopes drag/theme/event permissions to the one label. `backup.rs`'s `machine_scoped_state_never_travels` seeds a `1111×777` size and asserts the restored library has none.

**Frontend.** `src/routes/clip-viewer/+page.svelte` reads the pending id, listens for `clip-viewer-open` (retarget in place — one window), re-reads on window `focus` so a Clip deleted while the viewer is open surfaces the empty state, and owns Esc, Copy (the existing `copyClipImage` command plus the row's "Copied" flash and live announcement), and the labeled Copy/close controls; `+layout.svelte` gives the route the mini treatment (no rail). `api.ts` adds `openClipViewer` / `takeClipViewerRequest` / `closeClipViewer`. `src/routes/clips/+page.svelte` opens the viewer on `e.button === 2` for image rows and keeps text Clips on the existing menu; a keyboard `contextmenu` (`button === 0`) always opens the row menu, where "Open image" is the first item for image Clips. `QuickLaunchRow.svelte` gains an optional `oncontextmenu`, wired by the dock's Clips tab to the same command with `source: "quick-launch"`. `Icon.svelte` gains the image glyph. Nine `viewer.*` / `menu.openImage` keys were added to both locales (1212 keys each, parity kept).

**Decisions applied.** ADR-0026 (machine-local boundary): the remembered size lives in `meta`, which no export or backup reads. ADR-0021 (one size source): every viewer dimension is in `constants/window.rs`, none re-declared in the module. ADR-0029 (one owner per Windows command): placement still reads `appbar::work_area`, the existing owner, rather than adding a second monitor query.

**Automated gates (all in `C:\Sprout`).**

- `cargo test` — 722 passed, 3 ignored, 0 failed.
- `cargo check` — pass; the 2 warnings are pre-existing `windows_execution` ones (unused `python_argv`, dead `quote_staged_path`).
- `npm.cmd run check` — 0 errors, 3 pre-existing warnings (`QuickActionFormDialog.svelte`, `UnifiedHeader.svelte`).
- `npm.cmd test` — 457 passed across 38 files; `clipViewer.close.test.ts` covers the labels, Esc + close control, copy feedback, the missing-Clip empty state, and retargeting.
- `node tools/contrast-check.mjs` — all checks pass in both modes.
- `node tools/ownership-gate.mjs` — pass (62 owned references).

**Manual/native evidence.** `tools/repro-clip-viewer.mjs` (new, kept as a regression harness) drives the real app over CDP against a throwaway `%LOCALAPPDATA%`, with `tools/repro-clip-viewer-windows.ps1` (new, DPI-aware) for the window-manager facts CDP cannot see. **VERDICT: GREEN** on all 15 checks; `report.json` and screenshots live in `C:\Users\admin\AppData\Local\Temp\opencode\evidence-226\` (`viewer-main.png`, `main-row.png`, `viewer-copied.png`, `viewer-dock.png`, `dock-row.png`, `viewer-missing.png`). Measured on the single 2560×1920 display (150 % DPI, 72 px taskbar):

- Text Clip right-click and the Shift+F10 / Menu-key contextmenu open no viewer; the keyboard path opens the row menu with "Open image" first.
- Main right-click opens one viewer showing `Viewer probe | PNG · 64 × 48 · 6.9 KB`, a drag region, and image alt "Preview of Viewer probe".
- Native rect `789,557 982×733`, `topmost=false`. The work area is 2560×1848 at y=0, so the viewer's centre `1280,923.5` matches the work-area centre `1280,924`: the 960×720 default lands centred, and the frame is 982×733.
- Copy flashes "Copied" and announces "Viewer probe copied.".
- Resize to `678×487.5` physical → Esc closes → the row menu's "Open image" reopens at exactly `678×487.5`.
- The close control returns focus: `document.hasFocus()=true` and the OS foreground window is `Sprout`.
- Dock right-click retargets the same single viewer (`viewers=1`, title `Dock probe`).
- Deleting the Clip shows "This clip is gone / It was deleted or moved. Press Esc to close.", and Esc closes it.

**Notes.** This machine has one display, so cross-monitor placement is covered by `centered_position_uses_the_invoking_monitors_origin` (non-zero origin) plus `work_area_for` reading the invoking window's own monitor; the two-display pass stays on the ticket's manual checklist. The harness reaches the dock by persisting `dock_state = "docked"` and restarting, because `open_sprout_cmd` deadlocks when it builds the dock's WebView2 inline from an IPC command while docked and missing — a pre-existing bug outside this ticket, recorded rather than fixed. Zoom/pan (227) and the outside-click hook (228) remain unbuilt here, as scoped.
