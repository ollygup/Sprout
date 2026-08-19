<script lang="ts">
  import type { Product } from "$lib/types";
  import { envActionLabel } from "$lib/types";
  import Icon from "./Icon.svelte";
  import type { MenuRequest } from "./ContextMenu.svelte";

  let {
    product,
    index,
    expanded = false,
    onmenu,
    oninfo,
    animate = false,
  }: {
    product: Product;
    index: number;
    expanded?: boolean;
    onmenu: (request: MenuRequest) => void;
    oninfo: () => void;
    animate?: boolean;
  } = $props();

  const visibleEnv = $derived(product.default_env.slice(0, 3));
  const extraEnv = $derived(product.default_env.length - visibleEnv.length);
  let card: HTMLElement | undefined = $state();
  let dots: HTMLButtonElement | undefined = $state();

  function openAtCursor(e: MouseEvent) {
    e.preventDefault();
    // A keyboard-triggered contextmenu (Shift+F10 / Menu key) has detail 0
    // and no meaningful cursor position — anchor to the card instead.
    if (e.detail === 0) {
      onmenu({ kind: "anchor", anchor: card ?? null, focusFirst: true, returnTo: card ?? null });
    } else {
      onmenu({ kind: "cursor", x: e.clientX, y: e.clientY, returnTo: card ?? null });
    }
  }

  function onDotsClick(e: MouseEvent) {
    e.stopPropagation();
    onmenu({
      kind: "anchor",
      anchor: dots ?? null,
      focusFirst: e.detail === 0,
      returnTo: dots ?? null,
    });
  }

  function onCardKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      oninfo();
    }
  }
</script>

<div
  bind:this={card}
  class="packet"
  class:animate
  tabindex="0"
  role="button"
  aria-haspopup="menu"
  aria-expanded={expanded}
  aria-label={`More info for ${product.name}`}
  style={animate ? `animation-delay: ${Math.min(index * 28, 420)}ms` : undefined}
  onkeydown={onCardKeydown}
  onclick={oninfo}
  oncontextmenu={openAtCursor}
>
  <div class="packet__band">
    <div class="packet__band-row">
      <h3 class="packet__name">{product.name}</h3>
      <span class="packet__type" class:custom={!product.winget_id}>
        {product.winget_id ? "winget" : "custom step"}
      </span>
    </div>
  </div>

  <div class="packet__body">
    {#if product.winget_id}
      <p class="packet__id" title="winget ID">{product.winget_id}</p>
    {:else}
      <p class="packet__id packet__id--custom">Custom install step</p>
    {/if}

    <div class="packet__tags">
      {#if product.install_location_hint}
        <span class="tag tag--hint" title="Install location hint">loc: {product.install_location_hint}</span>
      {/if}
      {#if product.install_dir}
        <span class="tag tag--hint" title="Install directory override">dir: {product.install_dir}</span>
      {/if}
      {#each visibleEnv as item (item.name + item.action)}
        <span class="tag" title="{envActionLabel[item.action]} {item.name} = {item.value}">
          {envActionLabel[item.action]} {item.name}
        </span>
      {/each}
      {#if extraEnv > 0}
        <span class="tag tag--more">+{extraEnv} more</span>
      {/if}
    </div>
  </div>

  <div class="packet__tear" aria-hidden="true"></div>

  <footer class="packet__foot">
    <button
      bind:this={dots}
      type="button"
      class="packet__dots"
      data-ctx-trigger
      aria-haspopup="menu"
      aria-expanded={expanded}
      aria-label={`More actions for ${product.name}`}
      title="More actions"
      onclick={onDotsClick}
    >
      <Icon name="dots" size={15} />
    </button>
  </footer>
</div>

<style>
  .packet {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 156px;
    overflow: hidden;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-card);
    cursor: pointer;
    transition: transform var(--dur) var(--ease-out),
      border-color var(--dur) var(--ease-out);
  }

  .packet:hover {
    transform: translateY(-2px);
    border-color: var(--border-strong);
  }

  .packet.animate {
    opacity: 0;
    animation: rise 360ms var(--ease-out) forwards;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  /* the two-tone seed-packet band: accent folds into warm at its foot */
  .packet__band {
    padding: var(--space-3) var(--space-4) var(--space-2);
    background: linear-gradient(
      180deg,
      var(--accent-tint) 0%,
      var(--accent-tint) 55%,
      var(--warm-tint) 100%
    );
    border-bottom: 1px solid var(--border);
  }

  .packet__band-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .packet__name {
    font-family: var(--font-display);
    font-size: var(--text-lg);
    font-weight: 600;
    line-height: 1.22;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .packet__type {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    padding: 2px 8px;
    border: 1px solid var(--accent-tint-border);
    border-radius: var(--radius-pill);
    color: var(--accent);
    background: var(--bg-surface);
  }

  .packet__type.custom {
    border-color: var(--warm-tint-border);
    color: var(--warm-text);
  }

  .packet__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4) 0;
  }

  .packet__id {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  .packet__id--custom {
    color: var(--warm-text);
    letter-spacing: 0.02em;
  }

  .packet__tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .tag {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: 0.04em;
    padding: 2px 7px;
    border: 1px solid var(--accent-tint-border);
    border-radius: var(--radius-lg);
    color: var(--accent);
    background: var(--accent-tint);
  }

  .tag--hint {
    border-color: var(--border-strong);
    color: var(--text-muted);
    background: transparent;
  }

  .tag--more {
    border-style: dashed;
    border-color: var(--border-strong);
    color: var(--text-muted);
    background: transparent;
  }

  /* the signature: a seed-packet tear line */
  .packet__tear {
    margin-top: auto;
    border-top: 2px dashed var(--warm);
    margin-bottom: 0;
  }

  .packet__tear::after {
    content: "";
    display: block;
    height: 4px;
    background: linear-gradient(
      90deg,
      var(--bg-sunken) 0 8px,
      var(--bg-surface) 8px 12px,
      var(--bg-sunken) 12px 20px,
      var(--bg-surface) 20px 24px,
      var(--bg-sunken) 24px 32px,
      var(--bg-surface) 32px 36px,
      var(--bg-sunken) 36px 44px,
      var(--bg-surface) 44px 48px,
      var(--bg-sunken) 48px 56px,
      var(--bg-surface) 56px 100%
    );
  }

  .packet__foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 3px var(--space-3) 4px;
  }

  .packet__dots {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .packet:hover .packet__dots,
  .packet:focus-within .packet__dots,
  .packet:focus .packet__dots {
    opacity: 1;
  }

  .packet__dots:hover,
  .packet__dots:focus-visible {
    background: var(--bg-hover);
    color: var(--text);
  }

  @media (hover: none) {
    .packet__dots {
      opacity: 1;
    }
  }
</style>