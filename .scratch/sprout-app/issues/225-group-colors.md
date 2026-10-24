# 225 — Group colors

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** A group can carry one color — a curated hue or any `#RRGGBB`. The group header shows a 3 px accent bar, the ⋯ menu shows a color dot, and the group body gets a tint of at most 12 % plus a 1 px outline. The group menu opens a small color dialog with ten swatches, a custom hex field, and "No color". Color is decorative only, survives rename/reorder/reparent, and is never written to an export or backup.

**Blocked by:** None — can start immediately. Do not run concurrently with 229 (shared `groups.rs`/`db.rs`).

**Status:** complete — schema, command, menu dot, dialog, and rendering landed; gates and light/dark evidence recorded below.

- [x] `groups.color` is nullable text; a migration adds it with existing groups neutral. Stored values are `#RRGGBB` only (no alpha, no shorthand, no named/rgb/hsl forms), normalized to lowercase; `NULL` clears.
- [x] `Group` carries `color: string | null`; `list_groups` returns it; a `set_group_color` command writes it; invalid values return a readable message and are never stored.
- [x] The group ⋯ menu has a Color row with a dot showing the current color; it opens a dialog with ten curated swatches, a custom `#RRGGBB` field, and "No color".
- [x] The dialog uses the existing dialog contract: focus trap, Escape cancels, inline error, keyboard-operable swatches, immediate apply on choose.
- [x] A colored group renders a 3 px left accent bar, a body tint of at most 12 %, and a 1 px outline; text, icons, badges, focus rings, and menu surfaces stay token colors.
- [x] Color is never the only signal: name, count, position, and menu remain sufficient. The accent/dot/outline pass the project's non-text contrast gate in both themes.
- [x] Colors survive rename, reorder, and reparent; clearing returns the group to neutral; child groups can have independent colors and do not inherit.
- [x] Export/backup never emits `color`; Presets, Plans, Runs, and Quick Action files are unaffected. A regression test asserts the backup document shape has no group color.
- [x] Backend tests cover valid/invalid/cleared colors; frontend pure tests cover hex parsing/normalization and palette selection.
- [x] `cargo test`, `cargo check`, `npm run check`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass; manual light/dark evidence is recorded.

**Claims (recheck at dispatch):** group schema/migration and validation; `Group` type/API; `collectionGroups.svelte.ts`; `GroupAccordion`; a new color dialog; i18n; backup-shape test. **Explicitly not built:** adding groups to the backup document; a user-managed palette; gradients, patterns, alpha, or color names; color inheritance; color in the dock's group headers beyond the shared rendering.

## Evidence — 2026-10-24

**Backend.** `groups.color` is added to the schema and to `ensure_group_color` in `src-tauri/src/db.rs` (nullable, existing rows neutral). `groups.rs` gains `Group.color`, the shared `SELECT_GROUP_SQL`/`group_from_row` read, `validate_group_color` (exactly `#` + six ASCII hex digits, lowercased), and `set_group_color` (single-column update, `None` clears, missing id refused). `set_group_color` is registered in `lib.rs`. Tests: `group_color_accepts_hex_normalizes_and_clears` (uppercase normalization, six invalid forms refused without overwriting, clear, unknown id) and `colors_survive_rename_and_reorder_and_stay_per_group` (rename, reorder, and per-group independence with no inheritance/spillover). `backup.rs` adds `backup_document_never_carries_group_color`, which writes a colored group, exports, and asserts the document contains neither `"color"` nor the stored value.

