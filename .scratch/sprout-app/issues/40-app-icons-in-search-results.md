# 40 — App icons in search results

**What to build:** Search results show each app's icon so the user can recognize the app at a glance. Icons are extracted lazily for visible rows only, held in memory, and never cached to disk. Parent spec: 37.

**Blocked by:** 39 — Installed-app search

**Status:** done

- [x] `candidate_icon` command: extracts the icon for a candidate target (SHGetFileInfoW → GDI bitmap → RGBA → PNG via the `png` dependency), returned as a data URL; graceful `None` when no icon exists (uninstalled exe, odd targets)
- [x] `png` 0.17 direct dependency added; GDI icon extraction is self-contained and leaks no handles (DestroyIcon/DeleteObject on all paths)
- [x] Search results render the icon for visible rows only, fetched per row on demand; scrolling fetches only newly visible rows
- [x] `cargo test` green (icon extraction is manual-verify — no headless test), `npm run check` 0 errors; synced to the share