# Monitors are told apart by hardware identity; seams refuse the dock

`enumerate_displays` is the single source for geometry and identity: each screen's rectangle plus its EDID identity (`edid-XXXX-YYYY`). Dock memory (edge, mode, width, companion ratio) is keyed identity-first with device-name fallback, so replugging a panel elsewhere keeps its preference and pre-identity rows still resolve; an all-zero EDID keys as `None`. An edge is eligible for docking only when it is a real wall — a seam where two screens touch by more than a tiny corner overlap (>1 px) lets the cursor slip straight across, so the hidden dock could never be called there and the edge is refused with the seam reason; a single-corner diagonal touch (≤1 px) is still a wall. Choosing a seamed edge is refused in Settings and the window alike, and a remembered choice that becomes a seam after a rearrangement silently migrates to the other wall of the same screen on the next real dock (preview-only reads never persist the migration).

## Consequences

- Only left and right edges are ever offered; middle lines have no handle and the opener does nothing.
- Stored preferences survive rearrangement and replugging without user repair, at the cost of a quiet migration the user never explicitly approved — accepted because the alternative (a dock that can't be summoned) is worse.
- The arrangement/seam/eligible-edge vocabulary in CONTEXT.md is the contract the dock, the reveal gate, and the Settings copy all share.
