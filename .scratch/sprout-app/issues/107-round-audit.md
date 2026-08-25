# 107 — Round audit: menus, virtual desktop, groups, dedup, scroll, accordion

**What to build:** Close the round: apply the web-interface-guidelines review over every changed component (ContextMenu + flyout, Disclosure/GroupAccordion, library page header/menus/badges, quick-actions/clips menus, New-group dialog, Quick Launch window tabs), verify the docs written this session still match what shipped, and run the full gate set. Confirm no regression in the surfaces the round only touched transitively (presets/products menus kept their structure while adopting the standard's ordering/danger placement).

**Blocked by:** 101, 102, 103, 104, 105, 106.

**Status:** ready-for-agent

- [ ] Web-interface-guidelines review clean or findings fixed across changed components
- [ ] Keyboard-only walkthrough: every menu incl. submenus operable end-to-end
- [ ] Light/dark passes on all six builder surfaces + window/dock both variants
- [ ] Docs match shipped behavior: CONTEXT.md terms, ADR-0015, research 0006 patterns 9–12, 0008 supersession line
- [ ] Gates: `npm.cmd run check` 0 errors/warnings; `vitest run` green; `cargo test` green; `npm.cmd run build` clean
- [ ] `tools\sync.ps1 -Up` reports round artifacts synced; re-run shows `0 copied`
