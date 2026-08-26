# 107 — Round audit: menus, virtual desktop, groups, dedup, scroll, accordion

**What to build:** Close the round: apply the web-interface-guidelines review over every changed component (ContextMenu + flyout, Disclosure/GroupAccordion, library page header/menus/badges, quick-actions/clips menus, New-group dialog, Quick Launch window tabs), verify the docs written this session still match what shipped, and run the full gate set. Confirm no regression in the surfaces the round only touched transitively (presets/products menus kept their structure while adopting the standard's ordering/danger placement).

**Blocked by:** 101, 102, 103, 104, 105, 106.

**Status:** done — code review clean (one finding fixed); hands-on keyboard, hover-feel, and light/dark passes still owed a human

- [x] Web-interface-guidelines review clean or findings fixed across changed components
- [ ] Keyboard-only walkthrough: every menu incl. submenus operable end-to-end
- [ ] Light/dark passes on all six builder surfaces + window/dock both variants
- [x] Docs match shipped behavior: CONTEXT.md terms, ADR-0015, research 0006 patterns 9–12, 0008 supersession line
- [x] Gates: `npm.cmd run check` 0 errors/warnings; `vitest run` green; `cargo test` green; `npm.cmd run build` clean
- [x] `tools\sync.ps1 -Up` reports round artifacts synced; re-run shows `0 copied`

**Verification notes (2026-08-26):**

Guidelines review over the round's changed components — ContextMenu root +
flyout, Disclosure/GroupAccordion, library page header/menus/badges,
quick-actions/clips menus, GroupNameDialog, PageFeaturesButton panel, Quick
Launch window tabs/lists:

- **One finding, fixed:** destination items marked with the check icon
  conveyed current state only visually — no semantics for assistive tech.
  `ContextMenuItem.checked?: boolean` added; defined items announce as
  `role="menuitemradio"` + `aria-checked`, and a checked item draws the check
  glyph without an explicit icon. All three list pages' builders now pass
  `checked` instead of conditional check icons (`moveToGroupChildren`, the
  Virtual desktop flyout).
- Passes worth recording: global `:focus-visible` ring applies inside menus
  (no outline resets anywhere); reduced-motion collapse is global; flyouts
  clamp to the viewport with inner scroll for long child lists; danger color
  rides only Remove rows; every destructive verb opens its ConfirmDialog;
  flash notices route through Notice (`status`/`alert` live roles); feature
  switches are belt-and-braces per research 0008 rule 2 (word + accent +
  `aria-checked` under `role="switch"`, one full-width hit target);
  Disclosure caret is transform-only; long names truncate at every level
  (rows, group titles, badges, targets).
- Keyboard model verified at code level (101's contract): ArrowRight/Enter
  open a flyout, Up/Down/Home/End traverse children, Escape returns focus to
  the parent row, Escape at root restores the trigger, Tab closes everything,
  disabled parents never open by pointer or key. The hands-on end-to-end
  walkthrough stays open above — code path review is not a screen-reader pass.

No-regression check on the transitive surfaces: Products keeps [Install now /
Edit / More info | Remove], Presets keeps [Plan with this / Edit? / Fork? /
Export | Remove] — structure unchanged, both gained only the separator +
danger-last placement. Window/dock tabs render through the shared
GroupAccordion/Tabs primitives this round already touched; their lists keep
the internal scroll from ticket 102.

Docs cross-check: CONTEXT.md's Launch entry / Virtual desktop / Group terms
match shipped behavior (annotation-without-switch; assignment-born groups
that dissolve when empty; name exclusivity now in the Group term). ADR-0015's
"opting out means unassigning entries individually via the explicit No
assignment item" matches the shipped submenu verbatim. Research 0006 patterns
9–12 stand as written; 0008's supersession line matches what shipped.

Final gates on the audited tree (incl. the audit fix and the 0.6.0 version
bump): `cargo test` 371 passed / 0 failed / 1 ignored (live probe),
`npm.cmd run check` 0 errors / 0 warnings, `vitest run` 36 passed,
`npm.cmd run build` clean.
