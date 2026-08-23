<script lang="ts">
  import type { Product } from "$lib/types";
  import { envActionLabel } from "$lib/types";
  import PacketCard from "./PacketCard.svelte";
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
</script>

<PacketCard
  name={product.name}
  cardLabel={`More info for ${product.name}`}
  badge={{
    text: product.winget_id ? "winget" : "custom step",
    tone: product.winget_id ? "accent" : "warm",
    upper: true,
  }}
  {index}
  {animate}
  {expanded}
  onactivate={oninfo}
  {onmenu}
>
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
</PacketCard>

<style>
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
</style>
