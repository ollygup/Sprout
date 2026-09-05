# One design system and disclosure rules govern every screen

`tokens.css` (Planting Ledger: Notion-derived palette, dual `data-theme`, type/space/shape/motion, WCAG-AA gated by `contrast-check`) is the sole source of visual values; shared `components/` (page header + features-menu slot, dialogs, disclosures, packet cards, tabs) is the sole source of patterns. No screen introduces ad-hoc colors, type sizes, radii, or one-off components — a deviation needs a ticket note and review before shipping. Placement follows the standing research instead of taste: progressive disclosure and clips (content-gated tabs, two levels max, tab hygiene — research 0004), page-chrome consistency (research 0005), Notion's factual method (visibility-on-surface vs configuration-elsewhere, minimal-until-content defaults, explicit-setup gating, view-scoped switches — research 0006, patterns 2–3, 8, 11–12), export scope as moment-of-use dialogs (research 0007), and feature menus over toolbar checkboxes (placement-follows-persistence, switch-reads-value, scent, owned-once, empty-invisible — research 0008). The version string follows the same single-source doctrine as geometry: `Cargo.toml` owns it, `tauri.conf.json` omits it, the UI reads it via `getVersion()`.

## Consequences

- New screens compose; they don't invent. Review starts with "which token, which component, which research rule" — anything else is the exception that proves the system.
- Research notes stay the evidence locker; this ADR is the lock: the rules are decisions now, not suggestions.
