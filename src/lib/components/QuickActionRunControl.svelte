<script lang="ts">
  import { onMount, tick } from "svelte";
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
    compact = false,
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
    /** Compact icon-only mode (ticket 130, converged in 134): the Quick
     *  Launch window/dock rows render `[flex text | fixed full-height icon
     *  Run/Stop]` — one content-driven height per density across all three
     *  tabs — while the roomy main-app page keeps icon+text. Fixed width by
     *  construction (icon + the shared Button padding, no text to measure),
     *  so Run→Stop→Stopping never reflows; the 44px width floor keeps the
     *  AAA target on the horizontal axis (research 0004:4, NN/g target size). */
    compact?: boolean;
  } = $props();

  // Ticket 124: Run→Stop must never change button width; color carries the meaning.
  // Measure the longest label once on mount and set --run-w on the control root;
  // every Button then enforces min-width:var(--run-w) — the only visible change
  // is primary(accent)→danger and the disabled Stopping treatment (research 0006:6,
  // 0005:2 one primary per row). No new Button variant; tokens only.
  let root: HTMLSpanElement | undefined = $state(undefined);
  let runW: number | null = $state(null);

  onMount(async () => {
    // Ticket 130's compact mode is fixed-width by construction (icon-only +
    // the shared Button padding) — there is nothing to measure and no
    // --run-w to set; every state is the same outer box.
    if (compact) return;
    await tick();
    if (typeof document === "undefined") return;

    const doMeasure = () => {
      // Detached probes so measurement is not clipped by display:none ancestors
      // (the Quick Launch Actions tab is hidden behind Launch on first open).
      const container = document.createElement("div");
      container.setAttribute("aria-hidden", "true");
      container.style.position = "absolute";
      container.style.visibility = "hidden";
      container.style.pointerEvents = "none";
      container.style.left = "-9999px";
      container.style.top = "0";
      container.style.display = "flex";
      container.style.gap = "8px";
      document.body.appendChild(container);

      const applyBtnStyle = (el: HTMLElement) => {
        el.style.display = "inline-flex";
        el.style.alignItems = "center";
        el.style.justifyContent = "center";
        el.style.gap = "var(--space-2)";
        el.style.padding = "8px 16px";
        el.style.border = "1px solid transparent";
        el.style.borderRadius = "var(--radius)";
        el.style.fontFamily = "var(--font-body)";
        el.style.fontSize = "var(--text-base)";
        el.style.fontWeight = "600";
        el.style.lineHeight = "1.2";
        el.style.whiteSpace = "nowrap";
        el.style.boxSizing = "border-box";
      };

      const makeProbe = (label: string, isStopping = false) => {
        const btn = document.createElement("span");
        applyBtnStyle(btn);
        if (isStopping) btn.style.borderColor = "var(--border-strong)";
        const icon = document.createElement("span");
        icon.style.flexShrink = "0";
        icon.style.display = "inline-block";
        if (isStopping) {
          icon.style.width = "11px";
          icon.style.height = "11px";
          icon.style.borderRadius = "50%";
          icon.style.border = "2px solid var(--border-strong)";
          icon.style.borderTopColor = "var(--text-muted)";
          icon.style.boxSizing = "border-box";
        } else {
          icon.style.width = "13px";
          icon.style.height = "13px";
        }
        btn.appendChild(icon);
        btn.appendChild(document.createTextNode(label));
        container.appendChild(btn);
        return btn;
      };

      const runEl = makeProbe("Run");
      const stopEl = makeProbe("Stop");
      const stoppingEl = makeProbe("Stopping…", true);

      const widths = [runEl, stopEl, stoppingEl].map((el) =>
        Math.ceil(el.getBoundingClientRect().width),
      );
      const max = Math.max(...widths);
      document.body.removeChild(container);
      if (max > 0 && Number.isFinite(max)) {
        runW = max;
        if (root) root.style.setProperty("--run-w", `${max}px`);
      }
    };

    try {
      const fonts = (document as unknown as { fonts?: { ready: Promise<void> } }).fonts;
      if (fonts?.ready) await fonts.ready;
    } catch {
      // ignore — measurement will still run
    }
    requestAnimationFrame(() => {
      doMeasure();
      // second pass after a tick — catches delayed font/layout
      setTimeout(doMeasure, 150);
    });
  });

  $effect(() => {
    if (root && runW) root.style.setProperty("--run-w", `${runW}px`);
  });
