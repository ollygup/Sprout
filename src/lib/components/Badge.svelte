<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    tone = "muted",
    children,
  }: {
    tone?: "accent" | "warm" | "muted" | "info" | "faint" | "danger" | "warn" | "status-applied" | "status-notes" | "status-cancelled" | "status-failed";
    children: Snippet;
  } = $props();
</script>

<span class="badge badge--{tone}">{@render children()}</span>

<style>
  .badge {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    white-space: nowrap;
  }

  .badge--accent {
    background: var(--accent-tint);
    color: var(--accent);
    border-color: var(--accent-tint-border);
  }

  .badge--warm {
    background: var(--warm-tint);
    color: var(--warm-text);
    border-color: var(--warm-tint-border);
  }

  .badge--muted {
    background: transparent;
    color: var(--text-muted);
    border-color: var(--border-strong);
  }

  .badge--info {
    background: var(--info-tint);
    color: var(--info-text);
    border-color: var(--info-tint-border);
  }

  .badge--faint {
    background: transparent;
    color: var(--text-muted);
    border: 1px dashed var(--border-strong);
  }

  .badge--danger {
    background: var(--danger-tint);
    color: var(--danger-text);
    border-color: var(--danger-tint-border);
  }

  .badge--warn {
    background: var(--warn-tint);
    color: var(--warn-text);
    border-color: var(--warn-tint-border);
  }

  .badge--status-applied {
    background: var(--status-applied-tint);
    color: var(--status-applied-text);
    border-color: var(--status-applied-tint-border);
  }

  .badge--status-notes {
    background: var(--status-notes-tint);
    color: var(--status-notes-text);
    border-color: var(--status-notes-tint-border);
  }

  .badge--status-cancelled {
    background: var(--status-cancelled-tint);
    color: var(--status-cancelled-text);
    border-color: var(--status-cancelled-tint-border);
  }

  .badge--status-failed {
    background: var(--status-failed-tint);
    color: var(--status-failed-text);
    border-color: var(--status-failed-tint-border);
  }
</style>