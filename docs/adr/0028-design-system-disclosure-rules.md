# One design system and disclosure rules govern every screen

> Latest status: amended 2026-09-09 for selective field guidance, dock visibility discovery and filter/action scope; implementation pending in 167-169 under spec 166. See the final amendments; earlier text is preserved.

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

`tokens.css` (Planting Ledger: Notion-derived palette, dual `data-theme`, type/space/shape/motion, WCAG-AA gated by `contrast-check`) is the sole source of visual values; shared `components/` (page header + features-menu slot, dialogs, disclosures, packet cards, tabs) is the sole source of patterns. No screen introduces ad-hoc colors, type sizes, radii, or one-off components — a deviation needs a ticket note and review before shipping. Placement follows the standing research instead of taste: progressive disclosure and clips (content-gated tabs, two levels max, tab hygiene — research 0004), page-chrome consistency (research 0005), Notion's factual method (visibility-on-surface vs configuration-elsewhere, minimal-until-content defaults, explicit-setup gating, view-scoped switches — research 0006, patterns 2–3, 8, 11–12), export scope as moment-of-use dialogs (research 0007), and feature menus over toolbar checkboxes (placement-follows-persistence, switch-reads-value, scent, owned-once, empty-invisible — research 0008). The version string follows the same single-source doctrine as geometry: `Cargo.toml` owns it, `tauri.conf.json` omits it, the UI reads it via `getVersion()`.

## Consequences

- New screens compose; they don't invent. Review starts with "which token, which component, which research rule" — anything else is the exception that proves the system.
- Research notes stay the evidence locker; this ADR is the lock: the rules are decisions now, not suggestions.

## Amendment — 2026-09-05 (executable-source audit)

The token/component rule remains mandatory, but the absolute claim that the code contains no local visual values is not accurate. For example, `.rail__wordmark` in `src/lib/components/NavRail.svelte` sets `font-size: 1.125rem`; the Quick Actions page and Quick Launch page contain `border-radius: 999px` rather than the existing radius token. These are implementation gaps against the decision, not newly approved exceptions. Shared `PageHeader` still owns the header/features slot, and `PageFeaturesButton` still hides an empty feature list and renders labeled On/Off switches.

`tools/contrast-check.mjs` is a standalone checker over a manually copied `palettes` object and an explicit list of contrast pairs. It does not load `tokens.css` or inspect rendered screens. Neither the `package.json` check/build scripts nor `.github/workflows/release.yml` invokes it, so "WCAG-AA gated" describes a required verification practice, not an automatic repository-wide or release-CI guarantee. A passing invocation alone would not prove that every current screen or token matches those copied inputs.

The application-version rule is implemented: `src-tauri/Cargo.toml` owns the app version, `src-tauri/tauri.conf.json` has no version property, and the frontend update state obtains the running version through `getVersion()`. The disclosure rules remain accepted design policy; this source-only audit does not claim to verify past visual reviews or the external research.

## Amendment — 2026-09-08 (selective field guidance and visibility discovery)

Accepted design, implementation pending in spec 166. Apply helper text selectively across all Sprout-authored fields: remove repeated keyboard tutorials and label restatements; retain concise constraints, defaults, validation, and consequential behavior. Improve unclear labels before adding explanations. Preserve the existing dialog submission grammar. Research 0010's per-textarea permanent-hint requirement is superseded; source inventory and proposed field dispositions live in research 0017.

Dock-hidden Launch entries, Quick Actions and Clips gain an informational eye-off annotation meaning Hidden from dock. The main-app toolbar gains a progressively disclosed Dock visibility filter with All / Shown in dock / Hidden from dock choices. Options wait behind an explicit trigger beside search; active filtering remains visible and resettable. This is a list filter, not a feature-enable switch or a hidden-state mutation. Filter lifetime and Start-all/reorder consequences remain open in spec 166. Existing shared components and design tokens remain required.

## Amendment — 2026-09-09 (accepted filter lifetime and action scope)

The second design round is accepted in 166, implemented by 168/169. Dock visibility choices are page-local, reset to All on page exit, and appear behind a clear trigger only if the full collection has hidden items or a non-All choice is active. An active filter and reset remain available with zero results; text search does not erase their discoverability. Search and visibility intersect. Reordering is unavailable while either filter is active; clear filters to reorder. Main Quick Launch uses Start matching (N) for the matching set, including collapsed groups, and disables it for zero matches. These settle the questions left open in the previous amendment; implementation remains pending.

Field cleanup in 167 also covers AI authoring fields added since the first audit: preserve the AI request/context Ctrl+Enter-to-generate ownership while removing its repeated keyboard teaching. The ordinary dialog submit grammar remains unchanged. The latest coverage addendum is research 0017.

## Amendment — 2026-09-12 (AI two-view dialog; spec 181, ticket 183)

The AI-first inline hero stacked above the full manual form (research 0006 applied case, 0019) is superseded for the Quick Action Add/Edit dialog: one dialog with two exclusive views behind `AI draft | Manual` tabs top-right (0004 rule 4 tab hygiene; 0006 pattern 8 view-scoped on-surface). The tab strip is entirely absent when AI is not ready (0004 rule 2; 0006 pattern 11 content-gated activation). Add opens AI-first, Edit opens manual-first; flips are instant with both labels visible for scent (0008 rules 1–3). The AI view holds a single `Describe what to do` textarea plus Generate and outcome (0006 pattern 2 minimal-until-content; 0006 pattern 1 config-elsewhere); discovery/disclosure appear only when needed, restoring the two-level maximum (0004 rule 3). `Use this draft` applies shell plus command and auto-flips to Manual for full review with applied feedback (0004 rule 5; HAX G9 accordion-editing). Standing token/component, single-size-source, and 167 copy rules are unchanged. Implementation pending in 183; research 0006 and 0019 gain dated decision updates on delivery.
