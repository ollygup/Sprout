# Opt-in feature controls: switches, menus, and where options live

Standing rules for any feature that ships **off by default** and needs a
switch — where that switch lives, what it looks like, and how it differs from
its two neighbors (moment-of-use dialogs, app-global Settings). Written after
ticket 88's first cut put a bare "Desktops" checkbox in a page's toolbar row;
user review rejected it, and re-validating against primary sources produced
these rules instead of taste. Future opt-in features (the Groups toggles,
tickets 90/91) apply this note directly.

## Sources

- Alita Kendrick, *Toggle-Switch Guidelines*, NN/g (2018) —
  https://www.nngroup.com/articles/toggle-switch-guidelines/ — toggles must
  take effect immediately without Save/Submit; their control-comparison table
  assigns checkboxes to submit-deferred choices and switches to instant ones.
- Kate Kaplan, *Designing Effective Contextual Menus: 10 Guidelines*, NN/g
  (2025) — https://www.nngroup.com/articles/contextual-menus-guidelines/ —
  menus reduce clutter but carry low information scent; mitigations are clear
  labels, proximity to the governed content, consistency, keyboard and
  screen-reader access.
- NN/g, *Customization Features Done Correctly* (report) —
  https://media.nngroup.com/media/reports/free/Customization_Features_Done_Correctly.pdf —
  84% of interface-customization issues were findability/page-design
  failures; users must be told what customization offers.
- Microsoft UWP toggle guidance + Material selection controls — corroborate
  the immediate-effect rule: switches for settings effective the moment they
  change, checkboxes when extra steps stand between choice and effect.
- Vercel Web Interface Guidelines (fetched 2026-08) — icon-only buttons need
  `aria-label`; label + control share one hit target; visible hover/focus
  feedback.
- Research 0006, pattern 8 *(Notion practice verified first-hand 2026-08)* —
  view-scoped feature switches live in an on-surface popover anchored to the
  governed surface's top right; account/global concerns alone centralize in
  Settings.

## The rules

1. **Placement follows persistence, not convenience.** A *durable preference*
   that reshapes its own surface gets a persistent quiet switch at that
   surface (the page-features menu). A *per-use scope choice* on an otherwise
   frequent action gets a moment-of-use dialog (research 0007 — export
   checklist). An *app-global concern* centralizes in Settings (research
   0006 patterns 1/4). Classify a new knob before choosing its home.
2. **The switch reads its value.** State is shown twice — an On/Off word and
   accent color (NN/g treats color plus state descriptor as the belt-and-
   braces option), with `role="switch"` + `aria-checked` underneath. A bare
   tick mark is not state visibility. The change applies immediately, no Save
   step — which is precisely what makes it a switch rather than a checkbox.
3. **Menus must carry their own scent.** A contextual-menu trigger hides what
   it contains (NN/g: low information scent is the idiom's price), so every
   row shows the feature name *and* a plain-language description covering
   both states. Rows are one full-width hit target (`aria-labelledby` +
   `aria-describedby`, no dead zones); Escape/outside-click closes with focus
   restored; keyboard and screen-reader operation are mandatory.
4. **Placement is owned once, by shared chrome.** The features menu renders
   through a dedicated slot in the shared PageHeader (research 0005 rules
   1/5/6), pinned to the toolbar lane's far end — never hand-placed per page.
5. **Empty means invisible.** A page with no applicable features passes no
   items and the whole control disappears (research 0004 rule 2) — below the
   Windows virtual-desktop gate there is no gear at all.

## Tensions recorded as accepted tradeoffs

- **Few actions behind a menu** (NN/g contextual-menus guideline 8 warns
  against hiding one or two actions behind an icon): Quick Launch currently
  carries exactly one switch. Accepted because Groups joins the same menu
  next (two+ rows), because both alternatives were evaluated and rejected by
  product review — bare toolbar checkbox, Settings-only — and because rule 3
  gives the rows the description text that restores scent.
- **Quiet trigger salience** (same source: don't render triggers to the point
  of invisibility): the gear uses the app's ⋯-menu treatment — muted,
  borderless until hover, but always present and never hover-only.
  Consistency with the learned in-app idiom outweighs extra salience here.

## Applied history

2026-08, ticket 88: desktop grouping first shipped as a checkbox beside the
toolbar search; review found the label cryptic, the state nearly invisible,
and toolbar parity with other pages broken. This note's rules are the
correction; the shipped result is the gear trigger + switch panel described in
rule 3, rendered through PageHeader's `features` slot per rule 4.

Supersession, 2026-08 menu/disclosure round: the desktop-assignment switch this
note was born from has since been removed entirely — research 0006 pattern 11
and ADR-0015 record why (its on-state did nothing until assignments existed,
failing the immediate-effect contract in rule 2's own source). The rules stand;
Groups remain their live applied case, and the annotation-vs-structure
classification in 0006 pattern 12 now decides which future knobs get a switch
at all.

Applied history, 2026-09 quick-access round: a blanket checkbox → toggle
conversion was proposed and refused under rule 2 — in-dialog flags (stoppable,
auto-run) and the export scope checklist (research 0007) apply on
Save/confirm, so switches there would lie about immediacy. Convention set:
each ticket audits the checkboxes on the surfaces it touches and converts
instant-effect ones only (ticket 128 step 0), instead of a standalone
conversion pass.
