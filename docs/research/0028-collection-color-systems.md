# 0028 — Colouring a collection: accent geometry, section tint, and hover inside a coloured group

Date: 2026-10-24. Status: research note feeding the group-colour ticket
(spec 221 §A.4 / ticket 225). First-party documentation only — vendor help
centres, API references, design-system docs. Values visible in a product but
absent from its documentation are labelled *observed, not documented*; no
number here is estimated from a screenshot.

## Question

Sprout draws a user-coloured group (a collection of packets) as a 3 px accent
bar on the header, a body wash at `GROUP_COLOR_TINT = 0.1`, and a 1 px inset
outline in the group colour. The user's report: the frame looks bad, and
hovering a row inside a coloured group shows no trace of the group colour.
Seven things need answering before the CSS changes:

1. Where do shipped products put the accent — bar, edge, badge, or nothing?
2. How strong is a coloured *section* background, and is it documented?
3. What happens to row hover *inside* a coloured section?
4. Do coloured sections carry borders or outlines?
5. How do the answers change in dark mode?
6. What contrast obligations actually bind, per WCAG?
7. What should Sprout do, concretely?

## Sources

- Notion Help, *Style & customize your page* — https://www.notion.com/help/style-and-customize-your-page
  — whole-block background via `⋮⋮` → `Color` or `/red`; ten-name palette; `Default` = gray outline.
- Notion API, *Data source properties* — https://developers.notion.com/reference/property-object
  — select `color` enum: `default, gray, brown, orange, yellow, green, blue, purple, pink, red`; names only, no hex, alpha, or per-theme value.
- Notion Help, *Enhanced Markdown* — https://www.notion.com/help/enhanced-markdown
  — the same names cover inline text colour; one vocabulary spans both.
- Discord Developers, *Role object* — https://discord.com/developers/docs/topics/permissions#role-object
  — `color` deprecated for `colors.primary` / `secondary` / `tertiary`; gradients and holographic presets are role-colour features. Fields, not rendering.
- Discord Support, *Role Management 101* — https://support.discord.com/hc/en-us/articles/214836687-Role-Management-101
  — role colour colours the member name and role chip; no background alpha.
- Linear Docs, *Project overview* — https://linear.app/docs/project-overview
  — "Customize color and icon from our presets": preset palette, no picker.
- Linear Docs, *Initiatives* — https://linear.app/docs/initiatives
  — colour coding is an initiative property used for scanning.
- Linear Docs, *Project labels* — https://linear.app/docs/project-labels
  — documents grouping and filtering, not colour.
- Linear Changelog, *Custom themes* (2020) — https://linear.app/changelog/2020-09-01-custom-themes
  — a theme generates the shades used for borders and elevated boxes, so one hue expands into a ramp rather than being reused flat.
- GitHub Docs, *Managing labels* — https://docs.github.com/en/issues/using-labels-and-milestones-to-track-work/managing-labels
  — a label has a name, description, and colour; random on creation, editable.
- Primer, *Color usage* — https://primer.style/product/getting-started/foundations/color-usage/
  — muted background roles pair with muted border roles, contrast is computed against `bgColor-muted` not the page, and high-contrast aims at 7:1.
- Apple HIG, *Color* — https://developer.apple.com/design/human-interface-guidelines/color
  — a custom colour needs light, dark, and increased-contrast variants; colour must not be the only carrier of information.
- Material Design 3, *Elevation* — https://developer.android.com/develop/ui/compose/designsystems/material3
  — surface elevation is expressed mainly through tonal colour overlays, and the overlay colour comes from the primary slot.
- Material Web, `_md-sys-state.scss` — https://github.com/material-components/material-web/blob/main/tokens/versions/v0_192/_md-sys-state.scss
  — state-layer opacities: hover `0.08`, focus `0.12`, pressed `0.12`, dragged `0.16`.
- Microsoft Fluent 2, *Color* — https://fluent2.microsoft.design/color
  — documents the rest → hover → selected darkening progression and per-theme ramps, but publishes no state alpha numbers.
- Microsoft Learn, *Layering, elevation, and shadows* — https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering
  — cards sit at elevation 8 with a 1 px stroke; the stroke does the separating.
- W3C, *Understanding SC 1.4.11* — https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
  — a control identified only by its background needs 3:1 against adjacent colours; hover/focus need not reach 3:1 but must not destroy state contrast.
- W3C, *Understanding SC 1.4.3* — https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
  — 4.5:1 body text, 3:1 large text.
- MDN, *`color-mix()`* — https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/color-mix
  — defaults to `oklab`, Baseline "widely available", percentages normalise, mixing with `transparent` adds alpha, and it cannot raise alpha to opaque.
- MDN, *Relative color syntax* — https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_colors/Relative_colors
  — `rgb(from <color> r g b / α)` derives a colour from an existing one; test is `@supports (color: hsl(from white h s l))`.

## What each product does

