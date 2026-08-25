<script lang="ts">
  import { onMount } from "svelte";
  import type { BackupCounts, Settings } from "$lib/types";
  import {
    exportBackup,
    getSettings,
    importBackup,
    inspectBackup,
    updateAutostart,
    updateSettings,
  } from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import Select from "$lib/components/Select.svelte";
  import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { theme, restoreTheme, selectTheme } from "$lib/theme.svelte";
  import type { ThemeMode } from "$lib/theme.svelte";
  import {
    checkForUpdates,
    installNow,
    updateState,
  } from "$lib/updateState.svelte";
  import { COLLECTIONS, EXPORT_ORDER } from "$lib/collections";
  import type { CollectionKey } from "$lib/collections";

  const themeOptions: { mode: ThemeMode; label: string }[] = [
    { mode: "system", label: "System" },
    { mode: "light", label: "Light" },
    { mode: "dark", label: "Dark" },
  ];

  const dockModeOptions: { value: string; label: string }[] = [
    { value: "auto-hide", label: "Auto-hide" },
    { value: "fixed", label: "Fixed" },
  ];

  const dockEdgeOptions: { value: string; label: string }[] = [
    { value: "left", label: "Left" },
    { value: "right", label: "Right" },
  ];

  const dockStateOptions: { value: string; label: string }[] = [
    { value: "floating", label: "Floating" },
    { value: "docked", label: "Docked" },
  ];

  const autostartOptions: { value: string; label: string }[] = [
    { value: "on", label: "On" },
    { value: "off", label: "Off" },
  ];

  let settings: Settings | null = $state(null);
  let timeout = $state(10);
  let retention = $state(30);
  let installDir = $state("");
  let launchConcurrency = $state(8);
  let dockMode = $state("auto-hide");
  let dockEdge = $state("left");
  let dockState = $state("floating");
  let autostart = $state("on");
  let loading = $state(true);
  let loadFailed = $state(false);
  let saving = $state(false);
  let saved = $state("");
  let error = $state("");

  // The manual update check's outcome (ticket 74): the found-version state
  // itself lives in the shared store, so the rail pill follows along.
  let checking = $state(false);
  let checkResult = $state<"idle" | "current" | "failed">("idle");
  let installConfirmOpen = $state(false);
  let installError = $state("");

  // The whole-app backup (ticket 80): its own notices, separate from the
  // form's save feedback.
  let backupBusy = $state(false);
  let backupStatus = $state("");
  let backupError = $state("");
  let restoreFile = $state("");
  let restoreCounts: BackupCounts | null = $state(null);

  // Selective export (ticket 87): the collection checklist lives inside the
  // export dialog, not on the knob row (research 0007). Everything starts
  // included, so the plain flow still writes the whole-app backup.
  let exportOpen = $state(false);
  let include = $state<Record<CollectionKey, boolean>>({
    launch_entries: true,
    quick_actions: true,
    clips: true,
    products: true,
    presets: true,
  });
  const anyIncluded = $derived(Object.values(include).some(Boolean));

  onMount(() => {
    load();
  });

  async function load() {
    loading = true;
    loadFailed = false;
    error = "";
    try {
      const loaded = await getSettings();
      settings = loaded;
      timeout = loaded.default_timeout_minutes;
      retention = loaded.log_retention_days;
      installDir = loaded.install_dir;
      launchConcurrency = loaded.launch_concurrency;
      dockMode = loaded.dock_mode;
      dockEdge = loaded.dock_edge;
      dockState = loaded.dock_state;
      autostart = loaded.autostart;
      const persisted = loaded.theme as ThemeMode;
      if (persisted === "system" || persisted === "light" || persisted === "dark") {
        if (persisted !== theme.mode) restoreTheme(persisted);
      }
      loadFailed = false;
    } catch {
      loadFailed = true;
    } finally {
      loading = false;
    }
  }

  async function pick(mode: ThemeMode) {
    saved = "";
    error = "";
    try {
      await selectTheme(mode);
    } catch {
      error = "Couldn't save the theme — it applies for now, but won't survive a restart.";
    }
  }

  async function pickAutostart(value: string) {
    const previous = autostart;
    autostart = value;
    saved = "";
    error = "";
    try {
      await updateAutostart(value === "on");
    } catch {
      // Neither the setting nor the registration changed — put the toggle
      // back so it tells the truth.
      autostart = previous;
      error =
        "Couldn't change the start-with-Windows registration — try again.";
    }
  }

  async function browseInstallDir() {
    error = "";
    try {
      const picked = await open({
        title: "Default install directory",
        multiple: false,
        directory: true,
      });
      if (typeof picked === "string" && picked) installDir = picked;
    } catch {
      error = "Couldn't open the folder picker — type the path directly instead.";
    }
  }

  async function save() {
    if (!settings) return;
    saving = true;
    saved = "";
    error = "";
    try {
      await updateSettings({
        default_timeout_minutes: Math.max(1, Math.floor(timeout) || 1),
        log_retention_days: Math.max(1, Math.floor(retention) || 1),
        theme: theme.mode,
        install_dir: installDir.trim(),
        launch_concurrency: Math.min(50, Math.max(1, Math.floor(launchConcurrency) || 1)),
        dock_mode: dockMode,
        dock_edge: dockEdge,
        dock_state: dockState,
        autostart,
        // Not this screen's knob — the Quick Launch toolbar owns it (ticket
        // 88); the loaded value passes through untouched.
        desktop_assignments: settings.desktop_assignments,
      });
      saved = "Saved — the next run honors these.";
      // Reflect any clamping back into the fields.
      timeout = Math.max(1, Math.floor(timeout) || 1);
      retention = Math.max(1, Math.floor(retention) || 1);
      launchConcurrency = Math.min(50, Math.max(1, Math.floor(launchConcurrency) || 1));
    } catch {
      error = "Couldn't save the settings — try again. If it keeps failing, close Sprout and relaunch.";
    } finally {
      saving = false;
    }
  }

  async function runUpdateCheck() {
    checking = true;
    try {
      const result = await checkForUpdates();
      checkResult =
        result.status === "available"
          ? "idle"
          : result.status === "up-to-date"
            ? "current"
            : "failed";
    } finally {
      checking = false;
    }
  }

  async function applyInstall() {
    try {
      await installNow();
      // A successful spawn exits the app within the second.
      installConfirmOpen = false;
    } catch (e) {
      // Failure reopens the dialog with the error; the row's install
      // button stays available for a retry.
      installError = String(e);
      installConfirmOpen = true;
    }
  }

  /** "3 products, 1 preset, 5 launch entries" — the count list behind both
   *  backup notices and the restore confirmation. Nouns come from the
   *  shared collection names so notices never drift from the tabs. */
  function describeCounts(counts: BackupCounts): string {
    const phrase = (n: number, nouns: { one: string; many: string }) =>
      n && `${n} ${n === 1 ? nouns.one : nouns.many}`;
    return [
      phrase(counts.products, COLLECTIONS.products),
      phrase(counts.presets, COLLECTIONS.presets),
      phrase(counts.launch_entries, COLLECTIONS.launch_entries),
      phrase(counts.quick_actions, COLLECTIONS.quick_actions),
      phrase(counts.clips, COLLECTIONS.clips),
    ]
      .filter(Boolean)
      .join(", ");
  }

  function openExportDialog() {
    backupStatus = "";
    backupError = "";
    exportOpen = true;
  }

  async function exportSelected() {
    backupStatus = "";
    backupError = "";
    try {
      const path = await saveDialog({
        title: "Back up Sprout",
        defaultPath: "sprout-backup.json",
        filters: [{ name: "Sprout backup", extensions: ["json"] }],
      });
      if (!path) return;
      backupBusy = true;
      const counts = await exportBackup(path, include);
      backupStatus = `Backed up ${describeCounts(counts)} to ${path}`;
    } catch (e) {
      console.error(e);
      // Rejections are authored backend copy; infrastructure failures are rare.
      backupError = String(e);
    } finally {
      backupBusy = false;
    }
  }

  async function restoreViaDialog() {
    backupStatus = "";
    backupError = "";
    try {
      const picked = await open({
        title: "Open a Sprout backup",
        multiple: false,
        directory: false,
        filters: [{ name: "Sprout backup", extensions: ["json"] }],
      });
      if (typeof picked !== "string") return;
      backupBusy = true;
      restoreFile = picked;
      restoreCounts = await inspectBackup(picked);
    } catch (e) {
      console.error(e);
      backupError = String(e);
      restoreFile = "";
      restoreCounts = null;
    } finally {
      backupBusy = false;
    }
  }

  async function importBackupFile(file: string) {
    backupBusy = true;
    try {
      const summary = await importBackup(file);
      const restored = describeCounts(summary.inserted);
      const skipped =
        summary.skipped.products +
        summary.skipped.presets +
        summary.skipped.launch_entries +
        summary.skipped.quick_actions +
        summary.skipped.clips;
      if (!restored) {
        backupStatus = "Nothing to restore — everything in the file already exists.";
      } else if (skipped > 0) {
        backupStatus =
          `Restored ${restored}. ${skipped} item${skipped === 1 ? " was" : "s were"} already present and kept.`;
      } else {
        backupStatus = `Restored ${restored}.`;
      }
    } catch (e) {
      console.error(e);
      // Rejections are authored backend copy; infrastructure failures are rare.
      backupError = String(e);
    } finally {
      backupBusy = false;
    }
  }