</script>

<span
  bind:this={root}
  class="qa-run"
  class:qa-run--compact={compact}
  style={runW && !compact ? `--run-w:${runW}px` : undefined}
>
  {#if compact}
    <!-- Ticket 130's dock/window grammar: icon-only, fixed-width,
         full-card-height Run/Stop. The text side of the row owns the
         details dialog (research 0006:13 one grammar per surface); this
         button only runs/stops. `title` mirrors `aria-label` so pointer,
         keyboard, touch and screen-reader users get the same verb + name
         with no hover dependency (research 0004:4, WIG). -->
    {#if stopping}
      <Button
        variant="secondary"
        disabled
        aria-label={`Stopping ${name}`}
        title={`Stopping ${name}`}
        aria-describedby={describedby}
      >
        <span class="spin spin--compact" aria-hidden="true"></span>
      </Button>
    {:else if stoppable && running}
      <Button
        variant="danger"
        onclick={onstop}
        aria-label={`Stop ${name}`}
        title={`Stop ${name}`}
        aria-describedby={describedby}
      >
        <Icon name="stop" size={15} />
      </Button>
    {:else}
      <Button
        variant="primary"
        onclick={onrun}
        aria-label={`Run ${name}`}
        title={`Run ${name}`}
        aria-describedby={describedby}
      >
        <Icon name="play" size={15} />
      </Button>
    {/if}
  {:else if stopping}
    <!-- Ticket 92's contract, owned once since ticket 98: Stop in flight —
         disabled and muted until the exit event lands; the spinner is the
         honest "something is happening" (research 0004 rule 5). -->
    <Button
      variant="secondary"
      disabled
      aria-label={`Stopping ${name}`}
      aria-describedby={describedby}
      style="min-width:var(--run-w)"
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
      style="min-width:var(--run-w)"
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
      style="min-width:var(--run-w)"
    >
      <Icon name="play" size={13} />
      Run
    </Button>
  {/if}
</span>

<style>
  .qa-run {
    display: inline-flex;
    /* Fallback before JS measures — widest of Run/Stop/Stopping… at the
       Button's typography (14px/600, 8px gap, 8px 16px padding) so even the
       first paint and the Stop→Stopping flip never reflow at 340px. JS
       refines it per locale once on mount. */
    --run-w: 118px;
  }

  /* Ticket 130's compact box, converged in ticket 134: the icon-only Button
     keeps its own padding (8px 16px) and border, so the outer box is fixed
     by construction — ~49px wide (16 + 15 icon + 16 + 2 border) at every
     state, never reflowed by a label change and never squeezing under text
     pressure. It fills the row height (`height: 100%` — the row is
     content-driven like the Launch/Clip cards, so every tab shares one
     height per density); the 44px width floor keeps the AAA target on the
     horizontal axis (research 0004:4, NN/g target size). */
  .qa-run--compact {
    display: flex;
    align-self: stretch;
    flex-shrink: 0;
  }

  .qa-run--compact :global(.btn) {
    flex: 1;
    min-width: 44px;
    height: 100%;
    padding: 8px 16px;
    border-radius: 0 var(--radius) var(--radius) 0;
  }

  .qa-run--compact :global(.btn:focus-visible) {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  /* The compact spinner rides the same 15px icon box as play/stop so the
     Stopping flip changes nothing outside the glyph (research 0004:5 —
     the spinner alone carries the feedback, no label reflow). Border-box
     keeps the 2px ring inside the 15px, exactly the icon's outer box.
     Doubled class beats the base `.spin` width below regardless of order. */
  .spin.spin--compact {
    width: 15px;
    height: 15px;
  }

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
