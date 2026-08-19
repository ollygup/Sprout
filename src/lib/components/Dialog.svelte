<script lang="ts">
  import type { Snippet } from "svelte";
  import { fade } from "svelte/transition";
  import Icon from "./Icon.svelte";

  let {
    open,
    title,
    onclose,
    children,
    width = 460,
    focusTarget,
  }: {
    open: boolean;
    title: string;
    onclose: () => void;
    children: Snippet;
    width?: number;
    /** A selector for the element that receives focus on open (e.g. the
     *  dangerous button of a destructive confirm); defaults to the first
     *  focusable. */
    focusTarget?: string;
  } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let lastFocus: HTMLElement | null = null;
  const titleId = `dialog-title-${Math.random().toString(36).slice(2, 8)}`;

  $effect(() => {
    if (open && dialog) {
      lastFocus = document.activeElement as HTMLElement;
      if (!dialog.open) dialog.showModal();
      focusFirst();
      const t = setTimeout(() => focusFirst(), 30);
      return () => clearTimeout(t);
    }
    if (!open && dialog?.open) {
      dialog.close();
      lastFocus?.focus();
    }
  });

  function focusFirst() {
    if (!dialog) return;
    const f = focusTarget
      ? dialog.querySelector<HTMLElement>(focusTarget)
      : dialog.querySelector<HTMLElement>("input, select, button, [tabindex]");
    f?.focus();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
    if (event.key === "Tab" && dialog) {
      const focusables = Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        )
      ).filter((el) => !el.hasAttribute("disabled"));
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
  }

  let pointerDownInside = false;

  function onPointerDown(event: PointerEvent) {
    const rect = dialog?.getBoundingClientRect();
    if (!rect) return;
    pointerDownInside =
      event.clientX >= rect.left &&
      event.clientX <= rect.right &&
      event.clientY >= rect.top &&
      event.clientY <= rect.bottom;
  }

  function onBackdrop(event: MouseEvent) {
    const startedInside = pointerDownInside;
    pointerDownInside = false;
    const rect = dialog?.getBoundingClientRect();
    if (!rect) return;
    const clickedInside =
      event.clientX >= rect.left &&
      event.clientX <= rect.right &&
      event.clientY >= rect.top &&
      event.clientY <= rect.bottom;
    if (!clickedInside && !startedInside) onclose();
  }
</script>

{#if open}
  <dialog
    bind:this={dialog}
    class="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    style="width: {width}px; max-width: min(92vw, {width}px);"
    onkeydown={onKeydown}
    onpointerdown={onPointerDown}
    onclick={onBackdrop}
    transition:fade={{ duration: 140 }}
  >
    <header class="dialog__head">
      <h2 id={titleId} class="dialog__title">{title}</h2>
      <button class="icon-btn" onclick={onclose} aria-label="Close dialog">
        <Icon name="x" size={15} />
      </button>
    </header>
    <div class="dialog__body">
      {@render children()}
    </div>
  </dialog>
{/if}

<style>
  .dialog {
    border: none;
    border-radius: var(--radius);
    background: var(--bg-surface);
    color: var(--text);
    padding: 0;
    box-shadow: var(--shadow-dialog);
    overflow: hidden;
    overscroll-behavior: contain;
  }

  .dialog::backdrop {
    background: var(--scrim);
  }

  .dialog__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--border);
  }

  .dialog__title {
    font-family: var(--font-display);
    font-size: var(--text-xl);
    font-weight: 600;
    line-height: var(--leading-tight);
  }

  .dialog__body {
    padding: var(--space-5);
    max-height: 70vh;
    overflow-y: auto;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
</style>
