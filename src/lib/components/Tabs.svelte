<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$lib/components/Icon.svelte";

  /** One tab in the strip (ticket 52): an id the panel snippet keys on and
   *  the button's visible label. `shortLabel` and `icon` back the strip's
   *  degradation chain; `title` names the tab for hover tooltips and as its
   *  accessible name — what assistive tech reads once text disappears. */
  interface TabDef {
    id: string;
    label: string;
    shortLabel?: string;
    icon?: string;
    title?: string;
  }

  /** How far down the chain the strip currently sits (research 0004 rule
   *  4): full label → shortened label → icon-only. */
  type LabelFit = "full" | "short" | "icon";

  /** Shared accessible tab strip (ticket 52, added to the component
   *  foundation per the AGENTS.md design rule): a WAI-ARIA tablist with
   *  roving tabindex — the selected tab keeps `tabindex=0`, the rest are
   *  skipped, and ←/→/Home/End move the selection and focus. All panels
   *  stay mounted (hidden, not removed) so their `aria-labelledby` targets
   *  always exist and tab state survives panel toggles.
   *
   *  Label fitting is measured at runtime, never assumed (research 0004
   *  rule 4): physical-px windows render narrower at high DPI, so the
   *  strip picks full → shortened → icon-only by comparing measured label
   *  widths against its container — one pass, decided off-DOM via canvas
   *  text metrics, then applied. The v1 of this feature rendered each
   *  candidate level and watched the tablist for overflow; that observer
   *  saw the strip's own degradation resize it, reset to full labels, and
   *  loop until Svelte's update-depth guard aborted the flush — freezing
   *  the whole window mid-paint. Never put the thing you resize inside
   *  your own feedback path: the observer watches the parent box (which
   *  label lengths cannot move), and no effect writes state another part
   *  of the same flush reads. */
  let {
    tabs,
    selected,
    onselect,
    ariaLabel,
    panel,
  }: {
    tabs: TabDef[];
    selected: string;
    onselect: (id: string) => void;
    ariaLabel: string;
    /** Receives the tab id of the panel being rendered. */
    panel: Snippet<[string]>;
  } = $props();

  /** Icon side length the template renders for icon-only tabs — kept in
   *  step with the `<Icon>` size below. */
  const ICON_FIT_SIZE = 14;

  let fit = $state<LabelFit>("full");
  // Plain fields on purpose: nothing reactive may read them during a
  // flush, or the fitting decision becomes part of the graph it steers.
  let listEl: HTMLElement | null = null;
  let observer: ResizeObserver | null = null;
  let raf = 0;
  let canvasCtx: CanvasRenderingContext2D | null | undefined;

  /** The level the current tabs need, given the tablist's real content
   *  box: canvas-measured text widths (the actual font) plus paddings and
   *  gaps vs the parent-driven width. One pass, no intermediate renders. */
  function computeFit(): LabelFit {
    const el = listEl;
    if (!el || !el.isConnected) return "full";
    const ctx = (canvasCtx ??= document.createElement("canvas").getContext("2d"));
    if (!ctx) return "full";
    const list = getComputedStyle(el);
    const available =
      el.clientWidth -
      (parseFloat(list.paddingLeft) || 0) -
      (parseFloat(list.paddingRight) || 0);
    const firstButton = el.querySelector<HTMLElement>(".tabs__tab");
    if (available <= 0 || !firstButton) return "full";
    const btn = getComputedStyle(firstButton);
    ctx.font = `${btn.fontWeight} ${btn.fontSize} ${btn.fontFamily}`;
    const gap = parseFloat(list.columnGap) || 0;
    const padX = (parseFloat(btn.paddingLeft) || 0) + (parseFloat(btn.paddingRight) || 0);
    const textW = (s: string) => ctx.measureText(s).width + padX;
    const widthAt = (t: TabDef, level: LabelFit): number => {
      if (level === "icon") return t.icon ? ICON_FIT_SIZE + padX : textW(t.shortLabel ?? t.label);
      if (level === "short") return textW(t.shortLabel ?? t.label);
      return textW(t.label);
    };
    const totalAt = (level: LabelFit) =>
      tabs.reduce((sum, t) => sum + widthAt(t, level), 0) +
      gap * Math.max(0, tabs.length - 1);
    if (totalAt("full") <= available) return "full";
    if (totalAt("short") <= available) return "short";
    // Icons replace names outright — every tab must keep one, and a name
    // for tooltips and assistive tech, before text may disappear.
    return tabs.every((t) => t.icon && t.title) ? "icon" : "short";
  }

  /** Applies the fit decision on the next frame, outside any flush. */
  function scheduleFit() {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => {
      fit = computeFit();
    });
  }

  function watchStrip(node: HTMLElement) {
    listEl = node;
    // The parent's width moves with the window/DPI only — never with what
    // the strip renders into it, so watching it cannot feed back.
    observer = new ResizeObserver(scheduleFit);
    if (node.parentElement) observer.observe(node.parentElement);
    // Webfonts swapping in after first paint change label widths.
    document.fonts.ready.then(scheduleFit);
    return {
      destroy() {
        observer?.disconnect();
        observer = null;
        cancelAnimationFrame(raf);
        listEl = null;
      },
    };
  }

  // A different tab set gets a fresh fit — a tab appearing or leaving can
  // be exactly the difference between fitting and overflowing. Reads only;
  // the write lands on the next frame via scheduleFit.
  $effect(() => {
    void tabs
      .map((t) => `${t.id}|${t.label}|${t.shortLabel ?? ""}|${t.icon ?? ""}|${t.title ?? ""}`)
      .join("\n");
    scheduleFit();
  });

  function keydown(event: KeyboardEvent) {
    const index = tabs.findIndex((tab) => tab.id === selected);
    let next: number | null = null;
    if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
    else if (event.key === "ArrowLeft") next = (index - 1 + tabs.length) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    if (next === null) return;
    event.preventDefault();
    const id = tabs[next].id;
    onselect(id);
    requestAnimationFrame(() => document.getElementById(`tab-${id}`)?.focus());
  }
