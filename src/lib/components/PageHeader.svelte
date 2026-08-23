<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    titleId,
    actions,
    subtitle,
    toolbar,
  }: {
    title: string;
    /** The h1's id — pages pass their own so section[aria-labelledby] keeps
     *  pointing at it. */
    titleId: string;
    /** Header-row buttons, right-aligned. One primary action per header
     *  (research 0005); auxiliary actions render secondary beside it. */
    actions?: Snippet;
    /** The muted count/guidance line under the title. */
    subtitle?: Snippet;
    /** Full-width row below the header — the list filter's home. Search
     *  lives here, never inline in the actions row (research 0005). */
    toolbar?: Snippet;
  } = $props();
</script>

<header class="page-header">
  <div class="page-header__row">
    <h1 id={titleId} class="page-header__title">{title}</h1>
    {#if actions}
      <div class="page-header__actions">{@render actions()}</div>
    {/if}
  </div>
  {#if subtitle}
    <p class="page-header__sub">{@render subtitle()}</p>
  {/if}
  {#if toolbar}
    <div class="page-header__toolbar">{@render toolbar()}</div>
  {/if}
</header>

<style>
  .page-header {
    margin-bottom: var(--space-5);
  }

  /* align-items:center keeps every control its natural height — a tall
   * sibling (the search input) must never stretch the buttons next to it. */
  .page-header__row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
  }

  .page-header__actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .page-header__title {
    font-family: var(--font-display);
    font-size: var(--text-2xl);
    line-height: 1.15;
    color: var(--text);
    text-wrap: balance;
  }

  .page-header__sub {
    margin: var(--space-2) 0 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .page-header__toolbar {
    display: flex;
    margin-top: var(--space-5);
  }
</style>
