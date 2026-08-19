# 33 — UI copy rewrite to plain technical style

**What to build:** All user-facing text in the Library, Presets, Plan, History, Logs, and Settings screens — titles, subtitles, empty states, notices, dialogs, context menus, and aria-labels — is rewritten to plain technical-documentation style: no "Sprout never…" phrasing, no plant/grow/sprout wordplay, no dash-split aphorisms, short concrete sentences stating actual behavior. The rotating loading phrases collapse to a single "Loading…". The preset composer's copy is handled by ticket 35.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [x] Every screen's titles, subtitles, empty states, and notices follow the agreed register (exemplars: "Products", "N products. Right-click a card or open its ⋯ menu for actions.", "Conflicts need a decision before the run starts.", "Run records are kept indefinitely; raw log files expire per the retention setting.")
- [x] Loading-phrase rotators replaced with a single "Loading…" on all screens
- [x] No user-facing string uses Sprout as a sentence subject or describes what it "never" does; no wordplay outside the logo
- [x] Composer copy untouched (belongs to ticket 35)
- [x] `npm run check` 0 errors