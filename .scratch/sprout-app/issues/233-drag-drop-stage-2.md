# 233 — Drag & drop stage 2: nested group reparent/unnest

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** Groups become directly draggable across the two-level tree. A child group drags to another parent or back to the top level; an item-only top-level group drags into another top-level group; a group with children only reorders at the top level. Dropping on a header nests, dropping on an insertion line reorders, and dropping on the top-level empty area unnests. Invalid drops explain why, and the ⋯ "Move into…" path remains complete.

**Blocked by:** 232 — Drag & drop stage 1; 230 — Nested groups main-app UI.

**Status:** ready-for-agent

- [ ] Child groups drag between parents and to the top level; item-only top-level groups drag into another top-level group; groups with children reorder at the top level only.
- [ ] Drop on a group header nests into that group (when eligible); drop on an insertion line reorders at that level; drop on the top-level empty area unnests. Dropping on the current parent is a no-op.
- [ ] Invalid targets are refused with a not-allowed cursor and a short reason: depth 3, cycle/self, a group with children as a child, cross-collection, or a name collision in the destination.
- [ ] A group with children is a valid parent but never a valid child; a child cannot become a parent in the same gesture.
- [ ] Reparent/reorder uses the transactional group move contract; a failure reloads and surfaces the existing error line.
- [ ] The ⋯ "Move into…" menu remains the complete keyboard path; drag is an accelerator, never the only way.
- [ ] No dock drag, no cross-collection drag, no third level, and no drag while filtering.
- [ ] Pure tests cover nested drop-target resolution and every refusal; backend tests cover the move validation already owned by 229. Manual evidence covers all three collections, deep/wide trees, and keyboard parity.
- [ ] `cargo test`, `cargo check`, `npm run check`, `npm run test`, and `node tools/ownership-gate.mjs` pass.

**Claims (recheck at dispatch):** the shared drag controller; group headers; the transactional group move. **Explicitly not built:** a third level; auto-creating parents on drop; dock drag; cross-collection moves; changing the Move into… menu contract.