</script>

<section class="settings" aria-labelledby="settings-title">
  <PageHeader titleId="settings-title" title="Settings">
    {#snippet subtitle()}
      Defaults for authoring and housekeeping, persisted in the Library database and honored by
      every run.
    {/snippet}
  </PageHeader>

  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}
  {#if saved}
    <Notice tone="ok">{saved}</Notice>
  {/if}

  {#if loading}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed || !settings}
    <EmptyState icon="x" title="Couldn't read the settings">
      <p>
        Couldn't read the settings from
        <span class="mono">%LOCALAPPDATA%\Sprout\sprout.db</span> — the file may be locked or
        missing.
      </p>
      <p>Try again; if it keeps failing, close the app and relaunch.</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={load}>Try again</Button>
      </div>
    </EmptyState>
  {:else}
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <article class="knob">
        <div class="knob__body">
          <span class="knob__label">Theme</span>
          <p class="knob__hint">
            Follows the Windows appearance setting, or pins the app to one look. Applies
            immediately and is remembered next launch; no save needed.
          </p>
        </div>
        <div class="theme-picker" role="radiogroup" aria-label="Theme">
          {#each themeOptions as option (option.mode)}
            <button
              type="button"
              role="radio"
              class="theme-picker__option"
              class:theme-picker__option--active={theme.mode === option.mode}
              aria-checked={theme.mode === option.mode}
              onclick={() => pick(option.mode)}
            >
              {option.label}
            </button>
          {/each}
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <label class="knob__label" for="default-timeout">Default timeout</label>
          <p class="knob__hint">
            Minutes a requirement may take before its installer is killed. New requirements in the
            preset composer start with this value; you can still override each one.
          </p>
        </div>
        <div class="knob__input">
          <input
            id="default-timeout"
            name="default-timeout"
            class="field__input"
            type="number"
            min="1"
            max="1440"
            autocomplete="off"
            value={timeout}
            oninput={(e) => (timeout = Number((e.target as HTMLInputElement).value))}
          />
          <span class="knob__unit">min</span>
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <label class="knob__label" for="log-retention">Log retention</label>
          <p class="knob__hint">
            How long a finished run's raw log folder is kept before it is pruned. Pruning happens
            after every run and at app start. The runs list itself is never deleted.
          </p>
        </div>
        <div class="knob__input">
          <input
            id="log-retention"
            name="log-retention"
            class="field__input"
            type="number"
            min="1"
            max="3650"
            autocomplete="off"
            value={retention}
            oninput={(e) => (retention = Number((e.target as HTMLInputElement).value))}
          />
          <span class="knob__unit">days</span>
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <label class="knob__label" for="install-dir">Install directory</label>
          <p class="knob__hint">
            Where installs and upgrades land. Empty means the installer's own default location;
            pick or type an absolute path like D:\Apps. Software that ignores it is reported on
            the Plan. Never shared with exported presets.
          </p>
        </div>
        <div class="knob__input knob__input--wide">
          <input
            id="install-dir"
            name="install-dir"
            class="field__input field__input--dir"
            type="text"
            autocomplete="off"
            spellcheck="false"
            placeholder="(winget default)"
            value={installDir}
            oninput={(e) => (installDir = (e.target as HTMLInputElement).value)}
          />
          <Button type="button" variant="secondary" onclick={browseInstallDir}>Browse…</Button>
          {#if installDir}
            <Button type="button" variant="ghost" onclick={() => (installDir = "")}>Clear</Button>
          {/if}
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <label class="knob__label" for="launch-concurrency">Launch concurrency</label>
          <p class="knob__hint">
            How many Quick Launch apps may start at once before the rest queue.
            A slot frees as soon as the app's window appears.
          </p>
        </div>
        <div class="knob__input">
          <input
            id="launch-concurrency"
            name="launch-concurrency"
            class="field__input"
            type="number"
            min="1"
            max="50"
            autocomplete="off"
            value={launchConcurrency}
            oninput={(e) => (launchConcurrency = Number((e.target as HTMLInputElement).value))}
          />
          <span class="knob__unit">apps</span>
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <label class="knob__label" for="dock-state">Quick Launch window</label>
          <p class="knob__hint">
            Whether the Quick Launch window floats as a palette or docks to a screen edge as a
            bar. Applied to an open window on save and remembered next time it opens; the dock
            toggle inside the window writes back here.
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="dock-state"
            variant="small"
            value={dockState}
            onchange={(v) => (dockState = v)}
          >
            {#each dockStateOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <label class="knob__label" for="dock-mode">Dock mode</label>
          <p class="knob__hint">
            How the Quick Launch dock behaves when docked to a screen edge. Auto-hide slides it to a
            sliver when not hovered and reclaims the space; fixed keeps the strip permanently
            reserved, like a pinned taskbar.
          </p>
        </div>
        <div class="knob__input">
          <Select id="dock-mode" variant="small" value={dockMode} onchange={(v) => (dockMode = v)}>
            {#each dockModeOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <label class="knob__label" for="dock-edge">Default dock edge</label>
          <p class="knob__hint">
            Which screen edge the dock attaches to when first docked. The dock's own left/right
            switch overrides it per monitor.
          </p>
        </div>
        <div class="knob__input">
          <Select id="dock-edge" variant="small" value={dockEdge} onchange={(v) => (dockEdge = v)}>
            {#each dockEdgeOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <span class="knob__label">Start with Windows</span>
          <p class="knob__hint">
            Registers Sprout to start at login, resident in the tray — the main
            window stays closed and a docked Quick Launch bar reappears on its
            own. Turning it off removes the registration immediately; no restart
            needed.
          </p>
        </div>
        <div class="knob__input">
          <Select
            id="autostart"
            variant="small"
            value={autostart}
            onchange={(v) => pickAutostart(v)}
          >
            {#each autostartOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </Select>
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <span class="knob__label">Backup</span>
          <p class="knob__hint">
            Writes your collections into one JSON file you pick — choose what to include when you
            export; restoring adds what's missing and keeps what's already here. Run history, logs,
            settings, dock memory, and install directories never leave this PC.
          </p>
          {#if backupStatus}
            <Notice tone="ok">{backupStatus}</Notice>
          {/if}
          {#if backupError}
            <Notice tone="error">{backupError}</Notice>
          {/if}
        </div>
        <div class="knob__input">
          <Button variant="secondary" onclick={openExportDialog} disabled={backupBusy}>
            Export…
          </Button>
          <Button variant="secondary" onclick={restoreViaDialog} disabled={backupBusy}>
            Restore…
          </Button>
        </div>
      </article>

      <article class="knob">
        <div class="knob__body">
          <span class="knob__label">Sprout updates</span>
          <p class="knob__hint">
            Checks GitHub releases for a newer build. An update also appears
            beside the version in the navigation rail; installing downloads it
            and restarts Sprout.
          </p>
          {#if updateState.installing}
            <p class="knob__status" role="status">
              Installing Sprout {updateState.available?.version} — Sprout
              restarts when it finishes.
            </p>
          {:else if updateState.available}
            <Notice tone="ok">Sprout {updateState.available.version} is available.</Notice>
          {:else if checking}
            <p class="knob__status" role="status">Checking…</p>
          {:else if checkResult === "current"}
            <Notice tone="ok">You're up to date.</Notice>
          {:else if checkResult === "failed"}
            <Notice tone="warn">
              Couldn't reach the release feed just now — try again later.
            </Notice>
          {/if}
        </div>
        <div class="knob__input">
          {#if updateState.installing}
            <Button disabled>Installing…</Button>
          {:else if updateState.available}
            <Button
              onclick={() => {
                installError = "";
                installConfirmOpen = true;
              }}
            >
              Install {updateState.available.version}
            </Button>
          {:else}
            <Button variant="secondary" onclick={runUpdateCheck} disabled={checking}>
              {checking ? "Checking…" : "Check for updates"}
            </Button>
          {/if}
        </div>
      </article>

      <div class="form__actions">
        <Button kind="submit" disabled={saving}>
          {saving ? "Saving…" : "Save settings"}
        </Button>
      </div>
    </form>
  {/if}
</section>

<ConfirmDialog
  open={installConfirmOpen}
  title="Update available"
  confirmLabel={updateState.installing ? "Installing…" : "Install and restart"}
  onconfirm={applyInstall}
  oncancel={() => (installConfirmOpen = false)}
>
  <p>Install Sprout {updateState.available?.version} now?</p>
  <p>Sprout restarts when the installer finishes.</p>
  {#if installError}
    <Notice tone="error">{installError}</Notice>
  {/if}
</ConfirmDialog>

<ConfirmDialog
  open={restoreCounts !== null}
  title="Restore backup?"
  confirmLabel="Restore"
  onconfirm={() => {
    const file = restoreFile;
    restoreCounts = null;
    restoreFile = "";
    if (file) void importBackupFile(file);
  }}
  oncancel={() => {
    restoreCounts = null;
    restoreFile = "";
  }}
>
  {#if restoreCounts && describeCounts(restoreCounts)}
    <p>
      <strong>{restoreFile.split(/[\\/]/).pop()}</strong> contains
      {describeCounts(restoreCounts)}.
    </p>
    <p>Items that already exist here are kept — nothing is overwritten.</p>
  {:else}
    <p>This file contains no items to restore.</p>
  {/if}
</ConfirmDialog>

<ConfirmDialog
  open={exportOpen}
  title="Export backup"
  confirmLabel="Export selected…"
  confirmDisabled={!anyIncluded}
  onconfirm={() => {
    exportOpen = false;
    void exportSelected();
  }}
  oncancel={() => (exportOpen = false)}
>
  <p>Everything is included by default — untick what should stay out of the file.</p>
  <div class="export-picker" role="group" aria-label="Collections to include">
    {#each EXPORT_ORDER as key (key)}
      <label class="export-picker__item">
        <input type="checkbox" bind:checked={include[key]} />
        <span>{COLLECTIONS[key].label}</span>
      </label>
    {/each}
  </div>
</ConfirmDialog>

<style>
  .settings {
    max-width: 680px;
    margin: 0 auto;
  }

  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .empty-cta {
    margin-top: var(--space-4);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .knob {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
    padding: var(--space-4);
  }

  .knob__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .knob__label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }

  .knob__hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .knob__status {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .knob__input {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
  }

  .knob__input--wide {
    flex: 1 0 auto;
    min-width: 0;
  }

  .field__input--dir {
    width: auto;
    min-width: 220px;
    flex: 1;
    text-align: left;
  }

  .theme-picker {
    display: flex;
    gap: var(--space-1);
    flex-shrink: 0;
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-1);
  }

  .theme-picker__option {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: var(--space-1) var(--space-2);
    cursor: pointer;
  }

  .theme-picker__option:hover {
    color: var(--text);
  }

  .theme-picker__option--active {
    background: var(--accent-tint);
    color: var(--accent);
  }

  .field__input {
    width: 110px;
    font-family: var(--font-mono);
    font-size: var(--text-base);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: var(--space-2) var(--space-3);
    text-align: right;
  }

  .field__input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .knob__unit {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
    width: 2.5em;
  }

  .form__actions {
    display: flex;
    justify-content: flex-end;
  }

  .export-picker {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .export-picker__item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
  }

  .export-picker__item input[type="checkbox"] {
    margin: 0;
    accent-color: var(--accent);
    width: 14px;
    height: 14px;
  }
</style>