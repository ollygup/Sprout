<script lang="ts">
  import { page } from "$app/state";
  import "../lib/styles/tokens.css";
  import Icon from "$lib/components/Icon.svelte";
</script>

<svelte:head>
  <title>Something went wrong — Sprout</title>
</svelte:head>

<main class="error">
  <div class="error__card">
    <span class="error__status">{page.status}</span>
    <h1 class="error__title">The page could not be loaded</h1>
    <p class="error__detail">
      {page.error?.message ?? "An unexpected error occurred."}
    </p>
    <a class="error__home" href="/">
      <Icon name="seedling" size={15} /> Back to Products
    </a>
  </div>
</main>

<style>
  .error {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    background: var(--bg-page);
    color: var(--text);
    padding: var(--space-5);
  }

  .error__card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    max-width: 440px;
    text-align: center;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-card);
    padding: var(--space-6) var(--space-5);
  }

  .error__status {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: var(--radius-lg);
    background: var(--accent-tint);
    color: var(--accent);
    border: 1px solid var(--accent-tint-border);
  }

  .error__title {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-xl);
    font-weight: 600;
  }

  .error__detail {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  .error__home {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
    padding: 8px 16px;
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--on-accent);
    font-size: var(--text-base);
    font-weight: 600;
    text-decoration: none;
  }

  .error__home:hover {
    background: var(--accent-hover);
  }
</style>