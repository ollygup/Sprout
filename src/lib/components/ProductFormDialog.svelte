<script lang="ts">
  import type { EnvAction, EnvWiring, Product, WingetMatch, WingetShow } from "$lib/types";
  import { searchWinget, showWinget } from "$lib/api";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import TextInput from "./TextInput.svelte";
  import Icon from "./Icon.svelte";
  import Disclosure from "./Disclosure.svelte";
  import Select from "./Select.svelte";

  let {
    open,
    product,
    onsave,
    oncancel,
    onerror,
    error = "",
  }: {
    open: boolean;
    product: Product | null;
    onsave: (product: Product) => void | Promise<void>;
    oncancel: () => void;
    onerror: (message: string) => void;
    error?: string;
  } = $props();

  let name = $state("");
  let wingetId = $state("");
  let installHint = $state("");
  let installDir = $state("");
  let env = $state<EnvWiring[]>([]);
  let saving = $state(false);
  let advancedOpen = $state(false);

  // Winget ID authoring: the live registry search is the default for new
  // products; editing shows the existing ID as-is (never a network call to
  // reopen a product) with search one quiet link away.
  let searchMode = $state(true);
  let query = $state("");
  let matches = $state<WingetMatch[]>([]);
  let searching = $state(false);
  let searched = $state(false);
  let searchFailed = $state(false);
  let chosen: WingetMatch | null = $state(null);
  let registry: WingetShow | null = $state(null);

  let searchInput: HTMLInputElement | undefined = $state();
  let chosenClear: HTMLButtonElement | undefined = $state();
  let searchSeq = 0;
  let debounce: ReturnType<typeof setTimeout>;

  $effect(() => {
    if (open) {
      name = product?.name ?? "";
      wingetId = product?.winget_id ?? "";
      installHint = product?.install_location_hint ?? "";
      installDir = product?.install_dir ?? "";
      env = product ? product.default_env.map((e) => ({ ...e })) : [];
      saving = false;
      advancedOpen = false;
      registry = null;
      chosen = null;
      searchSeq += 1;
      clearTimeout(debounce);
      if (product) {
        searchMode = false;
        query = wingetId;
        matches = [];
        searching = false;
        searched = false;
        searchFailed = false;
      } else {
        searchMode = true;
        query = "";
        matches = [];
        searching = false;
        searched = false;
        searchFailed = false;
      }
    }
  });

  function onSearchInput(value: string) {
    query = value;
    if (chosen) {
      chosen = null;
      registry = null;
      wingetId = "";
    }
    clearTimeout(debounce);
    const q = value.trim();
    if (q.length < 2) {
      searchSeq += 1;
      matches = [];
      searching = false;
      return;
    }
    const seq = ++searchSeq;
    searching = true;
    debounce = setTimeout(async () => {
      try {
        const rows = await searchWinget(q);
        if (seq !== searchSeq) return;
        matches = rows;
        searching = false;
        searchFailed = false;
        searched = true;
      } catch {
        if (seq !== searchSeq) return;
        matches = [];
        searching = false;
        searchFailed = true;
        searched = true;
      }
    }, 300);
  }

  function onSearchKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      const first = document.querySelector<HTMLElement>(".match");
      if (first) {
        event.preventDefault();
        first.focus();
      }
    }
    if (event.key === "Escape" && (chosen || query)) {
      event.preventDefault();
      event.stopPropagation();
      if (chosen) {
        clearChosen();
      } else {
        clearTimeout(debounce);
        query = "";
        matches = [];
        searching = false;
      }
    }
  }

  async function pick(match: WingetMatch) {
    chosen = match;
    wingetId = match.id;
    query = match.id;
    matches = [];
    searching = false;
    if (!name.trim()) name = match.name;
    // The clicked row unmounts — hand focus to the chip's clear button so
    // it never falls out of the dialog.
    requestAnimationFrame(() => chosenClear?.focus());
    const seq = ++searchSeq;
    try {
      const info = await showWinget(match.id);
      if (seq === searchSeq) registry = info;
    } catch {
      // The match stands on its own — enrichment is a nicety, never a block.
    }
  }

  function clearChosen() {
    chosen = null;
    registry = null;
    wingetId = "";
    query = "";
    matches = [];
    searching = false;
    searched = false;
    searchInput?.focus();
  }

  function registryLine(): string {
    const parts: string[] = [];
    if (registry?.publisher) parts.push(registry.publisher);
    const version = registry?.version ?? chosen?.version;
    if (version) parts.push(version);
    const source = registry?.source ?? chosen?.source;
    if (source) parts.push(source);
    if (registry?.moniker) parts.push(`moniker ${registry.moniker}`);
    return parts.join(" · ");
  }

  function goManual() {
    searchMode = false;
    if (!chosen) wingetId = query;
    clearTimeout(debounce);
    matches = [];
    searching = false;
    requestAnimationFrame(() =>
      document.getElementById("product-winget")?.focus()
    );
  }

  function goSearch() {
    searchMode = true;
    onSearchInput(wingetId);
    searchInput?.focus();
  }

  function setEnv(i: number, patch: Partial<EnvWiring>) {
    env[i] = { ...env[i], ...patch };
    env = env;
  }

  function addEnv() {
    env = [...env, { action: "set" as EnvAction, name: "", value: "" }];
  }

  function removeEnv(i: number) {
    env = env.filter((_, idx) => idx !== i);
  }

  async function submit() {
    const id = product?.id ?? slugify(name);
    if (!name.trim()) {
      onerror("Product name is required.");
      return;
    }
    if (!id.trim()) {
      onerror("Product id is required — give the product a name first.");
      return;
    }
    const envRows = env.filter((e) => e.name.trim() || e.value.trim());
    if (envRows.some((e) => !e.name.trim() || !e.value.trim())) {
      onerror("Every env wiring entry needs both a variable name and a value.");
      return;
    }
    saving = true;
    try {
      await onsave({
        id: id.trim(),
        name: name.trim(),
        winget_id: wingetId.trim() ? wingetId.trim() : null,
        install_location_hint: installHint.trim() ? installHint.trim() : null,
        install_dir: installDir.trim() ? installDir.trim() : null,
        default_env: envRows.map((e) => ({
          action: e.action,
          name: e.name.trim(),
          value: e.value.trim(),
        })),
        created_at: null,
        updated_at: null,
      });
    } finally {
      saving = false;
    }
  }

  function slugify(value: string): string {
    return value
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 40);
  }