</script>

<div class="tabs">
  <div
    role="tablist"
    aria-label={ariaLabel}
    class="tabs__list"
    tabindex="-1"
    use:watchStrip
    onkeydown={keydown}
  >
    {#each tabs as tab (tab.id)}
      <button
        id={`tab-${tab.id}`}
        type="button"
        role="tab"
        aria-selected={tab.id === selected}
        aria-controls={`panel-${tab.id}`}
        aria-label={tab.title}
        tabindex={tab.id === selected ? 0 : -1}
        class="tabs__tab"
        class:active={tab.id === selected}
        class:tabs__tab--icon={fit === "icon" && tab.icon !== undefined}
        title={tab.title}
        onclick={() => onselect(tab.id)}
      >
        {#if fit === "icon" && tab.icon}
          <Icon name={tab.icon} size={ICON_FIT_SIZE} />
        {:else if fit !== "full" && tab.shortLabel}
          {tab.shortLabel}
        {:else}
          {tab.label}
        {/if}
      </button>
    {/each}
  </div>
  {#each tabs as tab (tab.id)}
    <div
      id={`panel-${tab.id}`}
      role="tabpanel"
      aria-labelledby={`tab-${tab.id}`}
      class="tabs__panel"
      hidden={tab.id !== selected}
    >
      {@render panel(tab.id)}
    </div>
  {/each}
</div>

<style>
  .tabs__list {
    display: flex;
    gap: var(--space-1);
    padding: 0 var(--space-3);
    border-bottom: 1px solid var(--border);
  }

  .tabs__tab {
    appearance: none;
    margin: 0 0 -1px;
    padding: var(--space-3) var(--space-2);
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    font-family: var(--font-body);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-muted);
    cursor: pointer;
    /* Labels never wrap: wrapping would absorb the overflow the fitting
       measurement has to see (research 0004 rule 4). */
    white-space: nowrap;
    transition: color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .tabs__tab:hover {
    color: var(--text);
  }

  .tabs__tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .tabs__tab--icon {
    display: inline-flex;
    align-items: center;
    padding-inline: var(--space-3);
  }

  .tabs__tab:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .tabs__panel:focus {
    outline: none;
  }

  .tabs__panel[hidden] {
    display: none;
  }
</style>