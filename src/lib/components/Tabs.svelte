<script lang="ts">
  import type { Snippet } from "svelte";

  /** One tab in the strip (ticket 52): an id the panel snippet keys on and
   *  the button's visible label. */
  interface TabDef {
    id: string;
    label: string;
  }

  /** Shared accessible tab strip (ticket 52, added to the component
   *  foundation per the AGENTS.md design rule): a WAI-ARIA tablist with
   *  roving tabindex — the selected tab keeps `tabindex=0`, the rest are
   *  skipped, and ←/→/Home/End move the selection and focus. All panels
   *  stay mounted (hidden, not removed) so their `aria-labelledby` targets
   *  always exist and tab state survives panel toggles. */
  let {
    tabs,
    selected,
    onselect,
    ariaLabel,
    panel,
  }: {
    tabs: TabDef[];
    selected: string;
    onselect: (id: string) => void;
    ariaLabel: string;
    /** Receives the tab id of the panel being rendered. */
    panel: Snippet<[string]>;
  } = $props();

  function keydown(event: KeyboardEvent) {
    const index = tabs.findIndex((tab) => tab.id === selected);
    let next: number | null = null;
    if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
    else if (event.key === "ArrowLeft") next = (index - 1 + tabs.length) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    if (next === null) return;
    event.preventDefault();
    const id = tabs[next].id;
    onselect(id);
    requestAnimationFrame(() => document.getElementById(`tab-${id}`)?.focus());
  }
</script>

<div class="tabs">
  <div role="tablist" aria-label={ariaLabel} class="tabs__list" tabindex="-1" onkeydown={keydown}>
    {#each tabs as tab (tab.id)}
      <button
        id={`tab-${tab.id}`}
        type="button"
        role="tab"
        aria-selected={tab.id === selected}
        aria-controls={`panel-${tab.id}`}
        tabindex={tab.id === selected ? 0 : -1}
        class="tabs__tab"
        class:active={tab.id === selected}
        onclick={() => onselect(tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </div>
  {#each tabs as tab (tab.id)}
    <div
      id={`panel-${tab.id}`}
      role="tabpanel"
      aria-labelledby={`tab-${tab.id}`}
      class="tabs__panel"
      hidden={tab.id !== selected}
    >
      {@render panel(tab.id)}
    </div>
  {/each}
</div>

<style>
  .tabs__list {
    display: flex;
    gap: var(--space-1);
    padding: 0 var(--space-3);
    border-bottom: 1px solid var(--border);
  }

  .tabs__tab {
    appearance: none;
    margin: 0 0 -1px;
    padding: var(--space-3) var(--space-2);
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    font-family: var(--font-body);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-muted);
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .tabs__tab:hover {
    color: var(--text);
  }

  .tabs__tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .tabs__tab:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .tabs__panel:focus {
    outline: none;
  }

  .tabs__panel[hidden] {
    display: none;
  }
</style>