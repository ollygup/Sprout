# 78 — Quick Clips backend + main-app page

**What to build:** The Clip domain (glossary): a machine-local, hand-authored
plain-text list with one-click re-copy. New storage + commands mirroring the
launch-entry shape, and a new main-app page with search, add/edit/delete,
reordering — plus the nav entry that makes it discoverable (the window tab's
discoverability home per research 0004 rule 2).

**Blocked by:** 72 — the spec fixing authoring (manual paste only), content
shape (plain text, name-from-first-line), and ordering (user-controlled)

**Status:** done (2026-08-23)

- [x] `clips` table: id, name (optional — empty string when untitled), content (non-empty after trim), position; created lazily via the existing db init path
- [x] db CRUD + reorder functions and tests mirroring the launch-entry tempdir suite (create/list/update/delete/move incl. position swap integrity)
- [x] Commands following launch-entry naming/shape: list/create/update/delete/move, validation errors honest ("clip text can't be empty")
- [x] Clipboard write command: puts a clip's content on the clipboard via the clipboard-manager plugin (Rust side), returning success so surfaces can flash feedback
- [x] `/clips` page modeled on existing editor pages: shared search input filtering name+content client-side; add dialog with textarea + optional name (placeholder previews first line); edit dialog; delete confirm; up/down reorder identical in feel to Quick Actions
- [x] Empty state guides first use and mentions the window tab appearing once a clip exists
- [x] Nav rail gains "Quick Clips" positioned with its siblings; api/types additions typed end-to-end
- [x] Accessibility pass mirrors sibling pages (labels, focus, aria-live for copy feedback groundwork used by ticket 79)
- [x] `cargo test` green; `npm run check` 0 errors; synced to the share
