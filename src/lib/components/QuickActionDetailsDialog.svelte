<script lang="ts">
  import type { QuickAction } from "$lib/types";
  import { formatNote, hasNote } from "$lib/noteFormat";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import QuickActionRunControl from "./QuickActionRunControl.svelte";

  let {
    open,
    action,
    onclose,
    onedit,
    onrun,
    onstop,
    running = false,
    stopping = false,
  }: {
    open: boolean;
    action: QuickAction | null;
    onclose: () => void;
    onedit: (action: QuickAction) => void;
    onrun: (action: QuickAction) => void;
    onstop: (action: QuickAction) => void;
    running?: boolean;
    stopping?: boolean;
  } = $props();

  const rendered = $derived(action?.note ? formatNote(action.note) : "");
  const hasRenderedNote = $derived(hasNote(action?.note));
</script>

<!-- Row detail surface — one grammar per surface class (research 0006 pattern 13):
     the same centered Dialog the Products page uses, so click-a-row → details
     is learned once. Notes render read-only from markdown-lite; content past
     the existence glyph lives only here, not on compact surfaces
     (research 0004 rule 3, 0006 pattern 14). -->
<Dialog {open} title={action ? `About ${action.name}` : "More info"} onclose={onclose} width={480}>
  {#if action}
    <dl class="details">
      <div class="details__row">
        <dt>Command</dt>
        <dd class="mono">{action.command}</dd>
      </div>
      {#if action.cwd}
        <div class="details__row">
          <dt>Working directory</dt>
          <dd class="mono">{action.cwd}</dd>
        </div>
      {/if}
      {#if action.stoppable}
        <div class="details__row">
          <dt>Stop</dt>
          <dd class="mono">{action.stop_command ?? "kills the process tree"}</dd>
        </div>
      {/if}
    </dl>

    {#if hasRenderedNote}
      <div class="note-block">
        <p class="note-block__label">Note</p>
        <div class="note">{@html rendered}</div>
      </div>
    {:else}
      <p class="details__hint">No note — edit the action to add one.</p>
    {/if}

    <div class="details__actions">
      <Button variant="secondary" onclick={onclose}>Close</Button>
      <Button variant="secondary" onclick={() => onedit(action)}>Edit</Button>
      <QuickActionRunControl
        name={action.name}
        stoppable={action.stoppable}
        {running}
        {stopping}
        onrun={() => onrun(action)}
        onstop={() => onstop(action)}
      />
    </div>
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
    grid-template-columns: 120px 1fr;
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

  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .details__hint {
    margin: var(--space-4) 0 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .note-block {
    margin-top: var(--space-4);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border);
  }

  .note-block__label {
    margin: 0 0 var(--space-2);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  /* Note rendering — token families only, no ad-hoc values (AGENTS.md UI rule).
     Mirrors the body typography; list markers reuse the accent for scent. */
  .note {
    font-family: var(--font-body);
    font-size: var(--text-sm);
    line-height: var(--leading-body);
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .note :global(p) {
    margin: 0 0 var(--space-2);
  }

  .note :global(p:last-child) {
    margin-bottom: 0;
  }

  .note :global(ul),
  .note :global(ol) {
    margin: 0 0 var(--space-2);
    padding-left: var(--space-5);
  }

  .note :global(ul) {
    list-style: disc;
  }

  .note :global(ol) {
    list-style: decimal;
  }

  .note :global(li) {
    margin: 0;
    padding: 0;
  }

  .note :global(li::marker) {
    color: var(--text-muted);
  }

  .note :global(p:last-child),
  .note :global(ul:last-child),
  .note :global(ol:last-child) {
    margin-bottom: 0;
  }

  .details__actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-5);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border);
  }
</style>
