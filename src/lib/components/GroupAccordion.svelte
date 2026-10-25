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
   *  name/count/position remain the real signals (tickets 225, 240).
   *
   *  Nested groups reuse this component rather than growing a second
   *  accordion (ticket 230): `nested` indents a child under its parent, and
   *  every child collapses on its own — the parent's chevron still collapses
   *  the whole subtree (ticket 231, amended). `groupId` / `parentId` publish
   *  the drag/drop identity (tickets 232–233). */
  let {
    open,
    controls,
    name,
    count,
    color = null,
    onToggle,
    actions,
    grip,
    flush = false,
    nested = false,
    groupId,
    parentId = null,
    children,
  }: {
    open: boolean;
    controls: string;
    name: string;
    count: number;
    color?: string | null;
    onToggle: () => void;
    actions?: Snippet;
    grip?: Snippet;
    flush?: boolean;
    nested?: boolean;
    groupId?: number;
    parentId?: number | null;
    children: Snippet;
  } = $props();

  /** Only the main app passes a grip; the dock's strip never drags. */
  const draggable = $derived(grip !== undefined);
</script>

<section
  class="group"
  class:group--flush={flush}
  class:group--colored={color !== null}
  class:group--nested={nested}
  class:group--grip={draggable}
  class:group-tint={color !== null}
  data-group-id={groupId}
  data-group-parent={parentId ?? undefined}
  data-drop-group-section={groupId}
  style:--group-color={color ?? undefined}
>
  <div
    class="group__head"
    data-drop-group-header={groupId}
    data-group-parent={parentId ?? undefined}
  >
    {#if grip}
      {@render grip()}
    {/if}
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
      transition:fly={animation.mode === "on"
        ? { y: -8, duration: 200, easing: cubicOut }
        : { duration: 0 }}
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
    /* The leading gutter is layout, never colour: every group reserves the
       same strip for its bar and grip, so a coloured group's chevron, title
       and cards sit exactly where a neutral group's do, and a child under a
       coloured parent is indented exactly like one under a neutral parent.
       Colour only decides whether the bar is painted in that strip. */
    --group-inset: var(--space-3);
    padding-left: var(--group-inset);
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
     sections, and the rows sit flush with everything else. The strip's gutter
     is narrower because the strip is narrow, but it is still reserved for
     every group, coloured or not. */
  .group--flush {
    margin-top: 0;
    --group-inset: var(--space-2);
  }

  /* A child group sits one step inside its parent's body, so the two levels
     read as a tree without a second component. */
  .group--nested {
    margin-top: var(--space-3);
    margin-left: var(--space-2);
    --caret-column: 12px;
  }

  .group__head {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  /* The grip is positioned from the head's box, so pull the head out to the
     section's own leading edge and hand the inset back as padding: the 24 px
     strip then sits in the gutter left of the colour bar instead of on top of
     it, and a nested group's strip lands in the parent's indent with room of
     its own (ticket 232). The vertical padding grows the header's hit area
     without moving its text or the group's rhythm. */
  .group--grip .group__head {
    margin-left: calc(-1 * var(--group-inset, 0px));
    padding-left: var(--group-inset, 0px);
    padding-top: var(--space-1);
    padding-bottom: var(--space-1);
    margin-top: calc(-1 * var(--space-1));
    margin-bottom: calc(-1 * var(--space-1));
  }

  /* While a group is dragged, every top-level header advertises itself as a
     nesting target; the one under the pointer turns solid. A header drop draws
     no insertion line, so without this the gesture had no visible answer
     (ticket 233). */
  :global(body[data-drag-kind="group"])
    .group:not(.group--nested):not(.group--flush):not([data-drop-source])
    .group__head {
    outline: 1px dashed var(--border-strong);
    outline-offset: 2px;
    border-radius: var(--radius-sm);
  }

  /* The controller writes the attribute at drag time, so the compiler cannot
     see it; `:global` keeps the selector honest instead of dropping it. */
  :global(.group[data-drop-nest]) {
    border-radius: var(--radius-sm);
    background: var(--bg-hover);
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  :global(.group[data-drop-nest]) .group__head {
    outline: none;
  }

  /* The drag grip lives in the header's leading gutter (ticket 232). Its strip
     is a descendant of the header, so hovering the strip keeps this `:hover`
     alive: the header stays lit while the pointer travels to the handle, and
     the handle stays visible. Keyboard focus still reaches it. */
  .group__head:hover,
  .group__head:focus-within {
    --grip-opacity: 1;
  }

  /* The count stays visible at the strip's narrowest width — the title
     truncates, never the badge. */
  .group__head :global(.badge) {
    flex: none;
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

  /* The dock's nested strip only appears at ≥ 420 px content width, so the
     child can take a real indent step: its beam sits clear of the parent's
     instead of doubling it, and the sub-header keeps a chevron of its own
     (ticket 231, amended). */
  .group--nested.group--flush {
    margin-left: var(--space-3);
  }

  /* A colored group is one object, not a framed box (research 0028). A single
     3 px bar runs the section's full height at the leading edge and identifies
     the group; the hue itself sits on the items, not on the empty space between
     them (research 0028, addendum 2026-10-24). The section publishes the row tint, the hover
     surface and the hover text its caller's rows use, so the row chrome stays
     owned by the page and the accordion never learns row class names. The bar
     is painted inside the gutter every group already reserves (see .group), so
     colour changes nothing about where the chevron, title or cards land. Chrome
     (text, icons, badges, focus ring) stays on tokens; only these decorative
     surfaces read the group color. */
  .group--colored {
    position: relative;
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
     only the items carry the hue. No colour-only padding here — a coloured
     group's cards must sit on the same baseline and at the same left edge as a
     neutral group's. */

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
