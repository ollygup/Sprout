<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    value,
    placeholder,
    ariaLabel = "Search the library",
    onchange,
  }: {
    value: string;
    placeholder: string;
    ariaLabel?: string;
    onchange: (v: string) => void;
  } = $props();

  let input: HTMLInputElement | undefined = $state();
</script>

<div class="search">
  <span class="search__icon" aria-hidden="true"><Icon name="search" size={14} /></span>
  <input
    bind:this={input}
    class="search__input"
    type="search"
    name="library-search"
    autocomplete="off"
    aria-label={ariaLabel}
    {placeholder}
    value={value}
    oninput={(e) => onchange((e.target as HTMLInputElement).value)}
  />
  {#if value}
    <button
      type="button"
      class="search__clear"
      aria-label="Clear search"
      title="Clear search"
      onclick={() => {
        onchange("");
        input?.focus();
      }}
    >
      <Icon name="x" size={12} />
    </button>
  {/if}
</div>

<style>
  .search {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1;
    max-width: 360px;
  }

  .search__icon {
    position: absolute;
    left: 10px;
    display: inline-flex;
    color: var(--text-muted);
    pointer-events: none;
  }

  .search__input {
    width: 100%;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text);
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 9px 32px 9px 32px;
    transition: border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .search__input::placeholder {
    color: var(--text-muted);
    opacity: 0.8;
  }

  .search__input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .search__input::-webkit-search-cancel-button {
    display: none;
  }

  .search__clear {
    position: absolute;
    right: 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .search__clear:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
</style>
