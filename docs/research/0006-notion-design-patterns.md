# Notion design patterns

Factual patterns from Notion's product design, captured so UI decisions here can
cite evidence instead of re-deriving it. Companion to 0004 (progressive
disclosure) and 0005 (page-chrome consistency): where those two state Sprout's
standing rules, this note records the observed method behind a shipped,
widely-learned interface that solves the same problems.

## Sources

- SaaSUI, *Notion Settings* (real screenshots of settings surfaces) —
  https://www.saasui.design/pattern/settings/notion
- The Organized Notebook, *Notion's New UI Design Update* (June 2025 redesign
  breakdown, verified against live behavior) —
  https://theorganizednotebook.com/blogs/blog/notion-new-ui-design-update-june-2025
- Brainfeed, *Notion's New UI Design Update: What's Changed* (June 2025;
  independent corroboration of the same redesign) —
  https://brainfeed.ai/pages/notions-new-ui-design-update-whats-changed-tips-and-tips-and-what-changed-tips-and-more-june-2025-6i4aT.html
- getdesign.md, *Design System Analysis: Notion* (warm minimalism, serif
  headings, soft surfaces) — https://getdesign.md/notion/design-md
- shade-solutions/notion-design-system (community-token reference: `#FFFFFF`
  surface / `#37352F` text / single `#2383E2` accent, light; `#191919` dark) —
  https://github.com/shade-solutions/notion-design-system
- notion-kit UI reference (toggles documented as accessible collapsible
  sections that keep layouts clean) — https://notion-ui.vercel.app/
- **Primary, verified first-hand 2026-08:** Notion Help Center, *List view*
  and database-view docs — https://www.notion.com/help/lists — "Open the
  settings menu at the top right of your database → `Property visibility`";
  view-scoped toggles ("Wrap column", layout options) flip state shown by an
  accent-colored switch and the panel dismisses on outside click.

## The patterns

1. **Visibility on-surface, configuration elsewhere.** Controls that show/hide
   something sit on the surface they govern; editing what that thing *is*
   lives in a dedicated configuration flow (the June-2025 split of property
   visibility from "Edit properties"). One look, one place to change your
   mind — but authoring never hides behind a visibility toggle.
2. **Minimal-until-content defaults.** Inline databases hide view tabs until a
   second view exists; a freshly created database starts extremely minimal and
   its option surface grows only as content does. Structure is never shown
   before it holds anything.
3. **Explicit-setup gating for power features.** AI properties are invisible
   until the user deliberately sets them up — capability arrives by
   appointment, not by default visibility.
4. **Contextual controls live near their object.** Per-object settings open at
   the object (its own menu); only account/global concerns centralize in the
   Settings modal. Distance between a control and its subject is friction.
5. **Relocating familiar controls has a real cost.** The same redesign's
   three-dot → slider icon change broke tutorials and muscle memory — the
   most-cited complaint across both breakdowns. Moving or renaming an
   established affordance must buy enough clarity to repay the relearning.
6. **One reserved accent.** A near-white surface with one blue accent spent
   almost exclusively on primary actions; everything else is grayscale text
   hierarchy. Color signals the single next step, matching 0005's one-primary
   rule.
7. **Collapsible sections over page splits.** Toggles/disclosure sections keep
   long layouts scannable without fragmenting into new pages — organization
   stays inline and reversible.
8. **View-scoped switches live on the surface; global concerns centralize**
   *(verified first-hand against notion.com/help, 2026-08)*. A database's
   optional features — property visibility, wrapping, layout behaviors — are
   toggles inside an on-surface settings/view-options popover anchored to the
   top right of the surface they govern ("Open the settings menu at the top
   right of your database"); state reads from the switch's accent color and
   the panel dismisses on outside click. Only account/global concerns
   (workspace, account, appearance app-wide) centralize in Settings &
   preferences. This is the direct precedent for Sprout's page-features gear
   menu (`PageFeaturesButton`, research 0008): object-scoped feature switches
   never leave their object's surface, and never migrate into the app's
   Settings screen.

## Applied case study (2026-08, ticket 85 round)

Toolbar-row Groups/Desktop toggles follow pattern 1 (visibility control on the
governed surface, group CRUD revealed there, nothing duplicated in Settings);
opt-in-by-default advanced features and absent-until-a-group-exists sections
follow patterns 2–3; desktop assignment moving into each entry's edit dialog /
row menu follows pattern 4; the Run-accent/Stop-danger mapping spends color
exactly per pattern 6; dock/window accordions reuse the Disclosure primitive
per pattern 7. Pattern 5 is the standing caution behind keeping the rail's
item labels stable while only their order changes.

*(Supersession, 2026-08: the "toolbar-row toggles" control idiom in this case
study was revised after review into a page-features gear menu — research 0008
records why and what changed. Patterns 1–8 stand unchanged; only that one
idiom moved.)*
