<script lang="ts">
  import Button from "./Button.svelte";
  import type { RunOutcome } from "$lib/types";
  import {
    getActivity,
    getCompletionLabel,
    requestCancel,
    runAwareness,
  } from "$lib/runAwareness.svelte";

  const shown = $derived(runAwareness.activeRunId !== null);
  const finished = $derived(runAwareness.completion !== null);
  const activity = $derived(getActivity());
  const completionLabel = $derived(getCompletionLabel());

  function outcomeTone(outcome: RunOutcome): string {
    return {
      ok: "var(--status-applied)",
      with_notes: "var(--status-notes)",
      cancelled: "var(--status-cancelled)",
      failed: "var(--status-failed)",
    }[outcome];
  }

  const dotColor = $derived(
    finished && runAwareness.completion
      ? outcomeTone(runAwareness.completion.outcome)
      : "var(--warm)"
  );
</script>

{#if shown}
  <aside class="banner" role="status" aria-live="polite">
    <span
      class="banner__dot"
      class:live={!finished}
      style:background={dotColor}
      aria-hidden="true"
    ></span>
    <div class="banner__body">
      {#if finished}
        <p class="banner__line">
          Run finished — <span class="banner__outcome">{completionLabel}</span>
        </p>
        <p class="banner__sub">
          {runAwareness.completion?.error ??
            (runAwareness.completion?.outcome === "ok"
              ? "everything applied cleanly"
              : runAwareness.completion?.outcome === "with_notes"
                ? "some products need attention"
                : runAwareness.completion?.outcome === "cancelled"
                  ? "stopped after the current step"
                  : "something failed")}
        </p>
      {:else}
        <p class="banner__line">Run in progress</p>
        <p class="banner__sub">{activity}</p>
      {/if}
    </div>
    {#if !finished}
      <Button variant="danger" onclick={requestCancel} disabled={runAwareness.cancelRequested}>
        {runAwareness.cancelRequested ? "Cancelling after this step…" : "Cancel run"}
      </Button>
    {/if}
    <span class="banner__id">{runAwareness.activeRunId}</span>
  </aside>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-7);
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .banner__dot {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }

  .banner__dot.live {
    animation: banner-pulse 1s ease-in-out infinite;
  }

  @keyframes banner-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  .banner__body {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }

  .banner__line {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text);
  }

  .banner__outcome {
    color: var(--warm-text);
  }

  .banner__sub {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .banner__id {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-faint);
  }
</style>
