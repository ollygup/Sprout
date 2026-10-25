# One module owns every reusable window dimension

> Status: amended 2026-10-24 — original decision text preserved; see the executable-source audit amendment for current behavior, the 156-round amendment for the per-mode width caps, and the 2026-10-24 amendment for the dock's content breakpoint.

`constants/window.rs` is the single size source: every reusable window dimension lives there and nowhere else — the floating palette (`340×460`, width never draggable), the dock floor (equal to the floating width, so undock never shrinks the window), the dock width as a monitor-percentage (10–30%, default 18 ≈ 346 px on a 1920 reference, floored at the palette width and capped at 30% of the monitor so ultrawides keep a strip a strip), the auto-hide driver constants (poll, slide, sliver band, reveal gate), and the main window's default and minimum sizes. `tauri.conf.json` declares no windows; the programmatic build sizes from these constants. The frontend mirrors the numeric values it needs for sliders and clamps (with the same fallbacks the backend applies to broken stored values) but never re-derives them — the backend validates and floors, so a corrupt setting can neither collapse nor explode the strip.

## Considered options

- **Per-module constants.** Rejected: the dock width, the floating size, and the floor interact on every dock→undock round trip; three copies drift into a window that grows or shrinks by itself.
- **Work-area-based dock width.** Rejected: it feeds back — a reserving dock shrinks the work area its own width is computed from. Percent-of-full-monitor is stable across its own reservation.

## Consequences

- Any UI-dimension change starts by scanning `constants/window.rs` (AGENTS.md blocking rule); adding a second source is a review failure, not a style choice.
- A widened dock narrows back to the floor on undock; the floating palette is always exactly the floor.

## Amendment — 2026-09-05 (executable-source audit)

The backend width calculation is centralized in `dock_width_px` in `src-tauri/src/constants/window.rs`, but Settings repeats width constants and the formula in `effectiveWidthPx` (`src/routes/settings/+page.svelte`). The end-to-end single size source and “never re-derives” statements are therefore not fully enforced. The requirement to obtain reusable dimensions from the backend remains in force.

The Rust calculation gives the dock floor precedence over the percentage ceiling through `cap.max(DOCK_WIDTH)`. On sufficiently narrow monitors the minimum width can exceed 30% of monitor width; the ceiling is not absolute. Floating dimensions are 340×460 logical units (`src-tauri/src/quick_window.rs`, `open` and `undock`), whereas native dock rectangles use physical pixels. Equal physical widths across DPI scales are not guaranteed by the shared numeric constant.

`src-tauri/tauri.conf.json` still declares no configured windows. This amendment corrects the implementation description without authorizing duplicated dimensions or a new sizing policy.

## Amendment — 2026-09-07 (156-round split width caps)

The single 10–30% dock-width policy becomes per-mode: fixed keeps 10–30% (the reservation math stands — a wide fixed AppBar permanently takes workspace from maximized windows), while auto-hide allows 10–60% (an overlay reserves nothing, so the ultrawide-strip objection does not apply to it). `constants/window.rs` remains the single size source; one slider whose maximum follows the current mode; caps persist per monitor; broken stored values clamp exactly as today. Floor precedence, undock-narrows-to-floor, and the no-second-source rule are unchanged. Accepted in spec 156 (ticket 164); implementation pending.

## Amendment — 2026-09-16 (ticket 215)

Bezel constants join the single size source in `constants/window.rs`: collapsed 14px, peek 24px, height 12% of the docked monitor clamped to 64–160px physical, Y default centered (ratio 0.5 clamped 0..1; persist and drag in ticket 216). Open uses the full dock width at the resolved width-% with the auto-hide-like 60% overlay cap (`DOCK_WIDTH_MAX_PCT_BEZEL`; bezel reserves nothing extra). Svelte reads every bezel dimension through `get_bezel_*` commands — no second source.

## Amendment — 2026-10-24 (dock content breakpoint; ticket 231)

The dock's grouped list now has a layout mode switch, and it is deliberately **not** a window dimension. The tab panel's content box is the container (`container-type: inline-size` on the scroll panel, so padding and scrollbar sit outside the measurement), and `@container (min-width: 420px)` decides whether child groups nest with their own sub-headers or the subtree flattens into one list. `DOCK_GROUP_NEST_WIDTH = 420` in `src/lib/dockGroupLayout.ts` is the same line as a pure function — used by tests at 419/420/421 and as the fallback where computed styles are unavailable — and the two are guarded as a pair. This adds no window size, no width formula, and no reservation change: the dock's actual geometry still comes from `constants/window.rs`, and the mode switch only chooses a presentation for the width the window already has. Because the nested strip only exists at ≥ 420 px, a child takes a real 12 px indent (`--space-3`) so its beam clears the parent's, and each child sub-header is independently collapsible; the parent chevron still collapses the whole subtree. In flat mode the single beam keeps the parent's colour while each card keeps its own group's hue (ADR-0016, 2026-10-24 amendment).
