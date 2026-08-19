<script lang="ts">
  import type { EnvAction, EnvWiring, PresetRecord, Product, VerifyCommand } from "$lib/types";
  import { ComposerState } from "$lib/composerState.svelte";
  import { getSettings, listProducts } from "$lib/api";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import TextInput from "./TextInput.svelte";
  import Icon from "./Icon.svelte";
  import InfoTip from "./InfoTip.svelte";
  import Disclosure from "./Disclosure.svelte";
  import Select from "./Select.svelte";

  let {
    open,
    preset,
    error,
    onsave,
    oncancel,
    onerror,
  }: {
    open: boolean;
    preset: PresetRecord | null;
    error: string;
    onsave: (preset: PresetRecord) => void;
    oncancel: () => void;
    onerror: (message: string) => void;
  } = $props();

  let name = $state("");
  let description = $state("");
  let author = $state("");
  let version = $state("1");
  let products = $state<Product[]>([]);
  let saving = $state(false);
  let loaded = $state(false);
  let composer = new ComposerState();

  $effect(() => {
    if (open) {
      name = preset?.name ?? "";
      description = preset?.description ?? "";
      author = preset?.author ?? "";
      version = preset?.version ?? "1";
      composer.load(preset?.requirements ?? []);
      saving = false;
      onerror("");
      if (!loaded) {
        loaded = true;
        listProducts(null)
          .then((rows) => (products = rows))
          .catch((e) => onerror(String(e)));
        getSettings()
          .then((s) => (composer.defaultTimeout = s.default_timeout_minutes))
          .catch(() => {});
      }
    }
  });

  function productOption(id: string): Product | undefined {
    return products.find((p) => p.id === id);
  }

  function submit() {
    if (!name.trim()) {
      onerror("Preset name is required.");
      return;
    }
    if (!description.trim()) {
      onerror("Preset description is required — tell future you what this contains.");
      return;
    }
    const rowError = composer.firstError();
    if (rowError) {
      onerror(rowError);
      return;
    }
    saving = true;
    onsave({
      id: preset?.id ?? slugify(name),
      imported: false,
      schema_version: 1,
      platform: "windows",
      name: name.trim(),
      description: description.trim(),
      author: author.trim(),
      version: version.trim() || "1",
      requirements: composer.clean(),
    });
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
  title={preset ? `Edit ${preset.name}` : "Compose a preset"}
  onclose={oncancel}
  width={800}
>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <div class="meta">
      <TextInput
        id="preset-name"
        label="Preset name"
        required
        placeholder="e.g. Backend dev box"
        value={name}
        onchange={(v) => (name = v)}
        autofocus
      />
      <TextInput
        id="preset-desc"
        label="Description"
        required
        placeholder="e.g. Java 21, VSCode, DBeaver"
        value={description}
        onchange={(v) => (description = v)}
      />
      <div class="meta__row">
        <TextInput
          id="preset-author"
          label="Author"
          placeholder="optional"
          value={author}
          onchange={(v) => (author = v)}
        />
        <TextInput
          id="preset-version"
          label="Version"
          value={version}
          onchange={(v) => (version = v)}
        />
      </div>
    </div>

    <div class="apps">
      <div class="section-head">
        <p class="section-head__title">Applications</p>
        <InfoTip label="What an application is">
          <p>What the machine must have. Each application picks a product, a version policy,
            optional dependencies, timeout, env wiring, and verify commands.</p>
        </InfoTip>
      </div>

      {#if composer.requirements.length === 0}
        <p class="apps__none">No applications yet — a preset with nothing in it is just a name.</p>
      {/if}

      {#each composer.requirements as req, i (i)}
        {@const counts = composer.hiddenCounts(i)}
        {@const expanded = composer.expanded === i}
        <div class="app" class:app--expanded={expanded}>
          <div class="app__bar">
            {#if !req.product.id}
              <Select
                class="app__picker"
                aria-label={`Application ${i + 1} product`}
                value={req.product.id}
                onchange={(v) => {
                  const product = products.find((p) => p.id === v);
                  if (product) composer.setProduct(i, product);
                }}
              >
                <option value="" disabled>Choose a product…</option>
                {#each products.filter((p) => p.winget_id) as p (p.id)}
                  <option value={p.id}>{p.name}</option>
                {/each}
                {#if products.some((p) => !p.winget_id)}
                  <optgroup label="No winget id — custom install steps aren't supported yet">
                    {#each products.filter((p) => !p.winget_id) as p (p.id)}
                      <option value={p.id} disabled>{p.name}</option>
                    {/each}
                  </optgroup>
                {/if}
              </Select>
            {:else}
              <div class="app__name">
                <span class="app__product">{req.product.name || req.product.id}</span>
                {#if req.product.winget_id}
                  <span class="app__id">{req.product.winget_id}</span>
                {/if}
                {#if req.unresolved}
                  <span class="app__unresolved">removed from library</span>
                {/if}
              </div>
              <label class="app__field">
                <span class="app__field-label">Version policy</span>
                <Select
                  variant="small"
                  value={req.version_policy.kind}
                  onchange={(v) => composer.setPolicy(i, v as never)}
                >
                  <option value="latest" title="Upgrade to the newest version">latest</option>
                  <option value="pinned" title="Exactly this version">pinned</option>
                  <option value="present" title="Installed, never upgraded">present</option>
                </Select>
              </label>
              {#if req.version_policy.kind === "pinned"}
                <input
                  class="app__pinned"
                  type="text"
                  aria-label={`Application ${i + 1} pinned version`}
                  placeholder="e.g. 21.0.5"
                  value={req.version_policy.version}
                  oninput={(e) => composer.setPinnedVersion(i, (e.target as HTMLInputElement).value)}
                />
              {/if}
              {#if counts && !expanded}
                <span class="app__tags" aria-label={`Application ${i + 1} hidden options`}>
                  {[
                    counts.env ? `${counts.env} env` : null,
                    counts.verify ? `${counts.verify} verify` : null,
                    counts.deps ? `${counts.deps} dep` : null,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              {/if}
            {/if}
            {#if req.product.id}
              <Disclosure
                open={expanded}
                controls={`app-panel-${i}`}
                ariaLabel={`Application ${i + 1} advanced options`}
                onclick={() => composer.toggleExpand(i)}
              />
            {/if}
            <button
              type="button"
              class="app__remove"
              aria-label={`Remove application ${i + 1}`}
              onclick={() => composer.remove(i)}
            >
              <Icon name="x" size={13} />
            </button>
          </div>

          {#if req.product.id && expanded}
            <div id={`app-panel-${i}`} class="app__panel">
              <div class="panel__row">
                <label class="field field--timeout">
                  <span class="field__label">Timeout (minutes)</span>
                  <input
                    class="field__input"
                    type="number"
                    min="1"
                    value={req.timeout_minutes}
                    oninput={(e) =>
                      composer.setTimeoutMinutes(i, Number((e.target as HTMLInputElement).value))
                    }
                  />
                </label>

                <div class="panel__deps">
                  <div class="sub-row">
                    <p class="sub">Depends on</p>
                    <InfoTip label="How dependencies work">
                      <p>Order matters — a dependency installs before the application that depends
                        on it. Tick every product that must already be present.</p>
                    </InfoTip>
                  </div>
                  {#if composer.requirements.length <= 1}
                    <p class="none">No other applications in this preset to depend on.</p>
                  {:else}
                    <div class="panel__deps-list">
                      {#each composer.requirements as other, k (k)}
                        {#if other.product.id && other.product.id !== req.product.id}
                          <label class="chip">
                            <input
                              type="checkbox"
                              checked={req.depends_on.includes(other.product.id)}
                              onchange={() => composer.toggleDep(i, other.product.id)}
                            />
                            <span>{other.product.name || other.product.id}</span>
                          </label>
                        {/if}
                      {/each}
                    </div>
                  {/if}
                </div>
              </div>

              <div class="panel__section">
                <div class="sub-row">
                  <p class="sub">Env wiring</p>
                  <InfoTip label="How env wiring works">
                    <p>Environment variables written after install. Values may reference an install
                      folder with <span class="mono">&lt;InstallLocation:hint&gt;</span>.</p>
                  </InfoTip>
                </div>
                {#each req.env as row, j (j)}
                  <div class="rows">
                    <Select
                      variant="compact"
                      aria-label={`Application ${i + 1} env wiring ${j + 1} action`}
                      value={row.action}
                      onchange={(v) => composer.setEnv(i, j, { action: v as EnvAction })}
                    >
                      <option value="set">set</option>
                      <option value="prepend">prepend</option>
                    </Select>
                    <input
                      class="rows__input"
                      type="text"
                      placeholder="NAME"
                      aria-label={`Application ${i + 1} env wiring ${j + 1} variable name`}
                      value={row.name}
                      oninput={(e) =>
                        composer.setEnv(i, j, { name: (e.target as HTMLInputElement).value })
                      }
                    />
                    <input
                      class="rows__input"
                      type="text"
                      placeholder="value or <InstallLocation:hint>"
                      aria-label={`Application ${i + 1} env wiring ${j + 1} value`}
                      value={row.value}
                      oninput={(e) =>
                        composer.setEnv(i, j, { value: (e.target as HTMLInputElement).value })
                      }
                    />
                    <button
                      type="button"
                      class="rows__remove"
                      aria-label={`Remove env wiring ${j + 1} on application ${i + 1}`}
                      onclick={() => composer.removeEnv(i, j)}
                    >
                      <Icon name="x" size={12} />
                    </button>
                  </div>
                {/each}
                <button type="button" class="add" onclick={() => composer.addEnv(i)}>
                  <Icon name="plus" size={12} /> Add env wiring
                </button>
              </div>

              <div class="panel__section">
                <div class="sub-row">
                  <p class="sub">Verify commands</p>
                  <InfoTip label="How verify commands work">
                    <p>Run after install — a non-zero exit or non-matching output fails the
                      application.</p>
                  </InfoTip>
                </div>
                {#each req.verify as check, j (j)}
                  <div class="rows">
                    <input
                      class="rows__input rows__input--wide"
                      type="text"
                      placeholder="e.g. java -version"
                      aria-label={`Application ${i + 1} verify command ${j + 1}`}
                      value={check.command}
                      oninput={(e) =>
                        composer.setVerify(i, j, {
                          command: (e.target as HTMLInputElement).value,
                        })
                      }
                    />
                    <input
                      class="rows__input rows__input--wide"
                      type="text"
                      placeholder="match text in output (optional)"
                      aria-label={`Application ${i + 1} verify command ${j + 1} match text`}
                      value={check.match_text ?? ""}
                      oninput={(e) =>
                        composer.setVerify(i, j, {
                          match_text: (e.target as HTMLInputElement).value || null,
                        })
                      }
                    />
                    <button
                      type="button"
                      class="rows__remove"
                      aria-label={`Remove verify command ${j + 1} on application ${i + 1}`}
                      onclick={() => composer.removeVerify(i, j)}
                    >
                      <Icon name="x" size={12} />
                    </button>
                  </div>
                {/each}
                <button type="button" class="add" onclick={() => composer.addVerify(i)}>
                  <Icon name="plus" size={12} /> Add verify command
                </button>
              </div>
            </div>
          {/if}
        </div>
      {/each}

      <button type="button" class="add-app" onclick={() => composer.add()}>
        <Icon name="plus" size={13} /> Add application
      </button>
    </div>

    {#if error}
      <p class="form__error" role="alert">{error}</p>
    {/if}

    <div class="form__actions">
      <Button variant="secondary" onclick={oncancel} disabled={saving}>Cancel</Button>
      <Button kind="submit" disabled={saving}>
        {saving ? "Saving…" : preset ? "Save changes" : "Save preset"}
      </Button>
    </div>
  </form>
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .meta__row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 140px);
    gap: var(--space-3);
  }

  .apps {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .section-head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .section-head__title {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }

  .apps__none {
    margin: 0;
    font-size: var(--text-sm);
    font-style: italic;
    color: var(--text-muted);
  }

  .app {
    display: flex;
    flex-direction: column;
    padding: var(--space-2) 0;
  }

  /* The dashed fold line separating rows. */
  .app + .app {
    border-top: 1px dashed var(--border);
  }

  .app__bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .app__name {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
    flex: 1 1 auto;
    overflow: hidden;
  }

  .app__product {
    font-size: var(--text-base);
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .app__id {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: none;
  }

  .app__unresolved {
    font-size: var(--text-xs);
    font-style: italic;
    color: var(--danger-text);
    flex: none;
  }

  :global(.app__picker) {
    flex: 1 1 auto;
    min-width: 0;
  }

  .app__field {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex: none;
  }

  .app__field-label {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .app__pinned {
    flex: none;
    width: 110px;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    padding: 6px 8px;
  }

  .app__pinned:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .app__tags {
    flex: none;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
    white-space: nowrap;
  }

  .app__remove {
    flex: none;
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

  .app__remove:hover {
    background: var(--bg-hover);
    color: var(--danger-text);
  }

  .app__panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    margin-top: var(--space-2);
    padding: var(--space-3) var(--space-2) var(--space-1);
    border-top: 1px dashed var(--border);
  }

  .panel__row {
    display: grid;
    grid-template-columns: minmax(0, 150px) minmax(0, 1fr);
    gap: var(--space-4);
    align-items: start;
  }

  .panel__section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    border-top: 1px dashed var(--border);
    padding-top: var(--space-2);
  }

  .sub-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .sub {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .none {
    margin: 0;
    font-size: var(--text-xs);
    font-style: italic;
    color: var(--text-muted);
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

  .field__input {
    width: 100%;
    font-family: var(--font-body);
    font-size: var(--text-base);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 8px 10px;
  }

  .field__input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .panel__deps {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .panel__deps-list {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-xs);
    color: var(--text);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-pill);
    padding: 3px 9px;
    cursor: pointer;
    user-select: none;
  }

  .chip:has(input:checked) {
    border-color: var(--accent);
    background: var(--accent-tint);
  }

  .chip input {
    accent-color: var(--accent);
  }

  .rows {
    display: grid;
    grid-template-columns: 86px minmax(0, 1fr) minmax(0, 1.4fr) 26px;
    gap: var(--space-2);
    align-items: center;
  }

  .rows__input {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    padding: 6px 8px;
    min-width: 0;
  }

  .rows__input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .rows__remove {
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

  .rows__remove:hover {
    background: var(--bg-hover);
    color: var(--danger-text);
  }

  .add {
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

  .add:hover {
    color: var(--accent-hover);
  }

  .add-app {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px dashed var(--accent);
    border-radius: var(--radius);
    background: transparent;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
    cursor: pointer;
    padding: 8px 12px;
  }

  .add-app:hover {
    background: var(--accent-tint);
  }

  .form__error {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .form__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>