| Product | Where colour appears | Hover inside a coloured area | Source |
| --- | --- | --- | --- |
| Notion, blocks | Whole-block background, 10-name palette; `Default` draws a light gray outline | Colour is a block property, not a container; no documented hover tint inside a coloured block | Notion Help, *Style & customize* |
| Notion, select options | Same 10 names on the option chip | Not documented | Notion API, data-source properties |
| Discord, roles | Name text and role chip; gradient and holographic variants | Mention chips carry a low-alpha wash of the role colour — **observed, not documented** | Discord Devs, *Role object*; Support, *Role Management 101* |
| Linear, projects/initiatives | Preset colour on the project icon and initiative marker | Not documented; no section tint described | Linear Docs, *Project overview*; *Initiatives* |
| Linear, labels | Label chip | Not documented | Linear Docs, *Project labels* |
| GitHub, labels | Label chip colour, random on create, editable | Not documented | GitHub Docs, *Managing labels* |
| Apple platforms | Guidance, not a section: per-appearance variants required | n/a | Apple HIG, *Color* |
| Material 3 | Tonal overlays from the primary slot; state layers at fixed opacities | Hover = 8 % state layer over the *current* surface, so a tinted surface stays tinted | M3 elevation docs; `_md-sys-state.scss` |
| Fluent 2 / Windows | Rest → hover → selected ramp; 1 px stroke separates card from background | Documented as a progression, alphas unpublished | Fluent 2 *Color*; Learn, *Layering* |

Two cells are honestly empty: no first-party source documents hover inside a
coloured *section* in Notion, Linear, or GitHub. Only Material 3 documents the
mechanism — a state layer over the tinted surface; only Discord visibly ships one.

## Synthesis

**1. Accent geometry.** No first-party material describes a leading accent bar
on a section. Notion colours the *whole block*; Linear and GitHub colour a
*chip* or an *icon*; Discord colours *text*. The bar is a Sprout convention, so
it is decorative — but it cannot be defended as "how Notion does it". Its job
is to make the group's extent legible, and a header-only bar marks the header
rather than the collection, which is why the body reads as unowned.

