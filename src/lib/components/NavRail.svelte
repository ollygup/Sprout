<script lang="ts">
  import { page } from "$app/state";
  import SproutMark from "./SproutMark.svelte";

  const items = [
    { id: "products", label: "Products", href: "/" },
    { id: "presets", label: "Presets", href: "/presets" },
    { id: "plan", label: "Plan", href: "/plan" },
    { id: "launch", label: "Quick Launch", href: "/launch" },
    { id: "history", label: "History", href: "/history" },
    { id: "logs", label: "Logs", href: "/logs" },
    { id: "settings", label: "Settings", href: "/settings" },
  ];

  const current = $derived(page.url.pathname);
</script>

<nav class="rail" aria-label="Sprout sections">
  <div class="rail__brand">
    <span class="rail__logo" aria-hidden="true"><SproutMark size={18} /></span>
    <span class="rail__wordmark">Sprout</span>
  </div>

  <ul class="rail__list">
    {#each items as item}
      <li>
        <a
          href={item.href}
          class="rail__item"
          class:active={current === item.href}
          aria-current={current === item.href ? "page" : undefined}
        >
          <span class="rail__label">{item.label}</span>
        </a>
      </li>
    {/each}
  </ul>

  <p class="rail__foot">v0.1.0</p>
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    width: 200px;
    flex-shrink: 0;
    background: var(--bg-page);
    border-right: 1px solid var(--border);
    padding: var(--space-5) var(--space-3) var(--space-4);
  }

  .rail__brand {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2) var(--space-5);
  }

  .rail__logo {
    display: inline-flex;
  }

  .rail__wordmark {
    font-family: var(--font-display);
    font-size: 1.125rem;
    font-weight: 600;
    letter-spacing: var(--tracking-display);
  }

  .rail__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rail__item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 7px var(--space-3);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    text-decoration: none;
    color: var(--text-muted);
    transition: background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .rail__item:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .rail__item.active {
    background: var(--accent-tint);
    color: var(--accent);
  }

  .rail__foot {
    margin: auto 0 0;
    padding: var(--space-3) var(--space-2) 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }
</style>