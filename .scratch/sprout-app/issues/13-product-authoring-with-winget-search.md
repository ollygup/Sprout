# 13 — Product authoring with winget search

**What to build:** Adding or editing a Product no longer asks the user to invent a winget ID. The backend gains `winget search` and `winget show` commands; the add/edit dialog offers a live search field that returns real registry matches (name · publisher · version · source) to pick from. If nothing matches or the source is unreachable, a quiet progressive-disclosure link ("Not found? Type the ID manually") reveals a plain text field — authoring never blocks on the network. Products gain created/updated timestamps (lazy migration for existing databases). The dialog layout is overhauled: controls inside the frame, consistent spacing, readable density — the current overflow/spacing issues are gone.

**Blocked by:** 11 — App shell and design foundation; 12 — Products page rebuild

**Status:** done

- [x] Backend `winget search` / `winget show` commands run with `--accept-source-agreements` and non-interactive flags; output parsing is robust (JSON where supported, exact-row fallback) and resilient to localized output; a slow first search shows a "Searching…" state, never a hang
- [x] Add/Edit dialog: typing in the Winget ID field live-searches and shows a picker of real matches; picking fills the ID from the registry; existing manual entry reachable only via the quiet fallback link
- [x] Source unreachable or zero matches never blocks authoring — the manual field is one click away, with no warning badges or validation states anywhere
- [x] `created_at`/`updated_at` columns added to products with lazy migration; maintained on create/update; surfaced in the More info dialog (ticket 12's surface)
- [x] Dialog layout: all controls inside the frame, consistent spacing, no overflow, readable density
- [x] Backend tests cover search parsing edge cases and the migration on a pre-existing database
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok