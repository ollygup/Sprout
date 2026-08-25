<script lang="ts">
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    name,
    stoppable,
    running,
    stopping,
    onrun,
    onstop,
    describedby,
  }: {
    name: string;
    stoppable: boolean;
    running: boolean;
    stopping: boolean;
    onrun: () => void;
    onstop: () => void;
    /** The row tooltip this control describes (Quick Launch window rows);
     *  omitted where a surface has no tooltip to point at. */
    describedby?: string;
  } = $props();
</script>

{#if stopping}
  <!-- Ticket 92's contract, owned once since ticket 98: Stop in flight —
       disabled and muted until the exit event lands; the spinner is the
       honest "something is happening" (research 0004 rule 5). -->
  <Button
    variant="secondary"
    disabled
    aria-label={`Stopping ${name}`}
    aria-describedby={describedby}
  >
    <span class="spin" aria-hidden="true"></span>
    Stopping…
  </Button>
{:else if stoppable && running}
  <!-- The destructive verb gets the danger family, never the accent — one
       primary verb per row (research 0005 rule 2). -->
  <Button
    variant="danger"
    onclick={onstop}
    aria-label={`Stop ${name}`}
    aria-describedby={describedby}
  >
    <Icon name="stop" size={13} />
    Stop
  </Button>
{:else}
  <!-- Run is the row's primary verb — accent-filled (research 0005 rule 2);
       color signals the single next step (research 0006 pattern 6). -->
  <Button
    variant="primary"
    onclick={onrun}
    aria-label={`Run ${name}`}
    aria-describedby={describedby}
  >
    <Icon name="play" size={13} />
    Run
  </Button>
{/if}

<style>
  /* Ticket 98: the Stopping spinner — token families only (border track,
     muted head); the button's own disabled treatment mutes the whole thing.
     Reduced motion freezes it into a plain ring beside the "Stopping…"
     text instead of spinning. */
  .spin {
    flex-shrink: 0;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    border-top-color: var(--text-muted);
    animation: qa-run-spin 0.8s linear infinite;
  }

  @keyframes qa-run-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }
</style>
