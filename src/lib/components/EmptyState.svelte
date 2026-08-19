<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    icon = "seedling",
    title,
    children,
  }: {
    icon?: string;
    title: string;
    children: Snippet;
  } = $props();
</script>

<div class="empty">
  <span class="empty__icon" aria-hidden="true"><Icon name={icon} size={26} /></span>
  <p class="empty__title">{title}</p>
  <div class="empty__body">
    {@render children()}
  </div>
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-7) var(--space-5);
    text-align: center;
  }

  .empty__icon {
    display: inline-flex;
    color: var(--accent);
    margin-bottom: var(--space-2);
  }

  .empty__title {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--text);
  }

  .empty__body {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .empty :global(p) {
    margin: 0;
  }
</style>
