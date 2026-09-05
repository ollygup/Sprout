<script lang="ts">
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";

  /** One opt-in feature behind the page-features menu: what it is called,
   *  what enabling it does, and the switch itself. `onchange` receives the
   *  requested next state; persistence and rollback stay with the page. */
  export interface FeatureItem {
    label: string;
    description: string;
    value: boolean;
    onchange: (next: boolean) => void;
  }

  let {
    label,
    items = [],
  }: {
    /** The trigger's accessible name — "…'s features". */
    label: string;
    items?: FeatureItem[];
  } = $props();

  const baseId = `features-${Math.random().toString(36).slice(2, 8)}`;
  const panelId = `${baseId}-panel`;
  const triggerId = `${baseId}-trigger`;

  let open = $state(false);
  let wrapper: HTMLSpanElement | undefined = $state();
  let panel: HTMLDivElement | undefined = $state();
  let flipUp = $state(false);

  // A feature list with nothing on it must not occupy chrome (research 0004
  // rule 2) — pages pass [] when no feature applies (e.g. below the
  // virtual-desktop gate) and the whole control disappears.
  const hasFeatures = $derived(items.length > 0);

  $effect(() => {
    if (!open || !wrapper || !panel) return;
    // Same vertical flip probe as InfoTip: when the clipped container has no
    // room below, the panel opens upward instead.
    const raf = requestAnimationFrame(() => {
      if (!wrapper || !panel) return;
      const wr = wrapper.getBoundingClientRect();
      const pr = panel.getBoundingClientRect();
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
      flipUp = pr.height > below && pr.height <= above;
    });
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    if (!open) return;
    // Capture phase so Escape closes this menu before a dialog-level handler
    // can react to it; outside pointer-down closes without stealing the
    // click. Focus returns to the gear either way.
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        close();
      }
    };
    const onPointerDown = (e: PointerEvent) => {
      if (wrapper?.contains(e.target as Node)) return;
      close();
    };
    window.addEventListener("keydown", onKeydown, true);
    window.addEventListener("pointerdown", onPointerDown);
    return () => {
      window.removeEventListener("keydown", onKeydown, true);
      window.removeEventListener("pointerdown", onPointerDown);
    };
  });

  function close() {
    open = false;
    document.getElementById(triggerId)?.focus();
  }
</script>

{#if hasFeatures}
  <span bind:this={wrapper} class="features">
    <IconButton
      id={triggerId}
      icon="gear"
      {label}
      quiet
      aria-expanded={open}
      aria-controls={panelId}
      onclick={() => (open = !open)}
    />
    {#if open}
      <div
        bind:this={panel}
        id={panelId}
        class="features__panel"
        class:flip-up={flipUp}
        role="group"
        aria-label={label}
      >
        {#each items as item, i (item.label)}
          {#if i > 0}
            <div class="features__sep"></div>
          {/if}
          <!-- The row IS the switch (guidelines: one hit target, no dead
               zones): name from the label, description read after it, state
               from aria-checked plus the visible On/Off word. -->
          <button
            type="button"
            role="switch"
            aria-checked={item.value}
            aria-labelledby={`${baseId}-label-${i}`}
            aria-describedby={`${baseId}-desc-${i}`}
            class="feature"
            onclick={() => item.onchange(!item.value)}
          >
            <span class="feature__text">
              <span id={`${baseId}-label-${i}`} class="feature__label">
                {item.label}
              </span>
              <span id={`${baseId}-desc-${i}`} class="feature__desc">
                {item.description}
              </span>
            </span>
            <span class="feature__control" aria-hidden="true">
              <span
                class="feature__word"
                class:feature__word--on={item.value}
              >
                {item.value ? "On" : "Off"}
              </span>
              <span class="feature__track" class:on={item.value}>
                <span class="feature__knob"></span>
              </span>
            </span>
            </button>
        {/each}
      </div>
    {/if}
  </span>
{/if}

<style>
  .features {
    position: relative;
    display: inline-flex;
    flex: none;
  }

  .features__panel {
    position: absolute;
    z-index: 30;
    top: calc(100% + 8px);
    right: 0;
    display: flex;
    flex-direction: column;
    width: max-content;
    min-width: 260px;
    max-width: min(320px, calc(100vw - 24px));
    padding: var(--space-1);
    background: var(--bg-surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-dialog);
  }

  .features__panel.flip-up {
    top: auto;
    bottom: calc(100% + 8px);
  }

  .features__sep {
    height: 1px;
    margin: var(--space-1) var(--space-2);
    background: var(--border);
  }

  .feature {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out);
  }

  .feature:hover {
    background: var(--bg-hover);
  }

  .feature:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .feature__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .feature__label {
    font-family: var(--font-display);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  .feature__desc {
    font-size: var(--text-xs);
    line-height: var(--leading-body);
    color: var(--text-muted);
  }

  .feature__control {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }

  .feature__word {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .feature__word--on {
    color: var(--accent);
  }

  /* Transform-only knob slide; every transition names its properties. */
  .feature__track {
    position: relative;
    display: inline-block;
    width: 34px;
    height: 20px;
    border-radius: var(--radius-pill);
    background: var(--bg-sunken);
    border: 1px solid var(--border-strong);
    transition: background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .feature__track.on {
    background: var(--accent);
    border-color: var(--accent);
  }

  .feature__knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: var(--radius-pill);
    background: var(--bg-page);
    box-shadow: var(--shadow-card);
    transform: translateX(0);
    transition: transform var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out);
  }

  .feature__track.on .feature__knob {
    background: var(--on-accent);
    transform: translateX(14px);
  }
</style>
