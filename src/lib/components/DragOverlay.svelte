<script lang="ts">
  import type { DragState } from "$lib/collectionDrag.svelte";

  /** The drag's visual layer (tickets 232–233): a small ghost that follows the
   *  pointer, the insertion line for reorder drops, and the refusal copy when
   *  the target cannot accept the drop. It is `pointer-events: none`, so
   *  `elementFromPoint` keeps seeing the real row underneath.
   *
   *  The ghost and line are static boxes — no animated transform or opacity —
   *  which keeps the whole interaction reduced-motion-safe by construction.
   *  The live region announces refusals for screen-reader users. */
  let { drag, message }: { drag: DragState | null; message: string } = $props();
</script>

{#if drag}
  <div class="drag-layer" aria-hidden="true">
    {#if drag.line}
      <span
        class="drag-line"
        style:left={`${drag.line.left}px`}
        style:top={`${drag.line.top}px`}
        style:width={`${drag.line.width}px`}
      ></span>
    {/if}
    <div
      class="drag-ghost"
      class:invalid={!drag.valid}
      style:left={`${drag.x + 14}px`}
      style:top={`${drag.y + 14}px`}
    >
      <span class="drag-ghost__label">{drag.label}</span>
      {#if drag.reason}
        <span class="drag-ghost__reason">{drag.reason}</span>
      {/if}
    </div>
  </div>
{/if}

<p class="sr-only" aria-live="polite">{message}</p>

<style>
  .drag-layer {
    position: fixed;
    inset: 0;
    z-index: 1200;
    pointer-events: none;
  }

  .drag-line {
    position: fixed;
    height: 2px;
    border-radius: 1px;
    background: var(--accent);
    transform: translateY(-1px);
  }

  .drag-ghost {
    position: fixed;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: 280px;
    padding: var(--space-1) var(--space-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-page);
    box-shadow: var(--shadow-dialog);
    color: var(--text);
    font-size: var(--text-sm);
    line-height: 1.3;
  }

  .drag-ghost.invalid {
    border-style: dashed;
    color: var(--text-muted);
  }

  .drag-ghost__label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .drag-ghost__reason {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  /* The body carries the drag's cursor so every element under the pointer
     agrees, including rows that set their own cursor. */
  :global(body.sprout-dragging),
  :global(body.sprout-dragging *) {
    cursor: grabbing !important;
    user-select: none;
  }

  :global(body.sprout-dragging.sprout-drag-invalid),
  :global(body.sprout-dragging.sprout-drag-invalid *) {
    cursor: not-allowed !important;
  }
</style>
