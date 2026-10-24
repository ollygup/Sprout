<script lang="ts">
  import Icon from "./Icon.svelte";

  export interface ContextMenuItem {
    label: string;
    icon?: string;
    danger?: boolean;
    /** Native option-title parity for select popouts (ticket 204): a hover
     *  tooltip naming the choice's consequence (e.g. version policies). */
    title?: string;
    /** A disabled item (e.g. Move up on the first row) renders greyed and
     *  is skipped by keyboard navigation. */
    disabled?: boolean;
    /** A separator renders a hairline divider row instead of an item. */
    separator?: boolean;
    /** One level of nesting (ticket 101): the row opens a flyout of these
     *  children — organizational destinations, not verbs — instead of
     *  selecting. Children never nest further. */
    children?: Omit<ContextMenuItem, "children">[];
    /** Current-state mark for organizational destinations (ticket 106):
     *  defined → the row announces as `menuitemradio` with `aria-checked`,
     *  and true draws the check glyph even without an explicit icon. */
    checked?: boolean;
    /** A color dot in the icon slot (ticket 225). A hex value paints the
     *  dot; `null` draws a hollow neutral ring; `undefined` leaves the slot
     *  to `icon`/`checked`. */
    swatch?: string | null;
    onselect?: () => void;
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
    /** Anchored placement: below drops the menu under the trigger (default);
     *  above opens it over the content above the trigger — used where the
     *  content below cannot host web menus (e.g. above the native Companion
     *  child, which paints above all CSS). */
    placement?: "below" | "above";
    /** Anchored horizontal alignment: end right-aligns the menu to the
     *  trigger (default — correct for ⋯ row menus); start left-aligns it
     *  (for full-width selectors that read as a select popup). */
    align?: "start" | "end";
    /** Stretch the menu to at least the anchor's width — only meaningful
     *  with align start; ignored for cursor menus. */
    matchAnchorWidth?: boolean;
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
  let menuMinWidth = $state(0);
  let itemEls: (HTMLButtonElement | undefined)[] = [];

  let openIndex = $state<number | null>(null);
  let childActiveIndex = $state(0);
  let childPos = $state({ x: 0, y: 0 });
  let flyoutEl: HTMLDivElement | undefined = $state();
  let childEls: (HTMLButtonElement | undefined)[] = $state([]);
  let hoverCloseTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    if (!ctx?.open) return;
    activeIndex = 0;
    openIndex = null;
    const raf = requestAnimationFrame(() => {
      if (!menuEl) return;
      // Stretch to the anchor first so the measured size — and the rendered
      // width below, which Svelte rewrites wholesale on each pos update —
      // both follow the trigger.
      const anchorWidth =
        ctx.anchor && ctx.matchAnchorWidth
          ? Math.max(0, ctx.anchor.getBoundingClientRect().width)
          : 0;
      menuEl.style.minWidth = anchorWidth ? `${anchorWidth}px` : "";
      const size = menuEl.getBoundingClientRect();
      let x: number;
      let y: number;
      if (ctx.anchor) {
        const r = ctx.anchor.getBoundingClientRect();
        x = ctx.align === "start" ? r.left : r.right - size.width;
        y = ctx.placement === "above" ? r.top - size.height - 6 : r.bottom + 6;
      } else {
        x = ctx.x ?? 0;
        y = ctx.y ?? 0;
      }
      x = Math.max(8, Math.min(x, window.innerWidth - size.width - 8));
      y = Math.max(8, Math.min(y, window.innerHeight - size.height - 8));
      pos = { x, y };
      menuMinWidth = anchorWidth;
      if (ctx.focusFirst) itemEls[firstFocusable()]?.focus();
    });
    return () => {
      cancelAnimationFrame(raf);
      clearTimeout(hoverCloseTimer);
    };
  });

  // The open flyout sits beside its parent row, mirroring left near the
  // viewport's right edge and clamped like the root menu.
  $effect(() => {
    const i = openIndex;
    if (i === null || !ctx?.open) return;
    const raf = requestAnimationFrame(() => {
      const btn = itemEls[i];
      if (!btn || !flyoutEl) return;
      const br = btn.getBoundingClientRect();
      const size = flyoutEl.getBoundingClientRect();
      let x = br.right + 6;
      if (x + size.width > window.innerWidth - 8) {
        x = Math.max(8, br.left - size.width - 6);
      }
      let y = br.top - 4;
      y = Math.max(8, Math.min(y, window.innerHeight - size.height - 8));
      childPos = { x, y };
    });
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    if (!ctx?.open) return;
    const onWindowKeydown = (e: KeyboardEvent) => {
      const focusInside = menuEl?.contains(document.activeElement) ?? false;
      const focusInChild = flyoutEl?.contains(document.activeElement) ?? false;
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          if (focusInChild) {
            const parent = openIndex;
            closeSubmenu();
            itemEls[parent ?? activeIndex]?.focus();
          } else {
            closeAndRestore();
          }
          break;
        case "Tab":
          closeAndRestore();
          break;
        case "ArrowDown":
          if (focusInChild) {
            e.preventDefault();
            moveChildFocus(1);
          } else if (focusInside) {
            e.preventDefault();
            moveFocus(1);
          }
          break;
        case "ArrowUp":
          if (focusInChild) {
            e.preventDefault();
            moveChildFocus(-1);
          } else if (focusInside) {
            e.preventDefault();
            moveFocus(-1);
          }
          break;
        case "ArrowRight": {
          if (!focusInside || focusInChild) return;
          const item = ctx.items[activeIndex];
          if (item?.children && !item.disabled) {
            e.preventDefault();
            openSubmenu(activeIndex);
            childEls[firstFocusableChild(item.children)]?.focus();
          }
          break;
        }
        case "ArrowLeft":
          if (focusInChild) {
            e.preventDefault();
            const parent = openIndex;
            closeSubmenu();
            if (parent !== null) itemEls[parent]?.focus();
          }
          break;
        case "Home":
          if (focusInChild) {
            e.preventDefault();
            setActiveChild(firstFocusableChild(openChildren()));
          } else if (focusInside) {
            e.preventDefault();
            setActive(0);
          }
          break;
        case "End":
          if (focusInChild) {
            e.preventDefault();
            setActiveChild(lastFocusableChild(openChildren()));
          } else if (focusInside) {
            e.preventDefault();
            setActive((ctx?.items.length ?? 1) - 1);
          }
          break;
      }
    };
    const onWindowPointerDown = (e: PointerEvent) => {
      if (menuEl?.contains(e.target as Node)) return;
      // The trigger carries `data-ctx-trigger` on the button, but pointer
      // targets land on inner content (label span, chevron, icon svg) — walk
      // up so trigger clicks toggle instead of closing-then-reopening.
      if ((e.target as Element | null)?.closest?.("[data-ctx-trigger]")) return;
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

  function openChildren(): ContextMenuItem[] {
    return (openIndex !== null ? ctx?.items[openIndex]?.children : undefined) ?? [];
  }

  function firstFocusableChild(children: ContextMenuItem[]): number {
    for (let i = 0; i < children.length; i++) {
      const item = children[i];
      if (item && !item.separator && !item.disabled) return i;
    }
    return 0;
  }

  function lastFocusableChild(children: ContextMenuItem[]): number {
    for (let i = children.length - 1; i >= 0; i--) {
      const item = children[i];
      if (item && !item.separator && !item.disabled) return i;
    }
    return 0;
  }

  function moveChildFocus(dir: number) {
    const children = openChildren();
    const n = children.length;
    if (n === 0) return;
    let i = childActiveIndex;
    for (let step = 0; step < n; step++) {
      i = (i + dir + n) % n;
      const item = children[i];
      if (item && !item.separator && !item.disabled) {
        setActiveChild(i);
        return;
      }
    }
  }

  function setActiveChild(i: number) {
    childActiveIndex = i;
    childEls[i]?.focus();
  }

  function openSubmenu(i: number) {
    clearTimeout(hoverCloseTimer);
    childEls = [];
    childActiveIndex = 0;
    openIndex = i;
  }

  function closeSubmenu() {
    clearTimeout(hoverCloseTimer);
    childEls = [];
    openIndex = null;
  }

  /* Hovering away from a parent row and its flyout closes the flyout after a
     short grace period — long enough that the diagonal trip from the row to
     the flyout never slams it shut. Entering either side cancels the timer. */
  function scheduleFlyoutClose() {
    if (openIndex === null) return;
    clearTimeout(hoverCloseTimer);
    hoverCloseTimer = setTimeout(() => {
      openIndex = null;
      childEls = [];
    }, 240);
  }

  function cancelFlyoutClose() {
    clearTimeout(hoverCloseTimer);
  }

  function closeAndRestore() {
    if (menuEl?.contains(document.activeElement)) ctx?.returnTo?.focus();
    onclose();
  }

  function select(item: ContextMenuItem) {
    item.onselect?.();
    onclose();
  }

  /** Whether a menu list needs the leading check/icon gutter: any row
   *  carrying an icon or a radio state reserves the slot for every row, so
   *  unchecked rows align with checked ones instead of sitting left-shifted
   *  (the Windows/macOS menu convention; NN/g left-edge scannability —
   *  eyes scan straight down one edge). Lists with neither render exactly
   *  as before — no empty gutter appears. */
  function needsGutter(
    list: { separator?: boolean; icon?: string; checked?: boolean; swatch?: string | null }[],
  ): boolean {
    return list.some(
      (item) =>
        !item.separator &&
        (item.icon !== undefined ||
          item.checked !== undefined ||
          item.swatch !== undefined),
    );
  }
</script>

{#if ctx?.open}
  <div
    bind:this={menuEl}
    class="ctx-menu"
    role="menu"
    aria-label={ctx.label}
    style="left: {pos.x}px; top: {pos.y}px;{menuMinWidth ? ` min-width: ${menuMinWidth}px;` : ""}"
  >
    {#each ctx.items as item, i}
      {#if item.separator}
        <div class="ctx-sep" role="separator"></div>
      {:else}
        <button
          bind:this={itemEls[i]}
          type="button"
          class="ctx-item"
          class:active={activeIndex === i}
          class:danger={item.danger}
          role={item.checked === undefined ? "menuitem" : "menuitemradio"}
          aria-checked={item.checked}
          title={item.title}
          tabindex={activeIndex === i ? 0 : -1}
          disabled={item.disabled}
          aria-haspopup={item.children ? "true" : undefined}
          aria-expanded={item.children ? openIndex === i : undefined}
          onclick={() => {
            if (item.disabled) return;
            if (item.children) {
              if (openIndex === i) {
                closeSubmenu();
                itemEls[i]?.focus();
              } else {
                openSubmenu(i);
                childEls[firstFocusableChild(item.children)]?.focus();
              }
            } else {
              select(item);
            }
          }}
          onpointerenter={() => {
            if (item.disabled) return;
            activeIndex = i;
            if (item.children) {
              cancelFlyoutClose();
              if (openIndex !== i) openSubmenu(i);
            } else if (openIndex !== null) {
              closeSubmenu();
            }
          }}
          onpointerleave={() => {
            if (item.children && openIndex === i) scheduleFlyoutClose();
          }}
        >
          {#if ctx.items && needsGutter(ctx.items)}
            <span class="ctx-item__icon" aria-hidden="true">
              {#if item.swatch !== undefined}
                <span
                  class="ctx-item__swatch"
                  class:ctx-item__swatch--none={item.swatch === null}
                  style:--swatch-color={item.swatch ?? undefined}
                ></span>
              {:else if item.icon || item.checked}
                <Icon name={item.icon ?? "check"} size={14} />
              {/if}
            </span>
          {/if}
          <span>{item.label}</span>
          {#if item.children}
            <span class="ctx-item__more" aria-hidden="true"><Icon name="chevron-right" size={13} /></span>
          {/if}
        </button>
        {#if item.children && openIndex === i}
          <div
            bind:this={flyoutEl}
            class="ctx-menu ctx-submenu"
            role="menu"
            aria-label={item.label}
            tabindex="-1"
            style="left: {childPos.x}px; top: {childPos.y}px;"
            onpointerenter={cancelFlyoutClose}
            onpointerleave={scheduleFlyoutClose}
          >
            {#each item.children as child, ci}
              {#if child.separator}
                <div class="ctx-sep" role="separator"></div>
              {:else}
                <button
                  bind:this={childEls[ci]}
                  type="button"
                  class="ctx-item"
                  class:active={childActiveIndex === ci}
                  class:danger={child.danger}
                  role={child.checked === undefined ? "menuitem" : "menuitemradio"}
                  aria-checked={child.checked}
                  title={child.title}
                  tabindex={childActiveIndex === ci ? 0 : -1}
                  disabled={child.disabled}
                  onclick={() => select(child)}
                  onpointerenter={() => {
                    if (!child.disabled) setActiveChild(ci);
                  }}
                >
                  {#if item.children && needsGutter(item.children)}
                    <span class="ctx-item__icon" aria-hidden="true">
                      {#if child.swatch !== undefined}
                        <span
                          class="ctx-item__swatch"
                          class:ctx-item__swatch--none={child.swatch === null}
                          style:--swatch-color={child.swatch ?? undefined}
                        ></span>
                      {:else if child.icon || child.checked}
                        <Icon name={child.icon ?? "check"} size={14} />
                      {/if}
                    </span>
                  {/if}
                  <span>{child.label}</span>
                </button>
              {/if}
            {/each}
          </div>
        {/if}
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
    /* Ticket-204 deviation (ADR-0028 review slot, research 0021): long
       option lists scroll inside the popout instead of overflowing the
       window. Pure cap — short menus render exactly as before. */
    max-height: min(320px, calc(100vh - 16px));
    overflow-y: auto;
    padding: 4px;
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-dialog);
    /* Mount entrance: menus arrive from nowhere, so a short rise with a
       settle-scale keeps them from flashing in — the dropdown/drop-down
       grammar, on the fast token. Pure CSS on the motion tokens, so the
       Animation off hook and the OS reduced-motion rule collapse it with
       everything else. */
    animation: menu-in var(--dur-fast) var(--ease-out);
  }

  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(-6px) scale(0.97);
    }
  }

  .ctx-submenu {
    z-index: 101;
    max-height: calc(100vh - 16px);
    overflow-y: auto;
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
    align-items: center;
    justify-content: center;
    /* Fixed icon-size slot: rows with a glyph render exactly as before,
       while empty slots keep unchecked rows on the same left edge. */
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--text-muted);
  }

  .ctx-item__more {
    display: inline-flex;
    margin-left: auto;
    color: var(--text-muted);
  }

  /* A group color dot in the reserved icon slot; neutral is a hollow ring
     so "no color" reads without inventing a token color (ticket 225). */
  .ctx-item__swatch {
    width: 10px;
    height: 10px;
    border-radius: var(--radius-pill);
    background: var(--swatch-color);
  }

  .ctx-item__swatch--none {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--border-strong);
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