**2. Section background alpha.** No vendor publishes one. Notion's palette is
named colours with no hex or alpha; Linear's is "presets". Material 3 is the
only system publishing composited-surface numbers, and those are state layers
(0.08 hover, 0.12 focus/pressed, 0.16 dragged), not section identity. Any
"Notion uses 6 %" claim would be invention. What *is* documented is the shape
of the rule: a tint is a low-alpha composite over a real surface token
(`bgColor-muted` in Primer's case), not a hardcoded white.

**3. Hover inside a coloured section.** Only Material 3 documents it: the state
layer composites *over* the current surface, so hover inherits the section tint
instead of replacing it. This is the most useful finding for Sprout's
complaint. Sprout's row hover sets `background: var(--bg-hover)` — an opaque
token — erasing the group wash on the one row the user is pointing at. The fix
is not a second tint; it is compositing the group hue *into* the hover token.

**4. Borders and outlines.** The documented pattern is a border paired with a
muted fill (Primer), or a 1 px stroke doing the separation a shadow would
otherwise do (Windows layering). Neither is a coloured outline around a
coloured fill; Notion's `Default` block draws a *neutral* light-gray outline. A
1 px outline in the fill's own hue double-encodes one signal at low contrast,
and that is the part of Sprout's treatment that reads as a box around a box.

**5. Dark mode.** Apple requires a per-appearance variant of any custom colour;
Material derives surfaces per theme rather than reusing one alpha. The reason
is compositing: the same alpha over `#191919` shifts chroma far less than over
`#ffffff`, because a dark base can only move so far toward a hue. Dark mode
needs a *higher* alpha, and the bar must be re-checked against the dark page.

**6. Contrast.** WCAG 1.4.11 binds the bar only if the bar is the sole
identifier of the group — it is not, because the group has a name in text.
Hover and focus are explicitly allowed below 3:1 provided they do not wreck the
contrast of the state they overlay. What does bind is 1.4.3: row text stays
≥4.5:1 against the *composited* background, the tinted surface, not
`--bg-page` — Primer's "measure against `bgColor-muted`" is the same rule. A
6–14 % tint moves luminance by a few percent, so text that passed before still
passes, checked twice: light and dark.

## Recommendation for Sprout

Geometry — one coloured object, not a header with a frame:

```css
.group {
  /* bar spans the whole collection, header and body alike */
  border-inline-start: 3px solid var(--group-color);
  border-radius: 6px;
  overflow: clip;              /* keeps the tint inside the radius */
}

.group__body {
  background: color-mix(in oklab, var(--group-color) 8%, var(--bg-page));
}

@media (prefers-color-scheme: dark) {
  .group__body {
    background: color-mix(in oklab, var(--group-color) 14%, var(--bg-page));
  }
}
```

Row hover carries the hue instead of erasing it:

```css
.packet-row:hover {
  background: color-mix(in oklab, var(--group-color) 14%, var(--bg-hover));
}
```

- `in oklab` is the documented default and keeps the mix perceptually even across all ten hues.
- `--bg-hover` is opaque, so the result is opaque and cannot double-composite
  with the body wash underneath — no stacking, no compounding on nested rows.
- 14 % of hue into `#f1f1ef` is a visible step and still reads as the same
  family as the 8 % body. Both numbers are Sprout choices; no vendor publishes
  an equivalent pair.
- Fallback: `@supports not (color: color-mix(in oklab, red, blue))` sets `background: var(--bg-hover)`; hover degrades to today's neutral, the group tint still applies via the bar.

Borders: **drop the 1 px inset outline.** Replace it with nothing on the body
and, if the collection needs separation from the page, a single
`1px solid var(--border)` on the group container. Notion, Linear, GitHub and
Windows separate with neutral strokes or none; none draws a tinted frame around
a tinted fill.

Dark mode: 14 % body, bar stays `--group-color` — do not darken the bar itself,
or the group loses identity in the theme where it is hardest to see. Re-check
the bar at 3:1 against `#191919` for the palest curated hues; if one fails, that
hue needs a dark-mode variant in `groupColor.ts`, not a global alpha change.

Checks before landing: row text at 4.5:1 against the 8 % light and 14 % dark
composited backgrounds for all ten hues; bar at 3:1 against `--bg-page` in both
themes; hover at least one visible step from rest, measured as ΔL, not ratio.

## Observed vs documented

- **Documented:** M3 state-layer opacities and tonal-overlay mechanism; Notion
  palette names and whole-block colouring; Discord role colour fields; Linear
  preset colouring; GitHub label colour editability; Primer's muted-fill +
  muted-border pairing and contrast-against-fill rule; Apple's per-appearance
  requirement; WCAG 1.4.3 and 1.4.11; `color-mix()` semantics.
- **Observed, not documented:** Discord's coloured mention-chip wash; the
  visual strength of Notion's block colours; the alpha of any vendor's section
  tint; the alpha of any vendor's hover state outside Material.
- **Not found in primary sources:** any product documenting hover *inside* a
  coloured section, and any vendor-published section-tint alpha. Both gaps are
  stated rather than filled.

## Addendum — 2026-10-24 (measured contrast on the derived surfaces)

The recommendation above ships with one measured correction. Mixing 14 % of any
hue into `--bg-hover` (light `#f1f1ef`, dark `#2e2e2e`) lowers that surface's
luminance enough that `--text-muted` falls below AA: 3.79–3.88:1 light and
4.23–4.31:1 dark against the 4.5:1 required. The cause is the pair's missing
headroom — `--text-muted` already measures 4.52:1 light / 4.65:1 dark on plain
`--bg-hover`. The wash is safe: 8 % into `--bg-page` keeps muted text at
4.59–4.66:1 light, and 14 % into the dark page keeps it at 4.89–4.99:1. No tint
percentage that still reads as a hover preserves the pair, and choosing a
different hue does not recover it.

Resolution shipped: secondary text inside a hovered coloured row is promoted to
`--text` (9.10–9.32:1 light, 8.21–8.45:1 dark on the same composited surfaces).
The surface formula is otherwise unchanged. `tools/contrast-check.mjs` now mixes
in Oklab — the space `color-mix()` interpolates in — and gates row text, muted
text on the wash, row text on the hover surface, and a ≥3 Oklab-lightness hover
step for all ten hues in both themes.

## Addendum — 2026-10-24 (the hue belongs on the items, not the region)

The geometry recommended above tints the group **body** (`.group__body`), so the
8 %/14 % mix fills the region — the empty space above, below and between the
cards included. Shipped and compared side by side against a per-card variant,
the body tint lost on two counts:

- **It reads as a panel.** A tinted rectangle around the rows is a surface the
  user never asked for: the group looks like a different *container*, and the
  cards look like they float on it. The report was blunt — the frame "does not
  follow any good UI/UX rule".
- **It scales with emptiness.** The more padding and inter-card gap a collection
  has, the more of the colour is empty space. In the 340–420 px dock strip the
  wash became most of the window; on the main pages it drew a large block behind
  a short list.

The per-card variant keeps the same numbers and the same identifier, and moves
only *where* the colour lands:

```css
/* published once by the group; the item consumes it */
.group--colored {
  --group-row-bg: color-mix(in oklab, var(--group-color) 8%, var(--bg-page));
}
:root[data-theme="dark"] .group--colored {
  --group-row-bg: color-mix(in oklab, var(--group-color) 14%, var(--bg-page));
}

.group__body { background: transparent; }        /* spacing, not a panel */
.item-card  { background: var(--group-row-bg, transparent); }
```

Everything else from the recommendation stands: `in oklab`, 14 % for hover over
`--bg-hover`, no tinted frame, bar stays `--group-color` in both themes, and the
same contrast pairs (the colour behind the text is identical, so the measured
ratios above carry over unchanged). The trade-off is that the region no longer
signals membership when a group holds few items; the full-height bar carries
that job alone, which is why the bar must not shrink to a header stub.

Evidence: spec 221's amendment of the same date; `tools/repro-group-color-hover.mjs`
measures the card tint and the transparent body in both themes, main app and dock.