</script>

<Dialog
  {open}
  title={product ? `Edit ${product.name}` : "Add a product"}
  onclose={oncancel}
  width={560}
>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <TextInput
      id="product-name"
      label="Name"
      required
      placeholder="e.g. DBeaver"
      value={name}
      onchange={(v) => (name = v)}
      autofocus
    />

    {#if searchMode}
      <div class="field">
        <label class="field__label" for="product-winget-search">winget ID</label>
        <div class="search">
          <span class="search__icon" aria-hidden="true"><Icon name="search" size={14} /></span>
          <input
            bind:this={searchInput}
            id="product-winget-search"
            class="search__input"
            type="search"
            placeholder="type to search the registry…"
            autocomplete="off"
            value={query}
            oninput={(e) => onSearchInput((e.target as HTMLInputElement).value)}
            onkeydown={onSearchKeydown}
          />
        </div>
        <p class="field__hint">
          {chosen
            ? "Picked from the registry; this ID drives the install step."
            : "Live search of the winget registry; picking a match fills the ID."}
        </p>

        {#if chosen}
          <div class="chosen">
            <div class="chosen__meta">
              <p class="chosen__id mono">{chosen.id}</p>
              {#if registryLine()}
                <p class="chosen__reg">{registryLine()}</p>
              {/if}
            </div>
            <button
              bind:this={chosenClear}
              type="button"
              class="chosen__clear"
              aria-label="Search again"
              title="Search again"
              onclick={clearChosen}
            >
              <Icon name="x" size={12} />
            </button>
          </div>
        {:else if searching}
          <p class="search-status" aria-live="polite">Searching…</p>
        {:else if matches.length > 0}
          <ul class="matches" role="listbox" aria-label="winget matches">
            {#each matches as m (m.id)}
              <li class="matches__item">
                <button
                  type="button"
                  role="option"
                  aria-selected="false"
                  class="match"
                  onclick={() => pick(m)}
                >
                  <span class="match__name">{m.name}</span>
                  <span class="match__id mono">{m.id}</span>
                  <span class="match__meta mono">
                    {m.version}{m.source ? ` · ${m.source}` : ""}
                  </span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}

        {#if searchFailed}
          <p class="search-status">The registry is out of reach right now.</p>
        {/if}

        {#if !searching && searched && matches.length === 0 && !chosen}
          <p class="fallback">
            Not found?
            <button type="button" class="fallback__link" onclick={goManual}>
              Type the ID manually
            </button>
          </p>
        {/if}
      </div>
    {:else}
      <div class="field">
        <TextInput
          id="product-winget"
          label="winget ID"
          placeholder="e.g. DBeaver.DBeaver.Community"
          value={wingetId}
          onchange={(v) => (wingetId = v)}
          hint="Leave empty for custom install steps that are not winget-managed."
        />
        <p class="fallback">
          <button type="button" class="fallback__link" onclick={goSearch}>
            Search the winget registry instead
          </button>
        </p>
      </div>
    {/if}

    <div class="advanced">
      <Disclosure
        open={advancedOpen}
        controls="product-advanced-body"
        label="Advanced"
        onclick={() => (advancedOpen = !advancedOpen)}
      />

      <div id="product-advanced-body" class="advanced__body" hidden={!advancedOpen}>
        <TextInput
          id="product-hint"
          label="Install location hint"
          placeholder="e.g. Eclipse Temurin"
          value={installHint}
          onchange={(v) => (installHint = v)}
          info="How the install location hint works"
        >
          {#snippet infobody()}
            <p>Helps Sprout find the install folder when environment variables point at it.</p>
          {/snippet}
        </TextInput>

        <TextInput
          id="product-install-dir"
          label="Install directory"
          placeholder="e.g. D:\Apps"
          value={installDir}
          onchange={(v) => (installDir = v)}
          info="How install directory works"
          infotone="warn"
        >
          {#snippet infobody()}
            <p>Overrides the default install directory from Settings for this product only.</p>
            <p>
              Many installers ignore location flags, so this does not guarantee installation in a
              specified drive.
            </p>
          {/snippet}
        </TextInput>

        <div class="env">
          <p class="env__title">Environment variables</p>
          {#each env as row, i (i)}
            <div class="env__row">
              <Select
                variant="compact"
                value={row.action}
                onchange={(v) => setEnv(i, { action: v as EnvAction })}
              >
                <option value="set">set</option>
                <option value="prepend">prepend</option>
              </Select>
              <input
                class="env__name"
                type="text"
                placeholder="Variable name"
                aria-label={`Env wiring ${i + 1} variable name`}
                value={row.name}
                oninput={(e) => setEnv(i, { name: (e.target as HTMLInputElement).value })}
              />
              <input
                class="env__value"
                type="text"
                placeholder="Value"
                aria-label={`Env wiring ${i + 1} value`}
                value={row.value}
                oninput={(e) => setEnv(i, { value: (e.target as HTMLInputElement).value })}
              />
              <button
                type="button"
                class="env__remove"
                aria-label={`Remove env wiring ${i + 1}`}
                onclick={() => removeEnv(i)}
              >
                <Icon name="x" size={12} />
              </button>
            </div>
          {/each}
          <button type="button" class="env__add" onclick={addEnv}>
            <Icon name="plus" size={12} /> Add variable
          </button>
        </div>
      </div>
    </div>

    {#if error}
      <p class="form__error" role="alert">{error}</p>
    {/if}

    <div class="form__actions">
      <Button variant="secondary" onclick={oncancel} disabled={saving}>Cancel</Button>
      <Button kind="submit" disabled={saving}>
        {saving ? "Saving…" : product ? "Save changes" : "Add product"}
      </Button>
    </div>
  </form>
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .field__label {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .search {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search__icon {
    position: absolute;
    left: 10px;
    display: inline-flex;
    color: var(--text-muted);
    pointer-events: none;
  }

  .search__input {
    width: 100%;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 8px 32px 8px 32px;
    transition: border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .search__input::placeholder {
    color: var(--text-muted);
    opacity: 0.75;
  }

  .search__input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .search__input::-webkit-search-cancel-button {
    display: none;
  }

  .field__hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .search-status {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .matches {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    max-height: 220px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-page);
  }

  .match {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr) auto;
    gap: var(--space-3);
    align-items: baseline;
    width: 100%;
    text-align: left;
    border: none;
    background: transparent;
    padding: 7px 10px;
    cursor: pointer;
  }

  .match:hover,
  .match:focus-visible {
    background: var(--bg-hover);
  }

  .match__name {
    font-size: var(--text-sm);
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .match__id {
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .match__meta {
    font-size: var(--text-xs);
    color: var(--text-faint);
    white-space: nowrap;
  }

  .chosen {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    border: 1px solid var(--accent-tint-border);
    background: var(--accent-tint);
    border-radius: var(--radius);
    padding: 8px 10px;
  }

  .chosen__meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .chosen__id {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--accent);
    overflow-wrap: anywhere;
  }

  .chosen__reg {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .chosen__clear {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex: none;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    margin-left: auto;
  }

  .chosen__clear:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .fallback {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .fallback__link {
    border: none;
    background: transparent;
    padding: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .fallback__link:hover {
    color: var(--accent-hover);
  }

  .form__error {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .advanced {
    display: flex;
    flex-direction: column;
  }

  .advanced__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding-top: var(--space-3);
    border-top: 1px dashed var(--border);
  }

  /* `hidden` would lose to the `display: flex` above (author rules beat the
     UA's [hidden] rule), which kept the panel permanently open. */
  .advanced__body[hidden] {
    display: none;
  }

  .env {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .env__title {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .env__row {
    display: grid;
    grid-template-columns: 86px minmax(0, 1fr) minmax(0, 1.4fr) 26px;
    gap: var(--space-2);
    align-items: center;
  }

  .env__name,
  .env__value {
    min-width: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    padding: 6px 8px;
  }

  .env__name:focus,
  .env__value:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .env__remove {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .env__remove:hover {
    background: var(--bg-hover);
    color: var(--danger-text);
  }

  .env__add {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: none;
    background: transparent;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
    cursor: pointer;
    padding: 4px 2px;
  }

  .env__add:hover {
    color: var(--accent-hover);
  }

  .form__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .mono {
    font-family: var(--font-mono);
  }
</style>