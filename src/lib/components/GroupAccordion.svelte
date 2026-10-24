<script lang="ts">
  import type { Snippet } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { animation } from "$lib/animation.svelte";
  import Badge from "./Badge.svelte";
  import Disclosure from "./Disclosure.svelte";

  /** The shared collapsible section for the Groups feature (tickets 89–93):
   *  one group rendered as a default-expanded accordion — the labeled
   *  Disclosure chevron, the muted count Badge, and the body that exists
   *  only while open. Every surface that renders Groups (the Quick Launch,
   *  Quick Actions, and Clips pages plus the Quick Launch window's tabs)
   *  shows its groups through this component, so the section look, collapse
   *  wiring, and accessibility are defined once.
   *
   *  The rows stay the caller's: pass them as children. Main pages pass an
   *  `actions` snippet for their ⋯ group menu; `flush` drops the body indent
   *  for the Quick Launch window's narrow strip. A `color` paints one
   *  continuous leading bar and a calm body wash, and publishes the matching
   *  row-hover surface for the caller's rows (`--group-row-hover` /
   *  `--group-row-hover-text`); chrome stays token-only and the group's
   *  name/count/position remain the real signals (tickets 225, 240). */
  let {
    open,
    controls,
    name,
    count,
    color = null,
    onToggle,
    actions,
    flush = false,
    children,
  }: {
    open: boolean;
    controls: string;
    name: string;
    count: number;
    color?: string | null;
    onToggle: () => void;
    actions?: Snippet;
    flush?: boolean;
    children: Snippet;
  } = $props();
</script>

<section
  class="group"
  class:group--flush={flush}
  class:group--colored={color !== null}
  style:--group-color={color ?? undefined}
>
  <div class="group__head">
    <span class="group__title">
      <Disclosure {open} {controls} label={name} onclick={onToggle} />
    </span>
    <Badge tone="muted">{count}</Badge>
    {#if actions}
      {@render actions()}
    {/if}
  </div>
  {#if open}
    <!-- WHY fly and not height: panels settle with opacity/transform only so
         expansion never shifts layout, and the off switch renders the end
         state instantly (ADR-0034; ADR-0028 off path). The 8px travel reads
         the 200ms settle at the system's expressive level. -->
    <div
      id={controls}
      class="group__rows"
      transition:fly={animation.mode === "on" ? { y: -8, duration: 200, easing: cubicOut } : { duration: 0 }}
    >
      {@render children()}
    </div>
  {/if}
</section>

<style>
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-5);
    /* One source for the caret column: the head's icon box and the body
       indent both read it, so rows land exactly at the title-text start
       (Notion toggle anatomy — research 0006, pattern 9). */
    --caret-column: 14px;
  }

  /* The top margin separates sections from each other — a section already
     first in its parent sits under the header's own spacing, so it takes
     none, keeping header-to-content rhythm identical on every page. */
  .group:first-child {
    margin-top: 0;
  }

  /* The Quick Launch window's strip: its scroll container already spaces
     sections, and the rows sit flush with everything else. */
  .group--flush {
    margin-top: 0;
  }

  .group__head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .group__title {
    flex: 1;
    min-width: 0;
    display: flex;
    overflow: hidden;
  }

  /* Long names truncate instead of pushing the count badge out of the row. */
  .group__title :global(.disclosure) {
    min-width: 0;
  }

  .group__title :global(.disclosure__label) {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group--flush .group__head {
    padding-right: var(--space-2);
  }

  /* The caret box gets a fixed column width, so the label always starts at
     caret-column + gap no matter the glyph's intrinsic size. */
  .group__head :global(.disclosure__chevron) {
    display: inline-flex;
    flex: none;
    width: var(--caret-column);
    justify-content: center;
  }

  /* Body content aligns with the title text's start — caret column plus the
     labeled-mode gap. */
  .group__rows {
    padding-left: calc(var(--caret-column) + var(--space-1));
  }

  .group--flush .group__rows {
    padding-left: 0;
  }

  /* A colored group is one object, not a framed box (research 0028). A single
     3 px bar runs the section's full height at the leading edge and identifies
     the group; the hue itself sits on the items, not on the empty space between
     them (research 0028, addendum 2026-10-24). The section publishes the row tint, the hover
     surface and the hover text its caller's rows use, so the row chrome stays
     owned by the page and the accordion never learns row class names. The inset
     keeps the bar clear of the title and rows, so a colored group's content
     starts exactly where a neutral group's does. Chrome (text, icons, badges,
     focus ring) stays on tokens; only these decorative surfaces read the group
     color. */
  .group--colored {
    position: relative;
    padding-left: var(--space-3);
    --group-row-bg: color-mix(in oklab, var(--group-color) 8%, var(--bg-page));
    --group-row-hover: color-mix(in oklab, var(--group-color) 14%, var(--bg-hover));
    --group-row-hover-text: var(--text);
  }

  /* The same alpha over #191919 shifts chroma far less than over #ffffff, so
     dark mode needs the higher percentage for the tint to read (0028). */
  :global(:root[data-theme="dark"]) .group--colored {
    --group-row-bg: color-mix(in oklab, var(--group-color) 14%, var(--bg-page));
  }

  .group--colored::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    border-radius: var(--radius-pill);
    background: var(--group-color);
  }

  /* The body keeps the page surface: the region is spacing, not a panel, so
     only the items carry the hue. */
  .group--colored .group__rows {
    padding: var(--space-1) var(--space-2);
    padding-left: calc(var(--caret-column) + var(--space-1));
    border-radius: var(--radius-sm);
    background: transparent;
  }

  /* Dock strip (ticket 241): the flush variant drops the main app's padded
     card, so a coloured section takes one small inset for the bar and lets the
     rows keep the strip's full width. The strip keeps its rhythm; only the
     leading edge moves, exactly as the main app's does. */
  .group--colored.group--flush {
    padding-left: var(--space-2);
  }

  .group--colored.group--flush .group__rows {
    padding: 0;
  }

  /* A neutral group publishes no tint either, so a neutral child inside a
     colored parent inherits neither the hue nor its hover surfaces (spec 221
     A.6). */
  .group:not(.group--colored) {
    --group-color: initial;
    --group-row-bg: initial;
    --group-row-hover: initial;
    --group-row-hover-text: initial;
  }

  /* Without color-mix() the bar still identifies the group: the row tint drops
     to transparent, hover stays neutral, and secondary text keeps its token. */
  @supports not (background: color-mix(in oklab, red, blue)) {
    .group--colored {
      --group-row-bg: transparent;
      --group-row-hover: var(--bg-hover);
      --group-row-hover-text: var(--text-muted);
    }
  }
</style>
