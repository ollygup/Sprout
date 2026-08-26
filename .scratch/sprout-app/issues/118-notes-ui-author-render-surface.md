# 118 — Notes UI: author, render, surface

**What to build:** The Quick Action edit dialog gains a Notes field authored as plain text with simple bullets and numbered lists (hint included). Clicking an action row opens a centered details dialog — the same grammar Products use — showing the action with its rendered read-only note beside Run/Edit. Rows carrying a note show a small glyph in every list surface, including the compact window/dock lists where the glyph appears alone, content-free.

**Blocked by:** 117 — Note storage + API for Quick Actions.

**Status:** ready-for-agent

- [ ] Read-only formatter renders paragraphs, `-`/`*` bullet lists, and `1.`-style ordered lists; everything else escapes verbatim; covered by a unit test suite
- [ ] Edit dialog offers the Notes textarea with an authoring hint; clearing it removes every trace of the note (no ghost glyphs)
- [ ] Row click opens the centered details dialog built on shared Dialog primitives (focus trap, Escape, focus restore) matching the Product-details grammar
- [ ] The carrying-a-note glyph is content-gated: absent without a note, shown in main list and compact window/dock lists (glyph only there — no note content on constrained surfaces)
- [ ] Rendering styled exclusively from existing tokens/typography; `svelte-check` clean
