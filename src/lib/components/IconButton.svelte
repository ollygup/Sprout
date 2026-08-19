<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon from "./Icon.svelte";

  let {
    icon,
    label,
    onclick,
    disabled = false,
    quiet = false,
    ...rest
  }: {
    icon: string;
    label: string;
    onclick?: (e: MouseEvent) => void;
    disabled?: boolean;
    quiet?: boolean;
  } & HTMLButtonAttributes = $props();
</script>

<button
  type="button"
  class="icon-btn"
  class:quiet
  onclick={onclick}
  {disabled}
  aria-label={label}
  title={label}
  {...rest}
>
  <Icon name={icon} size={15} />
</button>

<style>
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text);
  }

  .icon-btn.quiet:hover:not(:disabled) {
    border-color: var(--border);
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
