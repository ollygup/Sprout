<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    tone = "ok",
    children,
    action,
  }: {
    tone?: "ok" | "error" | "warn";
    children: Snippet;
    action?: Snippet;
  } = $props();
</script>

<p
  class="notice notice--{tone}"
  role={tone === "error" ? "alert" : "status"}
  aria-live={tone === "error" ? undefined : "polite"}
>
  <span class="notice__text">{@render children()}</span>
  {#if action}
    <span class="notice__action">{@render action()}</span>
  {/if}
</p>

<style>
  .notice {
    margin: 0 0 var(--space-3);
    font-size: var(--text-sm);
    overflow-wrap: anywhere;
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .notice__text {
    flex: 1;
    min-width: 0;
  }

  .notice__action {
    flex: none;
    display: inline-flex;
  }

  .notice--ok {
    color: var(--accent);
  }

  .notice--error {
    color: var(--danger-text);
  }

  .notice--warn {
    color: var(--warm-text);
  }
</style>