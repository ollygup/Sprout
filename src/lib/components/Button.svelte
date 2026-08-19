<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  let {
    variant = "primary",
    kind = "button",
    type = "button",
    onclick,
    disabled = false,
    children,
    ...rest
  }: {
    variant?: "primary" | "secondary" | "danger" | "ghost";
    kind?: "button" | "submit";
    onclick?: () => void;
    disabled?: boolean;
    children: Snippet;
  } & HTMLButtonAttributes = $props();

  let btn: HTMLButtonElement | undefined = $state();
</script>

<button
  bind:this={btn}
  type={kind === "submit" ? "submit" : "button"}
  class="btn btn--{variant}"
  {disabled}
  onclick={onclick}
  {...rest}
>
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    padding: 8px 16px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    font-family: var(--font-body);
    font-size: var(--text-base);
    font-weight: 600;
    line-height: 1.2;
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--accent);
    color: var(--on-accent);
  }

  .btn--primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn--secondary {
    background: transparent;
    border-color: var(--border-strong);
    color: var(--text);
  }

  .btn--secondary:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .btn--danger {
    background: var(--danger-text);
    color: var(--on-accent);
  }

  .btn--danger:hover:not(:disabled) {
    filter: brightness(1.06);
  }

  .btn--ghost {
    background: transparent;
    color: var(--text-muted);
  }

  .btn--ghost:hover:not(:disabled) {
    color: var(--text);
    background: var(--bg-hover);
  }
</style>
