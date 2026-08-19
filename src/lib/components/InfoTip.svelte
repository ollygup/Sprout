<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  /** Progressive-disclosure info affordance (ticket 14): a quiet ⓘ button
   *  that opens a popover on click or keyboard (Enter/Space) — never
   *  hover-only. Escape and outside pointer-down close it and restore focus.
   *  The popover flips above the trigger when there is no room below inside
   *  the nearest clipping container (e.g. the dialog body's scroll area). */
  let {
    label,
    tone = "info",
    children,
  }: {
    label: string;
    tone?: "info" | "warn";
    children: Snippet;
  } = $props();

  const triggerId = `tip-trigger-${Math.random().toString(36).slice(2, 8)}`;
  const tipId = `tip-body-${Math.random().toString(36).slice(2, 8)}`;

  let wrapper: HTMLSpanElement | undefined = $state();
  let tip: HTMLDivElement | undefined = $state();
  let open = $state(false);
  let flipUp = $state(false);

  $effect(() => {
    if (!open) return;
    const raf = requestAnimationFrame(() => {
      if (!wrapper || !tip) return;
      const wr = wrapper.getBoundingClientRect();
      const tr = tip.getBoundingClientRect();
      let top = 8;
      let bottom = window.innerHeight - 8;
      let el = wrapper.parentElement;
      while (el) {
        const style = getComputedStyle(el);
        if (
          style.overflowY === "auto" ||
          style.overflowY === "scroll" ||
          style.overflowY === "hidden"
        ) {
          const r = el.getBoundingClientRect();
          top = r.top + 8;
          bottom = r.bottom - 8;
          break;
        }
        el = el.parentElement;
      }
      const below = bottom - wr.bottom - 10;
      const above = wr.top - top - 10;
      flipUp = tr.height > below && tr.height <= above;
    });
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    if (!open) return;
    // Capture phase: intercept Escape before the dialog's own handler can
    // close the whole dialog — the popover closes itself instead.
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        open = false;
        document.getElementById(triggerId)?.focus();
      }
    };
    const onPointerDown = (e: PointerEvent) => {
      if (wrapper?.contains(e.target as Node)) return;
      open = false;
    };
    window.addEventListener("keydown", onKeydown, true);
    window.addEventListener("pointerdown", onPointerDown);
    return () => {
      window.removeEventListener("keydown", onKeydown, true);
      window.removeEventListener("pointerdown", onPointerDown);
    };
  });
</script>

<span bind:this={wrapper} class="tip-wrap">
  <button
    id={triggerId}
    type="button"
    class="tip-btn"
    class:warn={tone === "warn"}
    class:open
    aria-label={label}
    aria-expanded={open}
    aria-controls={tipId}
    onclick={() => (open = !open)}
  >
    <Icon name="info" size={13} />
  </button>
  {#if open}
    <div
      bind:this={tip}
      id={tipId}
      class="tip"
      class:flip-up={flipUp}
      role="region"
      aria-labelledby={triggerId}
    >
      {@render children()}
    </div>
  {/if}
</span>

<style>
  .tip-wrap {
    position: relative;
    display: inline-flex;
    flex: none;
  }

  .tip-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--info-text);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .tip-btn:hover,
  .tip-btn:focus-visible,
  .tip-btn.open {
    background: var(--info-tint);
  }

  .tip-btn.warn {
    color: var(--warn-text);
  }

  .tip-btn.warn:hover,
  .tip-btn.warn:focus-visible,
  .tip-btn.warn.open {
    background: var(--warn-tint);
  }

  .tip {
    position: absolute;
    z-index: 30;
    top: calc(100% + 8px);
    left: 0;
    width: max-content;
    max-width: min(280px, calc(100vw - 24px));
    padding: var(--space-3);
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-dialog);
    font-size: var(--text-xs);
    line-height: var(--leading-body);
    color: var(--text-muted);
  }

  .tip.flip-up {
    top: auto;
    bottom: calc(100% + 8px);
  }

  .tip :global(p) {
    margin: 0;
  }

  .tip :global(.mono) {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
  }
</style>