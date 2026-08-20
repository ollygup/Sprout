<script lang="ts">
  import { onMount } from "svelte";
  import type { Settings } from "$lib/types";
  import { getSettings, updateSettings } from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import Select from "$lib/components/Select.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { theme, restoreTheme, selectTheme } from "$lib/theme.svelte";
  import type { ThemeMode } from "$lib/theme.svelte";

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

  let settings: Settings | null = $state(null);
  let timeout = $state(10);
  let retention = $state(30);
  let installDir = $state("");
  let launchConcurrency = $state(8);
  let dockMode = $state("auto-hide");
  let dockEdge = $state("left");
  let dockState = $state("floating");
  let loading = $state(true);
  let loadFailed = $state(false);
  let saving = $state(false);
  let saved = $state("");
  let error = $state("");

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
</script>

<section class="settings" aria-labelledby="settings-title">
  <header class="settings__header">
    <h1 id="settings-title" class="settings__title">Settings</h1>
    <p class="settings__sub">
      Defaults for authoring and housekeeping, persisted in the Library database and honored by
      every run.
    </p>
  </header>

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

      <div class="form__actions">
        <Button kind="submit" disabled={saving}>
          {saving ? "Saving…" : "Save settings"}
        </Button>
      </div>
    </form>
  {/if}
</section>

<style>
  .settings {
    max-width: 680px;
    margin: 0 auto;
  }

  .settings__header {
    margin-bottom: var(--space-5);
  }

  .settings__title {
    font-family: var(--font-display);
    font-size: var(--text-2xl);
    line-height: 1.15;
    color: var(--text);
    text-wrap: balance;
  }

  .settings__sub {
    margin: var(--space-2) 0 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
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
</style>