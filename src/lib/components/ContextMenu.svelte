<script lang="ts">
  import Icon from "./Icon.svelte";

  export interface ContextMenuItem {
    label: string;
    icon?: string;
    danger?: boolean;
    /** A disabled item (e.g. Move up on the first row) renders greyed and
     *  is skipped by keyboard navigation. */
    disabled?: boolean;
    /** A separator renders a hairline divider row instead of an item. */
    separator?: boolean;
    onselect: () => void;
  }

  /** How a trigger asks for its menu. Right-click supplies cursor coords;
   *  anchored requests (⋯ button, Enter on the card) supply the trigger to
   *  position against, with focusFirst true when opened by keyboard. */
  export type MenuRequest =
    | { kind: "cursor"; x: number; y: number; returnTo: HTMLElement | null }
    | { kind: "anchor"; anchor: HTMLElement | null; focusFirst: boolean; returnTo: HTMLElement | null };

  export interface ContextMenuState {
    open: boolean;
    items: ContextMenuItem[];
    label: string;
    x?: number;
    y?: number;
    anchor?: HTMLElement | null;
    /** Move focus into the menu when it opens (keyboard activation).
     *  Pointer opens never steal focus — focus stays where it is. */
    focusFirst?: boolean;
    returnTo?: HTMLElement | null;
  }

  let {
    ctx,
    onclose,
  }: {
    ctx: ContextMenuState | null;
    onclose: () => void;
  } = $props();

  let menuEl: HTMLDivElement | undefined = $state();
  let activeIndex = $state(0);
  let pos = $state({ x: 0, y: 0 });
  let itemEls: (HTMLButtonElement | undefined)[] = [];

  $effect(() => {
    if (!ctx?.open) return;
    activeIndex = 0;
    const raf = requestAnimationFrame(() => {
      if (!menuEl) return;
      const size = menuEl.getBoundingClientRect();
      let x: number;
      let y: number;
      if (ctx.anchor) {
        const r = ctx.anchor.getBoundingClientRect();
        x = r.right - size.width;
        y = r.bottom + 6;
      } else {
        x = ctx.x ?? 0;
        y = ctx.y ?? 0;
      }
      x = Math.max(8, Math.min(x, window.innerWidth - size.width - 8));
      y = Math.max(8, Math.min(y, window.innerHeight - size.height - 8));
      pos = { x, y };
      if (ctx.focusFirst) itemEls[firstFocusable()]?.focus();
    });
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    if (!ctx?.open) return;
    const onWindowKeydown = (e: KeyboardEvent) => {
      const focusInside = menuEl?.contains(document.activeElement) ?? false;
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          closeAndRestore();
          break;
        case "Tab":
          closeAndRestore();
          break;
        case "ArrowDown":
          if (focusInside) {
            e.preventDefault();
            moveFocus(1);
          }
          break;
        case "ArrowUp":
          if (focusInside) {
            e.preventDefault();
            moveFocus(-1);
          }
          break;
        case "Home":
          if (focusInside) {
            e.preventDefault();
            setActive(0);
          }
          break;
        case "End":
          if (focusInside) {
            e.preventDefault();
            setActive((ctx?.items.length ?? 1) - 1);
          }
          break;
      }
    };
    const onWindowPointerDown = (e: PointerEvent) => {
      if (menuEl?.contains(e.target as Node)) return;
      if ((e.target as HTMLElement | null)?.dataset?.ctxTrigger) return;
      onclose();
    };
    window.addEventListener("keydown", onWindowKeydown);
    window.addEventListener("pointerdown", onWindowPointerDown);
    return () => {
      window.removeEventListener("keydown", onWindowKeydown);
      window.removeEventListener("pointerdown", onWindowPointerDown);
    };
  });

  function moveFocus(dir: number) {
    const n = ctx?.items.length ?? 0;
    if (n === 0) return;
    let i = activeIndex;
    // Separators and disabled items are not focusable — step past them.
    for (let step = 0; step < n; step++) {
      i = (i + dir + n) % n;
      const item = ctx?.items[i];
      if (item && !item.separator && !item.disabled) {
        setActive(i);
        return;
      }
    }
  }

  /** The first focusable item index (skips separators and disabled items). */
  function firstFocusable(): number {
    for (let i = 0; i < (ctx?.items.length ?? 0); i++) {
      const item = ctx?.items[i];
      if (item && !item.separator && !item.disabled) return i;
    }
    return 0;
  }

  function setActive(i: number) {
    activeIndex = i;
    itemEls[i]?.focus();
  }

  function closeAndRestore() {
    if (menuEl?.contains(document.activeElement)) ctx?.returnTo?.focus();
    onclose();
  }

  function select(item: ContextMenuItem) {
    item.onselect();
    onclose();
  }
</script>

{#if ctx?.open}
  <div
    bind:this={menuEl}
    class="ctx-menu"
    role="menu"
    aria-label={ctx.label}
    style="left: {pos.x}px; top: {pos.y}px;"
  >
    {#each ctx.items as item, i (item.label)}
      {#if item.separator}
        <div class="ctx-sep" role="separator"></div>
      {:else}
        <button
          bind:this={itemEls[i]}
          type="button"
          class="ctx-item"
          class:active={activeIndex === i}
          class:danger={item.danger}
          role="menuitem"
          tabindex={activeIndex === i ? 0 : -1}
          disabled={item.disabled}
          onclick={() => select(item)}
          onpointerenter={() => {
            if (!item.disabled) activeIndex = i;
          }}
        >
          {#if item.icon}
            <span class="ctx-item__icon" aria-hidden="true"><Icon name={item.icon} size={14} /></span>
          {/if}
          <span>{item.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .ctx-menu {
    position: fixed;
    z-index: 100;
    display: flex;
    flex-direction: column;
    min-width: 168px;
    padding: 4px;
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-dialog);
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 7px var(--space-3);
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    font-family: var(--font-body);
    font-size: var(--text-sm);
    font-weight: 500;
    text-align: left;
    color: var(--text);
    cursor: pointer;
  }

  .ctx-item__icon {
    display: inline-flex;
    color: var(--text-muted);
  }

  .ctx-sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }

  .ctx-item:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .ctx-item:hover,
  .ctx-item.active {
    background: var(--bg-hover);
    color: var(--text);
  }

  .ctx-item.danger,
  .ctx-item.danger .ctx-item__icon {
    color: var(--danger-text);
  }

  .ctx-item.danger:hover,
  .ctx-item.danger.active {
    background: var(--danger-tint);
  }
</style>