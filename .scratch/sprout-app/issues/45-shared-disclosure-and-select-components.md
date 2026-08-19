# 45 — Shared disclosure & select components, Advanced section fix

**What to build:** One consistent disclosure pattern for every accordion in the app (Advanced section, preset requirement rows, Quick Launch desktop groups) and one consistent select styling for every dropdown — so no control in the app looks hand-built differently from its neighbours. The product edit form's Advanced section gets the first full adoption: a bare "Advanced" header with a chevron (no content-listing note, no count badge), properly padded body, and plain-language labels ("Environment variables", "Add variable", "Variable name"/"Value") with the install-location hint rewritten in user terms. The Library search placeholder becomes "Filter products…" so its filter semantics are unambiguous.

**Blocked by:** None — can start immediately

**Status:** ready-for-agent

- [x] A reusable disclosure component exists (chevron button with 26px hit area, hover background, rotate-on-open, `aria-expanded`/`aria-controls`, visible `:focus-visible` ring, transform-only transition respecting `prefers-reduced-motion`)
- [x] Product edit → Advanced uses it: summary reads only "Advanced", open body has real padding and `--space-4` gaps between fields (no margin-hack layout), env rows breathe
- [x] Env wiring copy is user language: "Environment variables", "Add variable", "Variable name"/"Value" placeholders, hint for the location field rewritten (no "resolve `<InstallLocation>`" jargon)
- [x] Preset composer requirement rows use the same chevron pattern (their env/verify/dep count tags stay)
- [x] A shared select treatment (explicit background/color per dark-mode guidance, custom chevron instead of the OS arrow glued to the border, consistent focus ring) replaces every native select in the app (shell picker, env action pickers, product picker, version policy)
- [x] Library search placeholder reads "Filter products…"
- [x] `npm run check` 0 errors (done: 0 errors / 0 warnings, 32 vitest tests pass); manual check of both dialogs (product add/edit, preset compose) in light and dark theme (needs a human with the app); synced to the share (done)
  - Follow-up from review: Advanced `hidden` was overridden by `display: flex` (panel permanently open) — fixed with `.advanced__body[hidden] { display: none }`; verified open/collapse via UI Automation in the live app. Advanced redesigned per feedback: no frame box, flat full-width disclosure header matching field labels, install-directory explanation moved to an InfoTip, env "None" line removed.
  - Second pass: install-location-hint explanation also moved into an InfoTip (info tone); install-directory InfoTip is now a warn-toned button (installer caveat — no guarantee); preset composer's "None — …" empty-state lines in Env wiring and Verify commands removed; PresetPacket "in your library"/"stored as authored" footer text removed (matches product card); Quick Launch "N entries — up to 8 launch at a time" page hint removed (dead `settings` load dropped).
