# 14 — Presets page and composer rebuild

**What to build:** The Presets page in the seed-catalog identity: packet-style preset cards with the context-menu system (Edit · Fork · Export · Remove; imported presets: Fork · Export · Remove), keeping the "+ New preset" and "Import" top-level buttons. Copy, empty state, and dynamic loading aligned with the Products voice. The preset composer dialog gets the readability overhaul: spacing, controls inside the frame, and progressive-disclosure info affordances (click/keyboard popovers) replacing the persistent hint paragraphs. Imported presets remain stored as authored, fork-to-edit.

**Blocked by:** 11 — App shell and design foundation; 12 — Products page rebuild; 13 — Product authoring with winget search

**Status:** done

- [x] Preset cards render in packet style with the context menu (Edit · Fork · Export · Remove; imported: Fork · Export · Remove); "+ New preset" and "Import" stay as visible buttons
- [x] Page copy and empty state rewritten in the established voice; loading messages rotate dynamically; no raw backend strings in errors
- [x] Composer dialog overhauled: consistent spacing, controls inside the frame, readable density, no overflow
- [x] Persistent hint paragraphs collapsed into info affordances (popover on click and keyboard — not hover-only); empty states keep their guidance inline
- [x] Import/export flows and the imported "stored as authored, fork to edit" behavior unchanged
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok