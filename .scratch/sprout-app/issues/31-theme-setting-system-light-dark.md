# 31 — Theme setting (System / Light / Dark)

**What to build:** Settings gains a Theme control with three options — System (follows Windows), Light, Dark. Selecting one applies immediately across the whole app; the choice persists and is honored on restart. Dark mode keeps the current design-token look.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] Settings exposes a Theme control, defaulting to System; System tracks the OS preference live
- [x] Explicit Light or Dark overrides the system preference everywhere, including native scrollbars/inputs (`color-scheme`)
- [x] Theme applies the moment it is selected, without saving the rest of the settings form; persisted and restored on restart
- [x] Backend settings tests cover the new key's default, roundtrip, and validation
- [x] `npm run check` 0 errors