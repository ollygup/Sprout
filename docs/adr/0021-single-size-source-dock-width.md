# One module owns every reusable window dimension

`constants/window.rs` is the single size source: every reusable window dimension lives there and nowhere else — the floating palette (`340×460`, width never draggable), the dock floor (equal to the floating width, so undock never shrinks the window), the dock width as a monitor-percentage (10–30%, default 18 ≈ 346 px on a 1920 reference, floored at the palette width and capped at 30% of the monitor so ultrawides keep a strip a strip), the auto-hide driver constants (poll, slide, sliver band, reveal gate), and the main window's default and minimum sizes. `tauri.conf.json` declares no windows; the programmatic build sizes from these constants. The frontend mirrors the numeric values it needs for sliders and clamps (with the same fallbacks the backend applies to broken stored values) but never re-derives them — the backend validates and floors, so a corrupt setting can neither collapse nor explode the strip.

## Considered options

- **Per-module constants.** Rejected: the dock width, the floating size, and the floor interact on every dock→undock round trip; three copies drift into a window that grows or shrinks by itself.
- **Work-area-based dock width.** Rejected: it feeds back — a reserving dock shrinks the work area its own width is computed from. Percent-of-full-monitor is stable across its own reservation.

## Consequences

- Any UI-dimension change starts by scanning `constants/window.rs` (AGENTS.md blocking rule); adding a second source is a review failure, not a style choice.
- A widened dock narrows back to the floor on undock; the floating palette is always exactly the floor.
