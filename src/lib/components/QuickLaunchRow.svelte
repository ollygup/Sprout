<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    mainLabel,
    onmain,
    disabled = false,
    tipId,
    tipName,
    tipBody,
    children,
    trailing,
  }: {
    /** The row's main verb, collection-owned and never homogenized —
     *  "Start X" / "Copy X…" / "About X" (research 0005:4: users should not
     *  wonder whether different words mean the same thing). */
    mainLabel: string;
    /** What the main click does — Start / Copy / Details per collection.
     *  The shell never learns which; one click, one verb, no nesting. */
    onmain: () => void;
    /** While set the main button waits (Launch single-flight) — the card
     *  keeps its geometry and drops the hover tint, exactly like before. */
    disabled?: boolean;
    /** Id of the rendered tip, mirrored to the button's `aria-describedby`
     *  so keyboard and screen-reader users get the same content as hover
     *  (research 0004:4, WIG). Omitted with the tip (Launch today). */
    tipId?: string;
    /** Tooltip content — always bold name + one truncated mono line (ticket
     *  93's contract), rendered by the shell so anchoring never drifts. */
    tipName?: string;
    tipBody?: string;
    /** Main-button content: badge + name + meta, collection-owned and styled
     *  at the call site (badge tones, note glyph and excerpts differ per
     *  collection and stay there per research 0006:14). */
    children: Snippet;
    /** Fixed right slot (Run/Stop, Starting… is main-side) — its presence
     *  switches split layout; absent, the main button is full-bleed. No
     *  boolean prop: structure follows content. */
    trailing?: Snippet;
  } = $props();
</script>

<!-- Ticket 134: the one home for dock/window row geometry (research 0005:5,
     0006:13 — the ticket-71 PacketCard shape: shell renders structure,
     variants slot content). Launch/Actions/Clips are thin adapters; deleting
     this re-scatters card CSS + tip anchoring + split/full rules across three
     snippets that must stay pixel-identical at three densities. -->
<li class="dock-row" class:dock-row--disabled={disabled}>
  <button
    type="button"
    class="dock-row__main"
    aria-label={mainLabel}
    aria-describedby={tipId}
    {disabled}
    onclick={onmain}
  >
    {@render children()}
  </button>
  {#if trailing}
    {@render trailing()}
  {/if}
  {#if tipName !== undefined}
    <span class="dock-row__tip" id={tipId}>
      <span class="dock-row__tip-name">{tipName}</span>
      <span class="dock-row__tip-body">{tipBody}</span>
    </span>
  {/if}
</li>

<style>
  /* One card box for all three tabs — the Launch/Clip tokens, shared so a
     density step rescales every tab together (research 0004:4). Content-
     driven height (no floor): the text side sets it, the trailing control
     fills it. No overflow clip — the below-anchored tip must escape. */
  .dock-row {
    position: relative;
    display: flex;
    align-items: stretch;
    padding: 0;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    transition: border-color var(--dur-fast) var(--ease-out);
  }

  .dock-row:not(.dock-row--disabled):hover,
  .dock-row:focus-within {
    border-color: var(--accent-tint-border);
  }

  /* The row's verb — a real <button>, keyboard/touch/screen-reader operable
     with no hover dependency (WIG); the card tints exactly like hovering via
     :focus-within, and the ring sits inside (-2px) so the card border never
     clips it. flex:1 + min-w-0 absorbs every dock width (WIG content
     handling); the trailing slot never shrinks. */
  .dock-row__main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    background: transparent;
    border: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .dock-row__main:focus {
    outline: none;
  }

  .dock-row__main:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .dock-row__main:disabled {
    cursor: default;
  }

  /* Ticket 93: hover/focus tooltip — bold name plus the row's content on one
     truncated line, anchored BELOW so the scrollport never clips it at the
     top. Hidden with opacity only — never display/visibility — so
     `aria-describedby` still exposes it to assistive tech, and
     `:focus-within` raises exactly what hovering does. */
  .dock-row__tip {
    position: absolute;
    z-index: 30;
    top: calc(100% + var(--space-2));
    left: 0;
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-dialog);
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-fast) var(--ease-out);
  }

  .dock-row:hover .dock-row__tip,
  .dock-row:focus-within .dock-row__tip {
    opacity: 1;
  }

  .dock-row__tip-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--qlw-meta);
    font-weight: 600;
    color: var(--text);
  }

  .dock-row__tip-body {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--qlw-micro);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }
</style>
