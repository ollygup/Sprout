<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { getSettings, setCompanionUrl, setCompanionUrlList } from "$lib/api";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import Button from "$lib/components/Button.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import Dialog from "$lib/components/Dialog.svelte";
  import Badge from "$lib/components/Badge.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";

  // Ticket 125 companion manager — machine-local only, never in Preset exports/backups beyond settings row (ADR-0009 spirit).
  // Reuses PageHeader / Dialog / IconButton (research 0006 visibility-on-surface vs configuration-elsewhere,
  // 0008 page-features never, 0004 content-gated). Saved URLs: add/edit/remove, reorder via ordered_list discipline
  // (position-preserving), dedup trimmed case-insensitive on host+path.

  let urls = $state<string[]>([]);
  let activeUrl: string | null = $state(null);
  let editIndex: number | null = $state(null);
  let siteDraft = $state("");
  let formOpen = $state(false);
  let formError = $state("");
  let removeIndex: number | null = $state(null);
  let loading = $state(true);
  let error = $state("");
  let notice = $state("");
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function normalizeList(list: string[]): string[] {
    const seen = new Set<string>();
    const out: string[] = [];
    for (const raw of list) {
      const trimmed = raw.trim();
      if (!trimmed) continue;
      if (!trimmed.toLowerCase().startsWith("https://")) continue;
      const key = trimmed.toLowerCase().replace(/\/+$/, "");
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(trimmed);
    }
    return out;
  }
  function validateInput(url: string): string | null {
    const t = url.trim();
    if (!t) return "Enter an https:// URL.";
    if (!t.toLowerCase().startsWith("https://")) return "Companion URL must be https://";
    if (t.includes(" ")) return "Companion URL must be a valid https:// URL.";
    return null;
  }
  function flash(msg: string) {
    notice = msg;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = ""), 3200);
  }

  onMount(() => {
    void load();
    return () => clearTimeout(noticeTimer);
  });

  async function load() {
    loading = true;
    try {
      const s = await getSettings();
      urls = normalizeList(s.companion_url_list ?? []);
      activeUrl = s.companion_url ?? null;
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function persistList(next: string[]) {
    const normalized = normalizeList(next);
    await setCompanionUrlList(normalized);
    urls = normalized;
    // If active disappeared, clear it
    if (activeUrl && !normalized.some((u) => u.toLowerCase() === activeUrl!.toLowerCase())) {
      activeUrl = null;
      await setCompanionUrl(null);
    }
  }

  async function addUrl() {
    const err = validateInput(siteDraft);
    if (err) { formError = err; return; }
    const trimmed = siteDraft.trim();
    if (urls.some((u) => u.toLowerCase() === trimmed.toLowerCase())) { formError = `"${trimmed}" is already saved.`; return; }
    const next = [...urls, trimmed];
    try {
      await persistList(next);
      siteDraft = "";
      formOpen = false;
      formError = "";
      error = "";
      flash(`Added ${trimmed}`);
    } catch (e) { formError = String(e); }
  }
  function startEdit(idx: number) {
    editIndex = idx;
    siteDraft = urls[idx] ?? "";
    formError = "";
    formOpen = true;
  }
  function cancelEdit() {
    formOpen = false;
    editIndex = null;
    siteDraft = "";
    formError = "";
  }
  function startAdd() {
    editIndex = null;
    siteDraft = "";
    formError = "";
    formOpen = true;
  }
  async function saveEdit() {
    if (editIndex === null) return;
    const err = validateInput(siteDraft);
    if (err) { formError = err; return; }
    const trimmed = siteDraft.trim();
    if (urls.some((u, i) => i !== editIndex && u.toLowerCase() === trimmed.toLowerCase())) { formError = `"${trimmed}" is already saved.`; return; }
    const wasActive = activeUrl && urls[editIndex!].toLowerCase() === activeUrl!.toLowerCase();
    const next = [...urls];
    next[editIndex!] = trimmed;
    try {
      await persistList(next);
      if (wasActive) {
        await setCompanionUrl(trimmed);
        activeUrl = trimmed;
      }
      editIndex = null;
      siteDraft = "";
      formOpen = false;
      formError = "";
      error = "";
      flash("Saved.");
    } catch (e) { formError = String(e); }
  }
  async function removeUrl(idx: number) {
    const removed = urls[idx];
    const next = urls.filter((_, i) => i !== idx);
    try {
      await persistList(next);
      if (activeUrl && removed.toLowerCase() === activeUrl.toLowerCase()) {
        activeUrl = null;
      }
      flash(`Removed ${removed}`);
    } catch (e) { error = String(e); }
  }
  async function moveUrl(idx: number, dir: -1 | 1) {
    const target = idx + dir;
    if (target < 0 || target >= urls.length) return;
    const next = [...urls];
    const tmp = next[idx];
    next[idx] = next[target];
    next[target] = tmp;
    try {
      await persistList(next);
    } catch (e) { error = String(e); }
  }
</script>

<svelte:head>
  <title>Companion — Sprout</title>
</svelte:head>

<section class="companion" aria-labelledby="companion-title">
  <PageHeader titleId="companion-title" title="Companion">
    {#snippet actions()}
      <Button variant="secondary" onclick={() => goto("/settings")}>Back to Settings</Button>
      <Button onclick={startAdd}>Add site</Button>
    {/snippet}
    {#snippet subtitle()}
      Add, edit, and arrange the sites available to Companion. Companion data stays on this PC.
    {/snippet}
  </PageHeader>

  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}
  {#if notice}
    <Notice tone="ok">{notice}</Notice>
  {/if}

  {#if loading}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else}
    <section class="saved-sites" aria-labelledby="saved-sites-title">
      <div class="saved-sites__header">
        <div>
          <h2 id="saved-sites-title" class="saved-sites__title">Saved sites</h2>
          <p class="saved-sites__hint">The first site is your quickest pick; reorder the list anytime.</p>
        </div>
        <span class="saved-sites__count">{urls.length}</span>
      </div>
      {#if urls.length > 0}
        <ul class="saved-list">
          {#each urls as url, idx (url)}
            <li class="saved-row">
              <span class="saved-row__url" title={url}>{url}</span>
              {#if activeUrl && activeUrl.toLowerCase() === url.toLowerCase()}
                <Badge tone="accent">Active</Badge>
              {/if}
              <Button variant="ghost" onclick={() => startEdit(idx)}>Edit</Button>
              <IconButton icon="chevron-up" label="Move up" quiet onclick={() => moveUrl(idx, -1)} disabled={idx===0} />
              <IconButton icon="chevron-down" label="Move down" quiet onclick={() => moveUrl(idx, 1)} disabled={idx===urls.length-1} />
              <Button variant="ghost" onclick={() => (removeIndex = idx)}>Remove</Button>
            </li>
          {/each}
        </ul>
      {:else}
        <EmptyState icon="monitor" title="No companion URLs yet">
          <p>Add an https:// site to make it available in the dock.</p>
        </EmptyState>
      {/if}
    </section>
  {/if}
</section>

<Dialog open={formOpen} title={editIndex === null ? "Add companion site" : "Edit companion site"} onclose={cancelEdit}>
  <form
    class="site-form"
    onsubmit={(event) => {
      event.preventDefault();
      if (editIndex === null) void addUrl();
      else void saveEdit();
    }}
  >
    <label class="site-form__label" for="companion-site-url">Site URL</label>
    <input
      id="companion-site-url"
      name="companion-site-url"
      class="site-form__input"
      type="text"
      inputmode="url"
      autocomplete="off"
      spellcheck="false"
      placeholder="https://music.youtube.com…"
      value={siteDraft}
      oninput={(event) => (siteDraft = (event.target as HTMLInputElement).value)}
      aria-describedby={formError ? "companion-site-error" : "companion-site-hint"}
    />
    {#if formError}
      <p id="companion-site-error" class="site-form__error" role="alert">{formError}</p>
    {:else}
      <p id="companion-site-hint" class="site-form__hint">Use the full https:// address.</p>
    {/if}
    <div class="site-form__actions">
      <Button variant="ghost" type="button" onclick={cancelEdit}>Cancel</Button>
      <Button kind="submit">{editIndex === null ? "Add site" : "Save changes"}</Button>
    </div>
  </form>
</Dialog>

<ConfirmDialog
  open={removeIndex !== null}
  title="Remove saved site?"
  confirmLabel="Remove site"
  danger
  oncancel={() => (removeIndex = null)}
  onconfirm={() => {
    const index = removeIndex;
    removeIndex = null;
    if (index !== null) void removeUrl(index);
  }}
>
  <p>This removes the site from Companion. You can add it again later.</p>
</ConfirmDialog>

<style>
  .companion {
    max-width: 1080px;
    margin: 0 auto;
  }
  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }
  .site-form__label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }
  .saved-sites__hint,
  .site-form__hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .site-form__input {
    width: 100%;
    font-family: var(--font-mono);
    font-size: var(--text-base);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: var(--space-2) var(--space-3);
    text-align: left;
    font-variant-ligatures: none;
  }
  .site-form__input:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }
  .saved-sites__count {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }
  .saved-sites {
    margin-top: var(--space-6);
  }
  .saved-sites__header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin-bottom: var(--space-3);
  }
  .saved-sites__title {
    margin: 0 0 var(--space-1);
    font-family: var(--font-display);
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--text);
  }
  .saved-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .saved-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    padding: var(--space-3);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .saved-row__url {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-variant-ligatures: none;
  }
  .site-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .site-form__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
  .site-form__error {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--danger-text);
  }
</style>
