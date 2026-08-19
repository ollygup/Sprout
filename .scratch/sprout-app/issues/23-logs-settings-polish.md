# 23 — Logs and Settings polish

**What to build:** The two residual pages token-migrated and copy-polished: dynamic loading messages, the established voice, and error states that say what happened + next step instead of raw backend strings. No functional changes to the knobs or log listings.

**Blocked by:** 11 — App shell and design foundation

**Status:** done

- [x] Both pages fully token-migrated (no hardcoded colors/radii/10px sizes)
- [x] Loading messages rotate dynamically; copy aligned with the voice; no raw backend strings in errors
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok