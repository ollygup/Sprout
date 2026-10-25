<script lang="ts">
  import Icon from "./Icon.svelte";

  /** The six-dot drag handle (ticket 232). Only the grip starts a drag, so
   *  the rest of the card or group header stays clickable. It is a real
   *  button with an accessible name and instructions: Enter/Space opens the
   *  same ⋯ menu the row already has (Move up/down, Move to group, Move
   *  into…), so drag is an accelerator and never the only path.
   *
   *  The grip sits in the host's leading gutter, absolutely positioned, so
   *  it never shifts the row when it fades in. Its full-height strip counts
   *  as part of the host: because the button is a descendant, hovering the
   *  strip keeps the host's `:hover` alive, so the card stays lit and the
   *  grip stays visible all the way from the card to the handle. Hosts only
   *  set `position: relative` and publish `--grip-opacity`; coarse pointers
   *  always show it. */
  let {
    label,
    disabled = false,
    onstart,
    onmenu,
  }: {
    label: string;
    disabled?: boolean;
    onstart: (event: PointerEvent) => void;
    onmenu?: (event: MouseEvent) => void;
  } = $props();
</script>

<button
  type="button"
  class="grip"
  aria-label={label}
  title={label}
  {disabled}
  onpointerdown={(event) => onstart(event)}
  onclick={(event) => {
    event.stopPropagation();
    // Pointer drags end in a click too; only a keyboard-generated click
    // (detail 0) opens the menu.
    if (event.detail === 0) onmenu?.(event);
  }}
  onkeydown={(event) => event.stopPropagation()}
>
  <Icon name="grip" size={14} />
</button>

<style>
  /* The whole gutter strip beside the host is the target, not just the
     14 px glyph: a full-height 24 px button is easy to hit (ticket 232).
     The strip is the host's own hover area too — hovering it keeps the
     card lit — so the handle needs no highlight of its own; the card's
     highlight plus the grab cursor is the whole affordance. */
  .grip {
    position: absolute;
    left: -24px;
    top: 0;
    bottom: 0;
    z-index: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: auto;
    padding: 0;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-faint);
    cursor: grab;
    opacity: var(--grip-opacity, 0);
    /* Always hit-testable, even while invisible: the strip has to keep the
       host hovered before the grip has faded in. A press here starts a drag
       (4 px threshold), so a plain click in the strip does nothing. Pen and
       touch must drag, not scroll the page. */
    touch-action: none;
    transition: opacity 120ms ease, color 120ms ease;
  }

  .grip:hover {
    color: var(--text-muted);
  }

  .grip:active {
    cursor: grabbing;
  }

  .grip:focus-visible {
    opacity: 1;
    outline: 2px solid var(--ring);
    outline-offset: 1px;
  }

  .grip:disabled {
    cursor: default;
    opacity: 0;
  }

  /* No hover on pen/touch surfaces — the grip must be discoverable. */
  @media (hover: none) {
    .grip {
      opacity: 1;
      pointer-events: auto;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .grip {
      transition: none;
    }
  }
</style>
