# 56 — Floating window life cycle + shared UI constants

**What to build:** The Quick Launch window stops auto-hiding on blur: it stays open until the user closes it (× / Alt+F4), is freely draggable, and always opens at the fixed 340×460 centered size. All reusable window-geometry constants move into one shared file, the docked strip uses the same width as the floating window, and undocking restores the exact original size. Parent spec: 55.

**Blocked by:** 55 — Quick Launch window: dock, floating UX, live sync, and Quick Action control (spec)

**Status:** done — blur no longer destroys (Focused(false) handler removed), window_constants.rs shared by quick_window/appbar/lib, undock restores inner+min+max in logical units (the builder's `inner_size` is logical — a PhysicalSize restore shrank the window on scaled displays; fixed 2026-08-20), 252 backend tests green (incl. new dock-width test), svelte-check 0 errors, synced to the share; manual drag/round-trip pass pending a human

- [x] Blur no longer destroys the window (`lib.rs` `Focused(false)` handler for the Quick Launch window — the floating path becomes a no-op; the docked path stays a no-op); × button / Alt+F4 still destroy it (tray reopens); tray left-click still raises or opens it
- [x] Dragging by the header (existing `data-tauri-drag-region`) verified while the cursor leaves the window bounds mid-drag — the window must not vanish (manual pass pending; blur no-op removes the destroy path)
- [x] New `src-tauri/src/window_constants.rs`: `WINDOW_WIDTH` (340), `WINDOW_HEIGHT` (460), `DOCK_WIDTH = WINDOW_WIDTH`, main-window size/minimums (1200×800 / 900×620); `quick_window.rs`, `appbar.rs`, and `lib.rs` reference it; the 320 `DOCK_WIDTH` in `appbar.rs` is deleted and its unit tests updated
- [x] `undock()` restores the exact 340×460 inner size with the same size-API family the builder used (inner + min + max), then centers; dock → undock → dock → undock round trip verified — the window is never smaller, including the auto-hidden-bar path (OS-hidden-bar `window.show()` kept; round trip manual pass pending)
- [x] AGENTS.md gains a design rule: reusable UI geometry constants live in `src-tauri/src/window_constants.rs` — never re-declared in another module; scan that file first before any UI-dimension change
- [x] `cargo test` green (252 passed, 1 device-probe ignored; appbar rect tests updated, dock-width test added); `npm run check` 0 errors; synced to the share