**Frontend.** `types.ts` adds `Group.color`; `api.ts` adds `setGroupColor`. `src/lib/groupColor.ts` owns the ten-hue palette (each stored as one hex, no per-theme variants) plus `normalizeGroupColor`/`parseGroupColor`; `groupColor.test.ts` covers parsing, normalization, and palette selection. `GroupAccordion.svelte` renders the colored anatomy: a 3 px `::before` bar on the header, `color-mix(in srgb, var(--group-color) 10%, transparent)` body tint, and a 1 px inset outline; the header/body keep the shared title-column alignment. `ContextMenu.svelte` gains the `swatch` slot (filled dot, hollow neutral ring) and `collectionGroups.svelte.ts` adds the Color menu row after Rename, the dialog state, and immediate apply. `GroupColorDialog.svelte` renders the ten swatches (5 × 2, labelled and `aria-pressed`), the custom hex field, the inline error, and "No color" inside the shared `Dialog` contract. The three surfaces (Quick Launch, Quick Actions, Clips) pass `color` and host the dialog. 22 `groups.color*` keys were added to `en.json` and `zh-CN.json` (1203 keys each, parity kept).

**Applied research.** 0004 rule 2 (split by frequency): color is set-once configuration, so it lives behind the group's own ⋯ menu rather than as an always-visible control; 0004 rule 3 (at most two disclosure levels): menu → dialog is the two levels, with no third surface. 0006 pattern 4 (contextual controls live near their object): the color control opens from the group it paints; 0006 pattern 10 (menu grammar): the row carries a glyph slot and sits with the other non-destructive verbs, destruction stays last.

**Automated gates (all in `C:\Sprout`).**

- `cargo test` — 716 passed, 3 ignored, 0 failed.
- `cargo check` — pass (pre-existing warnings only).
- `npm.cmd run check` — 0 errors, 3 pre-existing warnings (`QuickActionFormDialog.svelte`, `UnifiedHeader.svelte`; untouched by this ticket).
- `npm.cmd test` — 452 passed across 37 files.
- `node tools/contrast-check.mjs` — all checks pass in both modes. The ten palette entries report 4.07:1 (light page) / 3.77:1 (light surface) and 4.32:1 (dark page) / 3.77:1 (dark surface) at worst, against the 3:1 non-text requirement; indigo and slate are the tightest (3.13:1 dark surface).
- `node tools/ownership-gate.mjs` — pass (62 owned references).

**Manual light/dark evidence.** `tools/repro-group-colors.mjs` (new, kept as a regression harness) launches the app through CDP against a throwaway `%LOCALAPPDATA%` directory, seeds three colored groups plus one neutral, and measures the rendered anatomy in both themes; it finished **GREEN** on all checks. Screenshots and `report.json` live in `C:\Users\admin\AppData\Local\Temp\opencode\evidence-225\` (`library-light.png`, `library-dark.png`, `menu-light.png`, `menu-dark.png`, `dialog-light.png`, `dialog-dark.png`, `dialog-error-light.png`). Measured:

- Accent bar exactly `3px` on every colored group; neutral groups have no bar.
- Body tint resolves to `color(srgb … / 0.1)` — 10 %, under the 12 % ceiling; neutral bodies stay transparent.
- Outline is `inset 0 0 0 1px` of the hue; accent-on-page contrast 4.07:1 light / 4.32:1 dark; outline-on-tinted-body 3.55–3.62:1 light / 3.87–3.96:1 dark.
- Every group's title and count badge keep the same token color (`rgb(111, 110, 105)` light, `rgba(255, 255, 255, 0.5)` dark) regardless of color.
- The ⋯ menu's Color row shows a ~10 px dot in the group's hue; the neutral group shows the hollow ring.
- The dialog renders ten labelled swatches with the current one marked, takes focus inside itself, and Tab from the last control wraps back to the first.
- Escape closes the dialog without writing; an invalid custom value (`not-a-color`) shows the inline error and leaves the stored color unchanged; activating a swatch applies immediately (`#967b29` stored, dialog closed).

**Notes.** Reparent survival is structural rather than exercised end-to-end: nested groups do not exist until 229, and `color` is a single column keyed by group id, independent of `position`, membership, and the future parent link (the rename/reorder test already proves the value is untouched by structural moves). The Quick Launch window's group headers pass no `color`, per the ticket's explicit out-of-scope note.
