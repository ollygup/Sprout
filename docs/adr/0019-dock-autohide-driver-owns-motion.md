# The dock driver owns auto-hide motion; the OS only coordinates

There is no OS mechanism that moves or hides an appbar — motion is always the app's own (research `0003-appbar-autohide-os-contract`). So Sprout registers the dock with the shell for coordination only (exclusivity, notifications, z-order courtesy) and never lets the OS drive visibility: a poll driver (16 ms cadence, ~180 ms eased slide) owns the geometry, because the WebView2 child HWND swallows the mouse messages that message-driven hover would need. Hidden means fully off-screen with no handle — the 2 px sliver survives only as trigger-band math, not a visible strip. Reveal requires three things in order: cursor inside the wall band, accumulated toward-edge travel past the sensitivity threshold (12 px default, per-sample capped, along-edge motion ignored), then an uninterrupted dwell (200 ms default); any exit cancels instantly. `fixed` reserves workspace via the AppBar; `auto-hide` never reserves — other windows keep their full size whether the strip is hidden or slid out over them. Mode-setting is a pure state flip; the driver is the single writer of geometry (one placement composition feeds dock, reposition, and mode changes), so state and rect can never disagree for long.

## Consequences

- Auto-hide is independent of the taskbar's own auto-hide setting ("not tied to each other, never").
- A refused auto-hide (shell says the edge is busy) keeps the dock and records a transient blocked banner instead of unwinding the registration.
- Dwell and sensitivity are Settings (tunable, validated ranges) defaulting to the shipped gate constants — the single size source stays in `constants/window.rs`.
