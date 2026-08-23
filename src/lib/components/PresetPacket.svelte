<script lang="ts">
  import type { PresetRecord } from "$lib/types";
  import PacketCard from "./PacketCard.svelte";
  import type { MenuRequest } from "./ContextMenu.svelte";

  let {
    record,
    index,
    expanded = false,
    onmenu,
    animate = false,
  }: {
    record: PresetRecord;
    index: number;
    expanded?: boolean;
    onmenu: (request: MenuRequest) => void;
    animate?: boolean;
  } = $props();

  const requirements = $derived(record.requirements);
  const envCount = $derived(requirements.reduce((n, r) => n + r.env.length, 0));
  const verifyCount = $derived(requirements.reduce((n, r) => n + r.verify.length, 0));
  const visibleProducts = $derived(
    requirements.slice(0, 3).map((r) => r.product.name || r.product.id)
  );
  const extraProducts = $derived(requirements.length - visibleProducts.length);
</script>

<PacketCard
  name={record.name}
  cardLabel={`Actions for ${record.name}`}
  badge={{ text: `v${record.version}`, title: "Preset version" }}
  {index}
  {animate}
  {expanded}
  {onmenu}
>
  <p class="packet__desc">{record.description}</p>

  <div class="packet__tags">
    <span class="tag">
      {requirements.length} requirement{requirements.length === 1 ? "" : "s"}
    </span>
    {#each visibleProducts as product (product)}
      <span class="tag tag--product">{product}</span>
    {/each}
    {#if extraProducts > 0}
      <span class="tag tag--more">+{extraProducts} more</span>
    {/if}
    {#if record.imported}
      <span class="tag tag--imported" title="Imported from a .sprout.json — stored as authored, fork to edit">
        imported
      </span>
    {/if}
    {#if envCount > 0}
      <span class="tag tag--env" title="Env wiring entries">env: {envCount}</span>
    {/if}
    {#if verifyCount > 0}
      <span class="tag tag--env" title="Verify commands">verify: {verifyCount}</span>
    {/if}
  </div>

  {#if record.author}
    <p class="packet__author">by {record.author}</p>
  {/if}
</PacketCard>

<style>
  .packet__desc {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
    overflow-wrap: anywhere;
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
    border: 1px solid var(--warm-tint-border);
    border-radius: var(--radius-lg);
    color: var(--warm-text);
    background: var(--warm-tint);
  }

  .tag--product {
    border-color: var(--border-strong);
    color: var(--text-muted);
    background: transparent;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag--env {
    border-color: var(--accent-tint-border);
    color: var(--accent);
    background: var(--accent-tint);
  }

  .tag--imported {
    border-style: dashed;
    border-color: var(--warm-tint-border);
    color: var(--warm-text);
    background: transparent;
  }

  .tag--more {
    border-style: dashed;
    border-color: var(--border-strong);
    color: var(--text-muted);
    background: transparent;
  }

  .packet__author {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }
</style>
