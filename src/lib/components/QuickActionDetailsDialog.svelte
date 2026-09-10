<script lang="ts">
  import type { QuickAction } from "$lib/types";
  import { quickActionShellLabel } from "$lib/types";
  import { formatNote, hasNote } from "$lib/noteFormat";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
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
    /** Omitted where the surface owns no configuration (ticket 130): the
     *  Quick Launch window/dock shows details read-only — full configuration
     *  lives in the main app (research 0004:3 level 1 vs level 2). */
    onedit?: (action: QuickAction) => void;
    onrun: (action: QuickAction) => void;
    onstop: (action: QuickAction) => void;
    running?: boolean;
    stopping?: boolean;
  } = $props();

  const rendered = $derived(action?.note ? formatNote(action.note) : "");
  const hasRenderedNote = $derived(hasNote(action?.note));

  // The command scent: collapsed to ~3 lines until asked. Reset per action so
  // one long command left expanded never leaks into the next popup.
  let showFullCommand = $state(false);
  // Whether the clamped scent actually hides anything — the toggle and the
  // main-app hint render only then. An affordance that reveals nothing is
  // noise, so disclosure stays content-gated (ADR-0028 design system +
  // disclosure rules).
  let commandOverflows = $state(false);
  let commandEl: HTMLPreElement | undefined = $state(undefined);
  // Copy feedback (research 0004 rule 5 — silence reads as breakage): the
  // label flips only after the write lands, then falls back on the row-flash
  // cadence; the live region announces it politely.
  let copied = $state(false);
  let copyAnnouncement = $state("");
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    void action?.id;
    showFullCommand = false;
    commandOverflows = false;
    copied = false;
    copyAnnouncement = "";
    clearTimeout(copyTimer);
  });

  // Overflow is read post-paint only, never in render — while clamped,
  // anything past ~3 lines spills scrollHeight beyond the visible
  // clientHeight. Re-runs on action, open, expand/collapse, and resize, since
  // wrapping (and therefore overflow) moves with the dialog width.
  $effect(() => {
    void open;
    void action?.id;
    void showFullCommand;
    if (!open || !action || showFullCommand) return;
    if (typeof window === "undefined") return;
    let cancelled = false;
    const measure = () => {
      if (cancelled || !commandEl || showFullCommand) return;
      commandOverflows = commandEl.scrollHeight > commandEl.clientHeight + 1;
    };
    const frame = requestAnimationFrame(measure);
    try {
      const fonts = (
        document as unknown as { fonts?: { ready: Promise<void> } }
      ).fonts;
      if (fonts?.ready) void fonts.ready.then(() => measure());
    } catch {
      // ignore — the rAF pass already ran
    }
    window.addEventListener("resize", measure);
    return () => {
      cancelled = true;
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", measure);
    };
  });

  async function copyCommand() {
    if (!action) return;
    const text = action.command;
    try {
      if (
        typeof navigator !== "undefined" &&
        navigator.clipboard?.writeText
      ) {
        await navigator.clipboard.writeText(text);
      } else if (typeof document !== "undefined") {
        const area = document.createElement("textarea");
        area.value = text;
        area.setAttribute("readonly", "");
        area.style.position = "absolute";
        area.style.left = "-9999px";
        document.body.appendChild(area);
        area.select();
        document.execCommand("copy");
        document.body.removeChild(area);
      } else {
        return;
      }
      copied = true;
      copyAnnouncement = "Command copied.";
      clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 1200);
    } catch (e) {
      console.error(e);
    }
  }
</script>

<!-- Row detail surface — one grammar per surface class (research 0006 pattern 13):
     the same centered Dialog the Products page uses, so click-a-row → details
     is learned once. Notes render read-only from markdown-lite; content past
     the existence glyph lives only here, not on compact surfaces
     (research 0004 rule 3, 0006 pattern 14).
     Note-first (research 0004 rule 2, 0006:13–14): the unbounded why leads in
     full; the unbounded command collapses to a 3-line scent behind
     Show-command, so the narrow dock never grows a mono wall. -->
<Dialog {open} title={action ? `About ${action.name}` : "More info"} onclose={onclose} width={480}>
  {#if action}
    {#if hasRenderedNote}
      <div class="note-block note-block--first">
        <p class="note-block__label">Note</p>
        <div class="note">{@html rendered}</div>
      </div>
    {:else}
      <p class="details__hint details__hint--first">
        {#if onedit}
          No note — edit the action to add one.
        {:else}
          No note.
        {/if}
      </p>
    {/if}

    <div class="cmd-block">
      <p class="cmd-block__label">Command</p>
      <pre
        id="qa-details-command"
        bind:this={commandEl}
        class="cmd-scent mono"
        class:cmd-scent--clamped={!showFullCommand}
      >{action.command}</pre>
      <div class="cmd-block__row">
        {#if showFullCommand || commandOverflows}
          <Button
            variant="ghost"
            onclick={() => (showFullCommand = !showFullCommand)}
            aria-expanded={showFullCommand}
            aria-controls="qa-details-command"
          >
            {showFullCommand ? "Hide" : "Show command"}
          </Button>
        {/if}
        <Button
          variant="secondary"
          onclick={() => void copyCommand()}
          aria-label={copied ? `Copied ${action.name} command` : `Copy ${action.name} command`}
        >
          <Icon name={copied ? "check" : "copy"} size={13} />
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <p class="sr-only" role="status" aria-live="polite">{copyAnnouncement}</p>
      {#if !hasRenderedNote && commandOverflows && !showFullCommand}
        <p class="details__hint">Find the full command in the main app.</p>
      {/if}
    </div>

    <dl class="details">
      <div class="details__row">
        <dt>Shell</dt>
        <dd class="mono">{quickActionShellLabel[action.shell ?? "powershell"]}</dd>
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

    <div class="details__actions">
      <Button variant="secondary" onclick={onclose}>Close</Button>
      {#if onedit}
        <Button variant="secondary" onclick={() => onedit(action)}>Edit</Button>
      {/if}
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
    margin: var(--space-4) 0 0;
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

  .details__hint--first {
    margin-top: 0;
  }

  /* Command scent: the level-1 fast read (research 0004 rule 2, 0006:13–14).
     Collapsed it holds ~3 lines with an ellipsis; expanded it scrolls inside
     the dialog's own scroll container, never past Close/Run. Token box only. */
  .cmd-block {
    margin-top: var(--space-4);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border);
  }

  .note-block--first + .cmd-block {
    margin-top: var(--space-4);
  }

  .details__hint--first + .cmd-block {
    margin-top: var(--space-4);
  }

  .cmd-block__label {
    margin: 0 0 var(--space-2);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .cmd-scent {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: var(--leading-body);
  }

  .cmd-scent--clamped {
    display: -webkit-box;
    line-clamp: 3;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .cmd-block__row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .note-block {
    margin-top: var(--space-4);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border);
  }

  .note-block--first {
    margin-top: 0;
    padding-top: 0;
    border-top: none;
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
