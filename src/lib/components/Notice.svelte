<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    tone = "ok",
    children,
  }: {
    tone?: "ok" | "error" | "warn";
    children: Snippet;
  } = $props();
</script>

<p
  class="notice notice--{tone}"
  role={tone === "error" ? "alert" : "status"}
  aria-live={tone === "error" ? undefined : "polite"}
>
  {@render children()}
</p>

<style>
  .notice {
    margin: 0 0 var(--space-3);
    font-size: var(--text-sm);
    overflow-wrap: anywhere;
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