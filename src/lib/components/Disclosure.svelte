<script lang="ts">
  import Icon from "./Icon.svelte";

  /** Shared disclosure trigger (ticket 45): a caret button that toggles an
   *  associated panel. Icon-only (26×26 square, e.g. preset composer rows) or
   *  with a label (e.g. the product form's Advanced section). The filled
   *  triangle points right when closed and rotates 90° to point down when
   *  open — the <details>/Notion tree convention — via a transform-only
   *  transition; the global prefers-reduced-motion rule collapses it to
   *  nothing. */
  let {
    open,
    controls,
    label,
    ariaLabel,
    onclick,
  }: {
    open: boolean;
    controls: string;
    label?: string;
    ariaLabel?: string;
    onclick: () => void;
  } = $props();
</script>

<button
  type="button"
  class="disclosure"
  class:open
  class:disclosure--labeled={!!label}
  aria-expanded={open}
  aria-controls={controls}
  aria-label={ariaLabel}
  onclick={onclick}
>
  <span class="disclosure__chevron" aria-hidden="true">
    <Icon name="caret" size={14} />
  </span>
  {#if label}
    <span class="disclosure__label">{label}</span>
  {/if}
</button>

<style>
  .disclosure {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    width: 26px;
    height: 26px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0;
  }

  .disclosure:hover,
  .disclosure:focus-visible {
    background: var(--bg-hover);
    color: var(--text);
  }

  .disclosure :global(svg) {
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .disclosure.open :global(svg) {
    transform: rotate(90deg);
  }

  /* Labeled mode: a flat full-width section-header row (e.g. the product
     form's Advanced) — deliberately no pill, no background, so the header
     reads like the field labels above it, not like a button. */
  .disclosure--labeled {
    width: 100%;
    height: 26px;
    justify-content: flex-start;
    border-radius: 0;
    padding: 0;
    gap: var(--space-1);
    color: var(--text-muted);
  }

  .disclosure--labeled:hover,
  .disclosure--labeled:focus-visible {
    background: transparent;
    color: var(--text-muted);
  }

  .disclosure__label {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .disclosure--labeled:hover .disclosure__label,
  .disclosure--labeled:focus-visible .disclosure__label {
    color: var(--accent);
  }
</style>
