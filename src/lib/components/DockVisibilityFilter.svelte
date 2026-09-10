<script lang="ts">
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";
  import ContextMenu, {
    type ContextMenuItem,
    type ContextMenuState,
  } from "./ContextMenu.svelte";
  import type { DockVisibility } from "$lib/dockVisibility";

  let {
    value = "all",
    onchange,
  }: {
    /** The page's current choice — page-local state, never persisted. */
    value?: DockVisibility;
    /** Receives the newly chosen value; choosing All is the reset. */
    onchange: (next: DockVisibility) => void;
  } = $props();

  const options: { value: DockVisibility; label: string }[] = [
    { value: "all", label: "All" },
    { value: "shown", label: "Shown in dock" },
    { value: "hidden", label: "Hidden from dock" },
  ];

  const activeLabel = $derived(
    options.find((o) => o.value === value)?.label ?? "All"
  );
  const isActive = $derived(value !== "all");

  let menu: ContextMenuState | null = $state(null);
  let trigger: HTMLButtonElement | undefined = $state();

  // Progressive disclosure (research 0004 rule 2): the exclusive choices
  // wait behind one clear trigger; the active choice stays visible after
  // dismissal and resets explicitly to All. The shared ContextMenu carries
  // anchored positioning, checked radios, keyboard nav, and focus return.
  function openMenu(viaKeyboard: boolean) {
    if (menu?.open) {
      menu = null;
      return;
    }
    const items: ContextMenuItem[] = options.map((o) => ({
      label: o.label,
      checked: o.value === value,
      onselect: () => onchange(o.value),
    }));
    menu = {
      open: true,
      label: "Dock visibility",
      anchor: trigger ?? null,
      focusFirst: viaKeyboard,
      returnTo: trigger ?? null,
      items,
    };
  }
</script>

<span class="dock-filter">
  <button
    type="button"
    bind:this={trigger}
    class="dock-filter__trigger"
    class:is-active={isActive}
    aria-haspopup="menu"
    aria-expanded={menu?.open ?? false}
    aria-label={isActive
      ? `Dock visibility: ${activeLabel}. Activate to change the filter.`
      : "Dock visibility. Activate to filter the list."}
    title={isActive
      ? `Dock visibility: ${activeLabel}`
      : "Dock visibility"}
    data-ctx-trigger
    onclick={(e) => openMenu(e.detail === 0)}
  >
    <span class="dock-filter__text">
      Dock visibility{isActive ? `: ${activeLabel}` : ""}
    </span>
    <span class="dock-filter__chevron" aria-hidden="true">
      <Icon name="chevron" size={13} />
    </span>
  </button>
  {#if isActive}
    <IconButton
      icon="x"
      label="Reset dock visibility to All"
      quiet
      onclick={() => onchange("all")}
    />
  {/if}
</span>

<ContextMenu ctx={menu} onclose={() => (menu = null)} />

<style>
  .dock-filter {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }

  .dock-filter__trigger {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 9px 12px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--bg-surface);
    color: var(--text-muted);
    font-family: var(--font-body);
    font-size: var(--text-sm);
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out);
  }

  .dock-filter__trigger:hover {
    border-color: var(--accent);
    color: var(--text);
  }

  .dock-filter__trigger:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  /* The active choice stays visible after dismissal: accent border and
     tint, the same value-reading language as the kind pills. */
  .dock-filter__trigger.is-active {
    border-color: var(--accent);
    background: var(--accent-tint);
    color: var(--accent);
  }

  .dock-filter__text {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dock-filter__chevron {
    display: inline-flex;
    flex-shrink: 0;
  }
</style>
