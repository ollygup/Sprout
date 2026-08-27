<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type {
    Clip,
    Group,
    LaunchEntry,
    LaunchReport,
    QuickAction,
  } from "$lib/types";
  import {
    copyClip,
    getQuickLaunchDockState,
    getSettings,
    listClips,
    listGroups,
    listLaunchEntries,
    listQuickActions,
    runQuickAction,
    startLaunchEntry,
    startQuickLaunch,
    switchQuickLaunchDockEdge,
    toggleQuickLaunchDock,
  } from "$lib/api";
  import {
    quickActionRuns,
    stopActionRun,
    syncQuickActionRuns,
  } from "$lib/quickActionRuns.svelte";
  import QuickActionRunControl from "$lib/components/QuickActionRunControl.svelte";
  import { clipTitle, launchReportSummary } from "$lib/format";
  import { appIcons, lazyIcon } from "$lib/lazyIcon.svelte";
  import { createGroupCollapse } from "$lib/groupCollapse.svelte";
  import type { QuickLaunchDockState } from "$lib/types";
  import { restoreTheme, type ThemeMode } from "$lib/theme.svelte";
  import { titleBarDragRegion } from "$lib/quickLaunchTitleBar";
  import Button from "$lib/components/Button.svelte";
  import GroupAccordion from "$lib/components/GroupAccordion.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import SproutMark from "$lib/components/SproutMark.svelte";
  import Tabs from "$lib/components/Tabs.svelte";

  // The Quick Launch window (ticket 52): the tray's left-click target — a
  // miniature, frameless, read-only window. The backend owns its life cycle
  // (ticket 56: blur does nothing; the × button / Alt+F4 destroy it and the
  // tray reopens it at a fixed centered size — no geometry is remembered);
  // this page only renders and fires the existing runners.
  // Docking (ticket 53) is controlled from this header: the toggle pins the
  // window to the current monitor's remembered (or Settings-default) edge as
  // a Win32 AppBar, and the arrows move it left↔right while docked.
  // Quick Clips (ticket 79) joins as a third tab only while at least one
  // clip exists — an empty feature must not occupy chrome (research 0004
  // rule 2); the main app's /clips page is its discoverability home and its
  // only editing surface.
  // Ticket 93: the Launch tab lists its entries and every row starts just
  // that entry; Start all stays pinned above the list. The list mirrors the
  // main page's Groups toggle (flat when off; ungrouped-first plus
  // default-expanded accordions with count badges when on) — the window
  // itself has no configuration surface (CONTEXT: all configuration happens
  // in the main app).

  let entries = $state<LaunchEntry[]>([]);
  let actions = $state<QuickAction[]>([]);
  let clips = $state<Clip[]>([]);
  // Ticket 93: every tab mirrors its collection's Groups feature live from
  // Settings (`launch_groups` / `action_groups` / `clip_groups`), plus each
  // collection's groups in user order — the window has no configuration
  // surface (CONTEXT: all configuration happens in the main app).
  let launchGroupsOn = $state(false);
  let actionGroupsOn = $state(false);
  let clipGroupsOn = $state(false);
  let launchGroups = $state<Group[]>([]);
  let actionGroups = $state<Group[]>([]);
  let clipGroups = $state<Group[]>([]);
  const launchCollapse = createGroupCollapse();
  const actionCollapse = createGroupCollapse();
  const clipCollapse = createGroupCollapse();
  let loading = $state(true);
  let launching = $state(false);
  // Ticket 93: the entries with a single-entry start in flight — set on
  // click, cleared only by `launch-run-done` (research 0004 rule 5: silence
  // reads as breakage). The backend's runs are single-flight, so while
  // anything is in flight every start affordance waits.
  let startingEntries = $state<Set<number>>(new Set());
  // Ticket 93: the finished run's summary line — the same wording the system
  // notification and the main page's flash carry, auto-cleared like them.
  let runNotice = $state("");
  let runNoticeTimer: ReturnType<typeof setTimeout> | undefined;
  let error = $state("");
  let tab = $state("launch");
  const SEAM_REASON = "Borders another display — cursor can't stop there";

  // Ticket 59: the dock state is never null — while the window floats it
  // carries the target edge/mode the toggle would dock to (`docked: false`),
  // so the toggle's icon tells the truth before the first dock. Ticket 63:
  // `blocked` carries the shell's auto-hide refusal while docked — transient,
  // only ever set by the backend.
  let dock = $state<QuickLaunchDockState>({
    edge: "left",
    mode: "auto-hide",
    docked: false,
    blocked: null,
    left_eligible: true,
    right_eligible: true,
  });
  // Ticket 119 Study A: already-docked middle line (seam) reuses the blocked
  // banner — same wall rule and same reason line as Settings.
  const seamBlocked = $derived(
    dock.docked &&
      ((dock.edge === "left" && !dock.left_eligible) ||
        (dock.edge === "right" && !dock.right_eligible))
  );
  const showBlocked = $derived(dock.docked && (dock.blocked !== null || seamBlocked));
  // Ticket 79: one-click copy feedback — the copied row flashes "Copied"
  // for ~1.2 s and a polite live region announces it; silence reads as
  // breakage (research 0004 rule 5).
  let copiedId = $state<number | null>(null);
  let copiedAnnouncement = $state("");
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    load();
    refreshDock();
    // Ticket 42: the run finishes on the backend's background thread — the
    // summary lands as a system notification, this event releases the start
    // affordances (Start all plus ticket 93's entry rows) and posts the
    // summary line.
    // Ticket 57: the backend emits `quick-launch-changed` after every command
    // that mutates what this window renders — Launch entry mutations, Quick
    // Action mutations, Clip mutations, `update_settings`, `update_theme`.
    // The window listens once and re-runs its loads plus its dock-state
    // refresh, so entries/actions/clips/settings changed in the main app —
    // including the theme and the Groups toggle — appear without reopening
    // it.
    const unlisteners: (() => void)[] = [];
    listen<LaunchReport>("launch-run-done", (event) => {
      launching = false;
      startingEntries = new Set();
      flashRun(launchReportSummary(event.payload));
    }).then((fn) => unlisteners.push(fn));
    listen("quick-launch-changed", () => {
      load();
      refreshDock();
    }).then((fn) => unlisteners.push(fn));
    listen("displays-changed", () => {
      refreshDock();
    }).then((fn) => unlisteners.push(fn));
    // Ticket 61: a background dock failure — a shell-initiated re-assert
    // (ABN_POSCHANGED) or the drift watchdog — surfaces in the window's error
    // banner instead of leaving a half-docked bar.
    listen<string>("quick-launch-dock-error", (e) => {
      error = e.payload;
    }).then((fn) => unlisteners.push(fn));
    return () => {
      unlisteners.forEach((fn) => fn());
      clearTimeout(copiedTimer);
      clearTimeout(runNoticeTimer);
    };
  });

  async function refreshDock() {
    try {
      dock = await getQuickLaunchDockState();
    } catch (e) {
      console.error(e);
    }
  }

  async function toggleDock() {
    error = "";
    try {
      await toggleQuickLaunchDock();
      await refreshDock();
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  async function switchEdge(edge: "left" | "right") {
    error = "";
    try {
      await switchQuickLaunchDockEdge(edge);
      // The backend settles the blocked state during the switch (ticket 63) —
      // re-read instead of merging locally.
      await refreshDock();
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  // The window's one hard promise to itself: never an eternal "Loading…".
  // The ticket-79 freeze presented exactly that way — healthy commands,
  // dead paint — so any startup load outliving this budget reads as failed
  // and surfaces the error line with a Try again affordance.
  const LOAD_TIMEOUT_MS = 10_000;

  function withTimeout<T>(pending: Promise<T>, what: string): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`${what} did not answer in time`)),
        LOAD_TIMEOUT_MS
      );
      pending.then(
        (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        (e) => {
          clearTimeout(timer);
          reject(e);
        }
      );
    });
  }

  async function load() {
    loading = true;
    try {
      const [entriesResult, actionsResult, clipsResult, settings, lgs, ags, cgs] =
        await Promise.all([
          withTimeout(listLaunchEntries(), "The launch list"),
          withTimeout(listQuickActions(), "The quick actions list"),
          withTimeout(listClips(), "The clips list"),
          withTimeout(getSettings(), "The settings"),
          withTimeout(listGroups("launch"), "The launch groups list"),
          withTimeout(listGroups("action"), "The action groups list"),
          withTimeout(listGroups("clip"), "The clip groups list"),
          // Ticket 98: the shared run-state store — the same one the Quick
          // Actions page reads — seeds itself from the registry here.
          withTimeout(syncQuickActionRuns(), "The running-actions check"),
        ]);
      entries = entriesResult;
      actions = actionsResult;
      clips = clipsResult;
      // The same settings read carries the theme and all three Groups
      // features — every one live-updates through `quick-launch-changed`.
      launchGroupsOn = settings.launch_groups === "on";
      actionGroupsOn = settings.action_groups === "on";
      clipGroupsOn = settings.clip_groups === "on";
      const mode = settings.theme as ThemeMode;
      if (mode === "system" || mode === "light" || mode === "dark") {
        restoreTheme(mode);
      }
      launchGroups = lgs;
      actionGroups = ags;
      clipGroups = cgs;
      launchCollapse.prune(lgs.map((g) => g.id));
      actionCollapse.prune(ags.map((g) => g.id));
      clipCollapse.prune(cgs.map((g) => g.id));
      // Deleting the last clip removes the third tab again (accepted) — if
      // it was selected, land on Launch rather than a dead selection.
      if (clips.length === 0 && tab === "clips") tab = "launch";
      error = "";
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Ticket 79: base two tabs, plus Quick Clips iff at least one clip
  // exists. Short labels and icons feed the strip's measured degradation
  // chain (research 0004 rule 4); `title` keeps every stage named for
  // tooltips and assistive tech. Icon names verified against the existing
  // set in Icon.svelte (rocket / terminal / copy).
  const qlTabs = $derived.by(() => {
    const tabs = [
      {
        id: "launch",
        label: "Quick Launch",
        shortLabel: "Launch",
        icon: "rocket",
        title: "Quick Launch",
      },
      {
        id: "actions",
        label: "Quick Actions",
        shortLabel: "Actions",
        icon: "terminal",
        title: "Quick Actions",
      },
    ];
    if (clips.length > 0) {
      tabs.push({
        id: "clips",
        label: "Quick Clips",
        shortLabel: "Clips",
        icon: "copy",
        title: "Quick Clips",
      });
    }
    return tabs;
  });

  async function start() {
    launching = true;
    error = "";
    try {
      await startQuickLaunch();
    } catch (e) {
      console.error(e);
      error = String(e);
      launching = false;
    }
  }

  // ------------------- ticket 93/97: clickable entries + groups ----------

  /** Sections exist only once at least one group does — and in this
   *  read-only surface a group with no members renders nothing at all
   *  (research 0004 rule 2): there is no ⋯ menu here to fill it from. */
  const launchGrouped = $derived(launchGroupsOn && launchGroups.length > 0);

  const launchUngrouped = $derived(
    entries.filter((e) => e.group_id === null)
  );

  const launchSections = $derived(
    launchGroups
      .map((g) => ({
        group: g,
        rows: entries.filter((e) => e.group_id === g.id),
      }))
      .filter((s) => s.rows.length > 0)
  );

  const actionsGrouped = $derived(actionGroupsOn && actionGroups.length > 0);

  const actionsUngrouped = $derived(
    actions.filter((a) => a.group_id === null)
  );

  const actionSections = $derived(
    actionGroups
      .map((g) => ({
        group: g,
        rows: actions.filter((a) => a.group_id === g.id),
      }))
      .filter((s) => s.rows.length > 0)
  );

  const clipsGrouped = $derived(clipGroupsOn && clipGroups.length > 0);

  const clipsUngrouped = $derived(clips.filter((c) => c.group_id === null));

  const clipSections = $derived(
    clipGroups
      .map((g) => ({
        group: g,
        rows: clips.filter((c) => c.group_id === g.id),
      }))
      .filter((s) => s.rows.length > 0)
  );

  /** True while any launch run is in flight — Start all and the entry rows
   *  share one backend pipeline whose runs are single-flight, so every
   *  start affordance waits together rather than inviting a rejection. */
  const startInFlight = $derived(launching || startingEntries.size > 0);

  /** Starts just this entry through the same pipeline as Start all
   *  (ticket 93). The row says "Starting…" until `launch-run-done` lands;
   *  a rejection (single-flight guard, vanished entry) releases immediately
   *  and surfaces its reason in the error line. */
  async function startEntry(entry: LaunchEntry) {
    error = "";
    const next = new Set(startingEntries);
    next.add(entry.id);
    startingEntries = next;
    try {
      await startLaunchEntry(entry.id);
    } catch (e) {
      console.error(e);
      error = String(e);
      const recovered = new Set(startingEntries);
      recovered.delete(entry.id);
      startingEntries = recovered;
    }
  }

  /** Ticket 93: the finished run's summary as a quiet status line — visible
   *  feedback for both Start all and single-entry starts (research 0004
   *  rule 5), auto-cleared on the main page's flash cadence. */
  function flashRun(message: string) {
    runNotice = message;
    clearTimeout(runNoticeTimer);
    runNoticeTimer = setTimeout(() => (runNotice = ""), 3200);
  }

  async function run(action: QuickAction) {
    error = "";
    try {
      await runQuickAction(action.id);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  /** Stop (tickets 62 & 92) via the shared store's lifecycle (ticket 98):
   *  Stopping is set and cleared there; only a refusal surfaces here. */
  async function stop(action: QuickAction) {
    error = "";
    try {
      await stopActionRun(action.id);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  /** Copies via the clipboard command and flashes the row only once the
   *  write has honestly landed (ticket 78's command contract). */
  async function copy(clip: Clip) {
    error = "";
    try {
      await copyClip(clip.id);
      copiedId = clip.id;
      copiedAnnouncement = `${clipTitle(clip.name, clip.content)} copied.`;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedId = null), 1200);
    } catch (e) {
      console.error(e);
      error = String(e);
    }
  }

  function close() {
    invoke("close_quick_launch_window").catch((e) => console.error(e));
  }
</script>

<svelte:head>
  <title>Quick Launch</title>
</svelte:head>

<div
  class="qlw"
  class:qlw--docked={dock.docked}
  class:qlw--docked-left={dock.docked && dock.edge === "left"}
  class:qlw--docked-right={dock.docked && dock.edge === "right"}
>
  <header
    class="qlw__bar"
    data-tauri-drag-region={titleBarDragRegion(dock.docked)}
  >
    <span class="qlw__mark" aria-hidden="true"><SproutMark size={16} /></span>
    <h1 class="qlw__title">Quick Launch</h1>
    {#if dock.docked}
      <span class="qlw__dock-hint" aria-hidden="true">
        <Icon
          name={dock.edge === "left" ? "dock-left" : "dock-right"}
          size={13}
        />
      </span>
      <IconButton
        icon="chevron-left"
        label={dock.left_eligible ? "Dock to the left edge" : SEAM_REASON}
        quiet
        disabled={dock.edge === "left" || !dock.left_eligible}
        onclick={() => switchEdge("left")}
      />
      <IconButton
        icon="chevron-right"
        label={dock.right_eligible ? "Dock to the right edge" : SEAM_REASON}
        quiet
        disabled={dock.edge === "right" || !dock.right_eligible}
        onclick={() => switchEdge("right")}
      />
    {/if}
    <IconButton
      icon={dock.docked
        ? "undock"
        : dock.edge === "left"
          ? "dock-left"
          : "dock-right"}
      label={dock.docked
        ? "Undock — float again"
        : dock.edge === "left"
          ? "Dock to the left edge"
          : "Dock to the right edge"}
      quiet
      onclick={toggleDock}
    />
    <IconButton icon="x" label="Close window" onclick={close} />
  </header>

  {#if showBlocked}
    <!-- Ticket 63: auto-hide was refused by the shell — say why and offer the
         free edge instead of silently pinning the strip forever.
         Ticket 119 reuses the same banner for a seam-docked strip. -->
    <div class="qlw__blocked" role="status">
      <span class="qlw__blocked-icon" aria-hidden="true">
        <Icon name="warn" size={15} />
      </span>
      <p class="qlw__blocked-text">
        {#if seamBlocked}
          {SEAM_REASON}
        {:else}
          {dock.blocked}. The strip stays pinned until that edge frees up —
          hiding resumes on its own.
        {/if}
      </p>
      <Button
        variant="ghost"
        onclick={() => switchEdge(dock.edge === "left" ? "right" : "left")}
      >
        {#if seamBlocked}
          Move to outer edge
        {:else}
          Move to the {dock.edge === "left" ? "right" : "left"} edge
        {/if}
      </Button>
    </div>
  {/if}

  {#snippet launchRow(entry: LaunchEntry)}
    <!-- Ticket 93: one entry, one click. The whole row starts just that entry;
         the accessible name carries the verb so screen readers hear what the
         click does ("Start Spotify", not "Spotify, button"). -->
    <li>
      <button
        type="button"
        class="qlw__entry"
        aria-label={`Start ${entry.name}`}
        disabled={startInFlight}
        onclick={() => startEntry(entry)}
        use:lazyIcon={entry.kind === "app" ? entry.target : ""}
      >
        <span class="qlw__entry-badge" aria-hidden="true">
          {#if entry.kind === "app" && appIcons[entry.target]}
            <!-- Ticket 97: the app's real icon, lazily extracted; kind
                 glyphs stay for commands and unresolvable targets. -->
            <img
              class="qlw__entry-icon"
              src={appIcons[entry.target]}
              alt=""
              width={16}
              height={16}
            />
          {:else}
            <Icon
              name={entry.kind === "app" ? "rocket" : "terminal"}
              size={14}
            />
          {/if}
        </span>
        <span class="qlw__entry-name">{entry.name}</span>
        {#if startingEntries.has(entry.id)}
          <span class="qlw__entry-starting">Starting…</span>
        {/if}
      </button>
    </li>
  {/snippet}

  {#snippet actionRow(action: QuickAction)}
    <li class="qlw__action">
      <span class="qlw__action-name">{action.name}</span>
      <!-- Ticket 93: hover/focus tooltip — the bold name plus the command,
           truncated to one line. The tip stays in the DOM (opacity only), so
           `aria-describedby` below gives keyboard and screen-reader users
           the same content. -->
      <span class="qlw__tip" id={`qlw-tip-action-${action.id}`}>
        <span class="qlw__tip-name">{action.name}</span>
        <span class="qlw__tip-body">{action.command}</span>
      </span>
      <!-- Ticket 98: the three-state control is shared with the main app's
           Quick Actions page — one markup, one spinner, one vocabulary. -->
      <QuickActionRunControl
        name={action.name}
        stoppable={action.stoppable}
        running={quickActionRuns.running.has(action.id)}
        stopping={quickActionRuns.stopping.has(action.id)}
        onrun={() => run(action)}
        onstop={() => stop(action)}
        describedby={`qlw-tip-action-${action.id}`}
      />
    </li>
  {/snippet}

  {#snippet clipRow(clip: Clip)}
    {@const title = clipTitle(clip.name, clip.content)}
    <li class="qlw__clip-row">
      <button
        type="button"
        class="qlw__clip"
        aria-label={`Copy ${title} to the clipboard`}
        aria-describedby={`qlw-tip-clip-${clip.id}`}
        onclick={() => copy(clip)}
      >
        <span class="qlw__clip-badge" aria-hidden="true">
          <Icon name={copiedId === clip.id ? "check" : "copy"} size={14} />
        </span>
        <span class="qlw__clip-name">{title}</span>
        {#if copiedId === clip.id}
          <span class="qlw__clip-copied">Copied</span>
        {:else}
          <span class="qlw__clip-excerpt">{clip.content}</span>
        {/if}
      </button>
      <!-- Ticket 93: same tooltip contract as the action rows — bold name
           plus the full content on one truncated line. -->
      <span class="qlw__tip" id={`qlw-tip-clip-${clip.id}`}>
        <span class="qlw__tip-name">{title}</span>
        <span class="qlw__tip-body">{clip.content}</span>
      </span>
    </li>
  {/snippet}

  <div class="qlw__tabs">
    <Tabs
      tabs={qlTabs}
      selected={tab}
      onselect={(id) => (tab = id)}
      ariaLabel="Quick Launch window sections"
    >
      {#snippet panel(id)}
        {#if id === "launch"}
          {#if loading && entries.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else if entries.length === 0}
            <div class="qlw__empty">
              <span class="qlw__empty-icon" aria-hidden="true">
                <Icon name="rocket" size={22} />
              </span>
              <p class="qlw__empty-title">Nothing to launch</p>
              <p class="qlw__empty-body">
                Add entries in the main window's Quick Launch page — the
                tray's left-click opens this window, where Start all lives.
              </p>
            </div>
          {:else}
            <!-- Ticket 93: Start all stays pinned on top; the entry list
                 scrolls beneath it. -->
            <div class="qlw__launch">
              <p class="qlw__count">
                {entries.length} {entries.length === 1 ? "entry" : "entries"}
                in the Quick Launch list.
              </p>
              <Button onclick={start} disabled={startInFlight}>
                <Icon name="play" size={15} />
                {launching ? "Starting…" : "Start all"}
              </Button>
              <div class="qlw__list">
                {#if !launchGrouped}
                  <ul class="qlw__entries">
                    {#each entries as entry (entry.id)}
                      {@render launchRow(entry)}
                    {/each}
                  </ul>
                {:else}
                  {#if launchUngrouped.length > 0}
                    <ul class="qlw__entries">
                      {#each launchUngrouped as entry (entry.id)}
                        {@render launchRow(entry)}
                      {/each}
                    </ul>
                  {/if}
                  {#each launchSections as section (section.group.id)}
                    <!-- The shared GroupAccordion in its flush strip variant:
                         sections exist only while they have members —
                         nothing here can fill an empty one (research 0004
                         rule 2). -->
                    <GroupAccordion
                      flush
                      open={launchCollapse.isOpen(section.group.id)}
                      controls={`qlw-group-${section.group.id}`}
                      name={section.group.name}
                      count={section.rows.length}
                      onToggle={() => launchCollapse.toggle(section.group.id)}
                    >
                      <ul class="qlw__entries">
                        {#each section.rows as entry (entry.id)}
                          {@render launchRow(entry)}
                        {/each}
                      </ul>
                    </GroupAccordion>
                  {/each}
                {/if}
              </div>
            </div>
          {/if}
        {:else if id === "actions"}
          {#if loading && actions.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else if actions.length === 0}
            <div class="qlw__empty">
              <span class="qlw__empty-icon" aria-hidden="true">
                <Icon name="terminal" size={22} />
              </span>
              <p class="qlw__empty-title">No quick actions</p>
              <p class="qlw__empty-body">
                Compose PowerShell commands in the main window's Quick Actions
                page — they run here, hidden, as the current user.
              </p>
            </div>
          {:else}
            <!-- Ticket 97: the tab mirrors the collection's Groups toggle,
                 exactly like the Launch list. -->
            <div class="qlw__list qlw__list--padded">
              {#if !actionsGrouped}
                <ul class="qlw__actions">
                  {#each actions as action (action.id)}
                    {@render actionRow(action)}
                  {/each}
                </ul>
              {:else}
                {#if actionsUngrouped.length > 0}
                  <ul class="qlw__actions">
                    {#each actionsUngrouped as action (action.id)}
                      {@render actionRow(action)}
                    {/each}
                  </ul>
                {/if}
                {#each actionSections as section (section.group.id)}
                  <GroupAccordion
                    flush
                    open={actionCollapse.isOpen(section.group.id)}
                    controls={`qlw-actions-group-${section.group.id}`}
                    name={section.group.name}
                    count={section.rows.length}
                    onToggle={() => actionCollapse.toggle(section.group.id)}
                  >
                    <ul class="qlw__actions">
                      {#each section.rows as action (action.id)}
                        {@render actionRow(action)}
                      {/each}
                    </ul>
                  </GroupAccordion>
                {/each}
              {/if}
            </div>
          {/if}
        {:else}
          {#if loading && clips.length === 0}
            <p class="qlw__sifting" aria-live="polite">Loading…</p>
          {:else}
            <!-- Ticket 97: same Groups mirror as the other two tabs. -->
            <div class="qlw__list qlw__list--padded">
              {#if !clipsGrouped}
                <ul class="qlw__clips">
                  {#each clips as clip (clip.id)}
                    {@render clipRow(clip)}
                  {/each}
                </ul>
              {:else}
                {#if clipsUngrouped.length > 0}
                  <ul class="qlw__clips">
                    {#each clipsUngrouped as clip (clip.id)}
                      {@render clipRow(clip)}
                    {/each}
                  </ul>
                {/if}
                {#each clipSections as section (section.group.id)}
                  <GroupAccordion
                    flush
                    open={clipCollapse.isOpen(section.group.id)}
                    controls={`qlw-clips-group-${section.group.id}`}
                    name={section.group.name}
                    count={section.rows.length}
                    onToggle={() => clipCollapse.toggle(section.group.id)}
                  >
                    <ul class="qlw__clips">
                      {#each section.rows as clip (clip.id)}
                        {@render clipRow(clip)}
                      {/each}
                    </ul>
                  </GroupAccordion>
                {/each}
              {/if}
            </div>
          {/if}
        {/if}
      {/snippet}
    </Tabs>
  </div>

  {#if runNotice}
    <!-- Ticket 93: the finished launch run's summary (Start all and
         single-entry starts alike) — visible feedback, not just the system
         notification (research 0004 rule 5). -->
    <p class="qlw__status" role="status">{runNotice}</p>
  {/if}

  {#if error}
    <div class="qlw__error-row">
      <p class="qlw__error" role="alert">{error}</p>
      <Button variant="ghost" onclick={() => { load(); refreshDock(); }}>
        Try again
      </Button>
    </div>
  {/if}

  <div class="sr-only" role="status" aria-live="polite">
    {copiedAnnouncement}
  </div>
</div>

<style>
  .qlw {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-page);
    border: 1px solid var(--border);
  }

  .qlw__bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    flex-shrink: 0;
    user-select: none;
  }

  .qlw__mark {
    display: inline-flex;
    flex-shrink: 0;
  }

  /* The docked strip (ticket 53) gets a distinct edge: a hint in the header
     and a slightly deeper page background so the pinned bar reads as one
     surface against the desktop. Ticket 59: the header padding mirrors on
     both docked edges — the wider inset lands on the screen-edge side, so a
     left- and right-docked strip have identical side spacing (no gap
     asymmetry, no crowding against the neighboring application's space). */
  .qlw--docked {
    background: var(--bg-card);
  }

  .qlw--docked-left .qlw__bar {
    padding-left: var(--space-4);
    padding-right: var(--space-2);
  }

  .qlw--docked-right .qlw__bar {
    padding-left: var(--space-2);
    padding-right: var(--space-4);
  }

  .qlw__dock-hint {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--accent);
  }

  .qlw__title {
    flex: 1;
    min-width: 0;
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    letter-spacing: var(--tracking-display);
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .qlw__sifting {
    margin: 0;
    padding: var(--space-5) var(--space-4);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  /* Ticket 93: Start all and the count stay pinned; the entry list scrolls
     beneath them inside the tab panel. */
  .qlw__launch {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    flex: 1;
    min-height: 0;
    padding: var(--space-4);
  }

  /* Ticket 93/97: the scroll container every tab's list lives in. Launch
     nests it inside the pinned Start-all head; Actions/Clips use it directly
     with their own padding (`--padded`). The bottom runway is where
     below-anchored tooltips land at full scroll. */
  .qlw__list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-3);
  }

  .qlw__list--padded {
    padding: var(--space-3);
  }

  .qlw__list > :last-child {
    padding-bottom: calc(var(--space-7) + var(--space-6));
  }

  .qlw__count {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .qlw__entries {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .qlw__entry {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease-out);
  }

  .qlw__entry:hover:not(:disabled),
  .qlw__entry:focus-visible {
    border-color: var(--accent-tint-border);
  }

  .qlw__entry:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .qlw__entry:disabled {
    cursor: default;
  }

  .qlw__entry:disabled .qlw__entry-name {
    color: var(--text-muted);
  }

  .qlw__entry-badge {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--text-muted);
  }

  /* Ticket 97: the entry's real app icon, where one resolves. */
  .qlw__entry-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  .qlw__entry-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  .qlw__entry-starting {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .qlw__actions {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .qlw__action {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .qlw__action-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  /* Ticket 79: the read-only Quick Clips rows — whole-row click-to-copy,
   * same visual language as the actions list. No editing affordances here:
   * all CRUD stays on the main app's /clips page (research 0004 rule 3). */
  .qlw__clips {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .qlw__clip {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease-out);
  }

  .qlw__clip:hover {
    border-color: var(--accent-tint-border);
  }

  .qlw__clip:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .qlw__clip-badge {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--accent);
  }

  .qlw__clip-name {
    flex-shrink: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  .qlw__clip-excerpt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .qlw__clip-copied {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--accent);
  }

  .qlw__clip-row {
    position: relative;
  }

  /* Ticket 93/97: hover/focus tooltips on the action and clip rows — the
     bold name plus the row's content (command / clip text) truncated to one
     line. Anchored BELOW the row so the scrollport never clips it at the
     top; each `.qlw__list` carries bottom runway on its last child so a
     last-row tooltip fits at full scroll. The tip is hidden with opacity
     only — never display/visibility — so `aria-describedby` still exposes
     its content to assistive tech, and keyboard focus (`:focus-within`)
     raises exactly what hovering does. */
  .qlw__tip {
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

  .qlw__action:hover .qlw__tip,
  .qlw__action:focus-within .qlw__tip,
  .qlw__clip-row:hover .qlw__tip,
  .qlw__clip-row:focus-within .qlw__tip {
    opacity: 1;
  }

  .qlw__tip-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text);
  }

  .qlw__tip-body {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  /* The live region is the shared `.sr-only` utility (tokens.css). */

  .qlw__empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-7) var(--space-5);
    text-align: center;
  }

  .qlw__empty-icon {
    display: inline-flex;
    color: var(--accent);
    margin-bottom: var(--space-2);
  }

  .qlw__empty-title {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--text);
  }

  .qlw__empty-body {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  /* Ticket 93: the finished run's summary line — quiet visible feedback
     beside (not replacing) the system notification. */
  .qlw__status {
    margin: 0;
    padding: 0 var(--space-4) var(--space-2);
    font-size: var(--text-sm);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  .qlw__error-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-4) var(--space-4);
  }

  .qlw__error {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  /* Ticket 63: the blocked-auto-hide banner — the shell refused the edge, so
     the strip stays pinned and this says why. Shared warn tokens; `status`
     (not `alert`) because nothing is broken — hiding simply waits for the
     edge to free up. */
  .qlw__blocked {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 var(--space-4) var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: var(--warn-tint);
    border: 1px solid var(--warn-tint-border);
    border-radius: var(--radius);
  }

  .qlw__blocked-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--warn-text);
  }

  .qlw__blocked-text {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--text-sm);
    color: var(--warn-text);
    overflow-wrap: anywhere;
  }

  /* The tab strip fills the window below the header; the active panel
     stretches and lets its list scroll internally. The panel itself is a
     flex column: it is a plain block in Tabs.svelte, and a block panel
     clipped its direct-child lists (Actions/Clips grew past it with no
     scrollbar — ticket 102's root cause); flexing it lets every tab's
     `flex: 1; min-height: 0` scroll container actually resolve. */
  .qlw__tabs {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .qlw__tabs :global(.tabs) {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .qlw__tabs :global(.tabs__panel) {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* This page's flexed panel ties with Tabs' own `.tabs__panel[hidden]`
     once both carry their scope hashes, so re-state hiding here explicitly —
     otherwise stylesheet order decides and every tab renders at once. */
  .qlw__tabs :global(.tabs__panel[hidden]) {
    display: none;
  }
</style>