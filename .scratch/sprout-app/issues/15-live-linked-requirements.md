# 15 — Live-linked requirements and product delete impact

**What to build:** Requirements stop embedding stale product copies. A Requirement references its Product by id and resolves the current name and winget step live from the library at compose/plan/run time — editing a Product propagates to every preset using it, no preset editing needed. Imported presets keep their embedded snapshot (they are snapshots by definition); exports remain point-in-time snapshots. Deleting a Product prompts "It will also be removed from N preset(s) that contain it" and drops those requirements (imported presets keep theirs). Unresolvable requirements show a clear "product removed from library" state and are excluded from runs until resolved. The domain docs are updated: CONTEXT.md (Library retired from user language, Requirement = live reference, Quick install, attention outcome) plus ADR-0007 recording the live-link decision and its interaction with the immutable-presets ADR.

**Blocked by:** 12 — Products page rebuild; 13 — Product authoring with winget search; 14 — Presets page and composer rebuild

**Status:** done

- [x] Editing a Product's name or winget step changes every preset requirement that references it — verified in composer preview and plan computation without touching the preset
- [x] Imported presets keep their embedded snapshot; export writes a current snapshot
- [x] Deleting a Product prompts the preset impact ("also removed from N preset(s)") and drops those requirements; imported presets unaffected; run history keeps its records
- [x] Composer shows "product removed from library" for unresolvable requirements; such requirements are excluded from runs until re-linked or edited
- [x] Backend tests cover: propagation on edit, delete-with-impact, imported-preset snapshot retention, unresolvable exclusion
- [x] CONTEXT.md updated; ADR-0007 written (live-linked requirements vs. embedded copies; interaction with ADR-0005)
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok