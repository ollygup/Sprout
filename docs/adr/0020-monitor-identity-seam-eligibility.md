# Monitors are told apart by hardware identity; seams refuse the dock

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

`enumerate_displays` is the single source for geometry and identity: each screen's rectangle plus its EDID identity (`edid-XXXX-YYYY`). Dock memory (edge, mode, width, companion ratio) is keyed identity-first with device-name fallback, so replugging a panel elsewhere keeps its preference and pre-identity rows still resolve; an all-zero EDID keys as `None`. An edge is eligible for docking only when it is a real wall — a seam where two screens touch by more than a tiny corner overlap (>1 px) lets the cursor slip straight across, so the hidden dock could never be called there and the edge is refused with the seam reason; a single-corner diagonal touch (≤1 px) is still a wall. Choosing a seamed edge is refused in Settings and the window alike, and a remembered choice that becomes a seam after a rearrangement silently migrates to the other wall of the same screen on the next real dock (preview-only reads never persist the migration).

## Consequences

- Only left and right edges are ever offered; middle lines have no handle and the opener does nothing.
- Stored preferences survive rearrangement and replugging without user repair, at the cost of a quiet migration the user never explicitly approved — accepted because the alternative (a dock that can't be summoned) is worse.
- The arrangement/seam/eligible-edge vocabulary in CONTEXT.md is the contract the dock, the reveal gate, and the Settings copy all share.

## Amendment — 2026-09-05 (executable-source audit)

`enumerate_displays` supplies geometry and eligibility for the display surface, but native identity lookup is still implemented twice: `query_display_map` and `display_identity` in `src-tauri/src/appbar.rs` independently perform display configuration queries, and `monitor_identity` uses the latter. One native probe owner remains an architectural obligation, reinforced by ADR-0029, rather than achieved implementation locality.

`resolve_dock_prefs_migrated` in `src-tauri/src/quick_window.rs` switches a remembered seam edge to its opposite and persists that choice without checking that the opposite edge is eligible. A middle monitor with seams on both sides therefore has no guaranteed wall fallback. Explicit edge selections are checked; preview reads do not persist migration. Identity-first preference lookup and device-name fallback remain implemented. This amendment records the unvalidated fallback as a gap, not an approved exception to edge eligibility.
