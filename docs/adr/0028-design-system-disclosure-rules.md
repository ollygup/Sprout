# One design system and disclosure rules govern every screen

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

`tokens.css` (Planting Ledger: Notion-derived palette, dual `data-theme`, type/space/shape/motion, WCAG-AA gated by `contrast-check`) is the sole source of visual values; shared `components/` (page header + features-menu slot, dialogs, disclosures, packet cards, tabs) is the sole source of patterns. No screen introduces ad-hoc colors, type sizes, radii, or one-off components — a deviation needs a ticket note and review before shipping. Placement follows the standing research instead of taste: progressive disclosure and clips (content-gated tabs, two levels max, tab hygiene — research 0004), page-chrome consistency (research 0005), Notion's factual method (visibility-on-surface vs configuration-elsewhere, minimal-until-content defaults, explicit-setup gating, view-scoped switches — research 0006, patterns 2–3, 8, 11–12), export scope as moment-of-use dialogs (research 0007), and feature menus over toolbar checkboxes (placement-follows-persistence, switch-reads-value, scent, owned-once, empty-invisible — research 0008). The version string follows the same single-source doctrine as geometry: `Cargo.toml` owns it, `tauri.conf.json` omits it, the UI reads it via `getVersion()`.

## Consequences

- New screens compose; they don't invent. Review starts with "which token, which component, which research rule" — anything else is the exception that proves the system.
- Research notes stay the evidence locker; this ADR is the lock: the rules are decisions now, not suggestions.

## Amendment — 2026-09-05 (executable-source audit)

The token/component rule remains mandatory, but the absolute claim that the code contains no local visual values is not accurate. For example, `.rail__wordmark` in `src/lib/components/NavRail.svelte` sets `font-size: 1.125rem`; the Quick Actions page and Quick Launch page contain `border-radius: 999px` rather than the existing radius token. These are implementation gaps against the decision, not newly approved exceptions. Shared `PageHeader` still owns the header/features slot, and `PageFeaturesButton` still hides an empty feature list and renders labeled On/Off switches.

`tools/contrast-check.mjs` is a standalone checker over a manually copied `palettes` object and an explicit list of contrast pairs. It does not load `tokens.css` or inspect rendered screens. Neither the `package.json` check/build scripts nor `.github/workflows/release.yml` invokes it, so "WCAG-AA gated" describes a required verification practice, not an automatic repository-wide or release-CI guarantee. A passing invocation alone would not prove that every current screen or token matches those copied inputs.

The application-version rule is implemented: `src-tauri/Cargo.toml` owns the app version, `src-tauri/tauri.conf.json` has no version property, and the frontend update state obtains the running version through `getVersion()`. The disclosure rules remain accepted design policy; this source-only audit does not claim to verify past visual reviews or the external research.
