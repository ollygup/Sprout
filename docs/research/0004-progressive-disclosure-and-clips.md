# Progressive disclosure & compact-surface navigation

Standing rules for deciding what shows on any constrained surface (the Quick
Launch window/dock today; any future compact palette later). Written once so
future enhancements apply the same test instead of re-deriving it.

## Sources

- Jakob Nielsen, *Progressive Disclosure*, NN/g (2006) — https://www.nngroup.com/articles/progressive-disclosure/
- Smashing Magazine, *Responsive Navigation Patterns* (citing NN/g's "if you can show navigation, show it") — https://www.smashingmagazine.com/2017/04/overview-responsive-navigation-patterns/
- Brad Frost / Michael Scharnagl, *Priority+ Navigation Pattern* (via CSS-Tricks) — https://css-tricks.com/the-priority-navigation-pattern/
- uxpatterns.dev, *Tabs Pattern* — https://uxpatterns.dev/patterns/navigation/tabs
- UX StackExchange, *Desktop icons best practices* (labeling case studies) — https://ux.stackexchange.com/questions/131470/
- stellae.design, *Copy to Clipboard* — clipboard writes require visible feedback

## The rules

1. **Show navigation if you can.** Hiding surfaces behind toggles or overflow
   menus is the last resort. Priority+ ("⋯ more") exists for many items —
   never justify it with fewer than ~5.
2. **Split by frequency of use** (NN/g): features needed often stay visible;
   rare ones wait behind an obvious affordance. A surface may also stay
   *entirely absent* until it has content — an empty feature must not occupy
   chrome — provided another surface advertises it (discoverability home).
3. **At most two disclosure levels**: level 1 = fast read-only access;
   level 2 = full configuration elsewhere. Never put settings inside a
   quick-access surface.
4. **Tab hygiene** (uxpatterns.dev): 2–7 tabs, labels of 1–3 words. When a
   strip runs out of room, degrade in order: full label → shortened label →
   icon-only (icons require tooltip + `aria-label`; they are learned
   vocabulary, not self-describing). Verify fitting at real device DPI, not a
   1× screenshot.
5. **State-changing quick actions need feedback** (a "Copied" flash, a toast,
   a row highlight). Silence reads as breakage.

## The constraint that motivates this

`DOCK_WIDTH === WINDOW_WIDTH === 340` physical px (`constants/window.rs`,
asserted in `appbar.rs`). Physical-pixel sizing means high-DPI displays render
the window physically smaller while text keeps its CSS size — label fitting
must be measured at runtime or tested per-DPI.

## Applied case studies

- **Quick Clips tab (2026-08)**: rule 2 — the tab appears only once ≥1 Clip
  exists (main app page is the discoverability home); rule 3 — the window tab
  is read-only click-to-copy, all CRUD lives on `/clips`; rule 4 governs the
  full → short → icon label degradation; rule 5 mandates the Copied flash.
- **Groups sections (2026-08)**: rule 2 — group sections, row-menu assignment,
  and management affordances all stay absent until at least one group exists;
  the feature switch (page-features menu per research 0008) plus a New group
  button are the only chrome while the collection has no groups.
