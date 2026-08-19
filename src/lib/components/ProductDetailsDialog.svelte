<script lang="ts">
  import type { Product } from "$lib/types";
  import { envActionLabel } from "$lib/types";
  import Dialog from "./Dialog.svelte";

  let {
    open,
    product,
    onclose,
  }: {
    open: boolean;
    product: Product | null;
    onclose: () => void;
  } = $props();

  function formatDateTime(ts: number): string {
    return new Date(ts * 1000).toLocaleString([], {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

<Dialog {open} title={product ? `About ${product.name}` : "More info"} onclose={onclose} width={420}>
  {#if product}
    <dl class="details">
      <div class="details__row">
        <dt>Type</dt>
        <dd>{product.winget_id ? "winget-managed" : "custom install step"}</dd>
      </div>
      <div class="details__row">
        <dt>winget ID</dt>
        <dd class="mono">{product.winget_id ?? "none — runs via a custom install step"}</dd>
      </div>
      <div class="details__row">
        <dt>Install location hint</dt>
        <dd class="mono">{product.install_location_hint ?? "none"}</dd>
      </div>
      <div class="details__row">
        <dt>Install directory</dt>
        <dd class="mono">
          {product.install_dir ?? "default (from Settings)"}
          {#if product.install_dir}
            <span class="details__muted">— override</span>
          {/if}
        </dd>
      </div>
      {#if product.created_at}
        <div class="details__row">
          <dt>Added</dt>
          <dd>{formatDateTime(product.created_at)}</dd>
        </div>
      {/if}
      {#if product.updated_at}
        <div class="details__row">
          <dt>Last updated</dt>
          <dd>{formatDateTime(product.updated_at)}</dd>
        </div>
      {/if}
      <div class="details__row details__row--env">
        <dt>Default env wiring</dt>
        <dd>
          {#if product.default_env.length === 0}
            none
          {:else}
            <ul class="env-list">
              {#each product.default_env as row (row.name + row.action)}
                <li class="mono">
                  {envActionLabel[row.action]} <span class="env-list__name">{row.name}</span> = {row.value}
                </li>
              {/each}
            </ul>
          {/if}
        </dd>
      </div>
    </dl>
    <p class="details__hint">
      Presets that reference this product keep their own embedded copy of it.
    </p>
  {/if}
</Dialog>

<style>
  .details {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .details__row {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: var(--space-3);
    align-items: baseline;
  }

  .details__row dt {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .details__row dd {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .details__row--env {
    align-items: start;
  }

  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .env-list {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .env-list__name {
    color: var(--accent);
  }

  .details__muted {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .details__hint {
    margin: var(--space-4) 0 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }
</style>