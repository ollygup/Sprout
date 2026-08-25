<script lang="ts">
  import type { Snippet } from "svelte";
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
   *  for the Quick Launch window's narrow strip. */
  let {
    open,
    controls,
    name,
    count,
    onToggle,
    actions,
    flush = false,
    children,
  }: {
    open: boolean;
    controls: string;
    name: string;
    count: number;
    onToggle: () => void;
    actions?: Snippet;
    flush?: boolean;
    children: Snippet;
  } = $props();
</script>

<section class="group" class:group--flush={flush}>
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
    <div id={controls} class="group__rows">
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

  .group__rows {
    padding-left: var(--space-5);
  }

  .group--flush .group__rows {
    padding-left: 0;
  }
</style>
