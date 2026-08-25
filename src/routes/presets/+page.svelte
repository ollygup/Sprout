<script lang="ts">
  import { onMount } from "svelte";
  import type { PresetRecord } from "$lib/types";
  import { listPresets, createPreset, updatePreset, deletePreset, exportPreset, importPreset } from "$lib/api";
  import { goto } from "$app/navigation";
  import { launchImport } from "$lib/launchImport.svelte";
  import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import PresetPacket from "$lib/components/PresetPacket.svelte";
  import PresetFormDialog from "$lib/components/PresetFormDialog.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import ContextMenu, {
    type ContextMenuItem,
    type ContextMenuState,
    type MenuRequest,
  } from "$lib/components/ContextMenu.svelte";

  let presets = $state<PresetRecord[]>([]);
  let loading = $state(true);
  let loadFailed = $state(false);
  let notice = $state("");
  let error = $state("");
  let importWarning = $state("");
  let importing = $state(false);
  let formError = $state("");
  let formOpen = $state(false);
  let editing: PresetRecord | null = $state(null);
  let deleting: PresetRecord | null = $state(null);
  let menu: (ContextMenuState & { presetId: string }) | null = $state(null);
  let animateRack = $state(false);

  onMount(() => {
    load();
  });

  $effect(() => {
    if (!launchImport.path) return;
    const path = launchImport.path;
    launchImport.path = null;
    doImport(path, "The preset you opened has been imported.");
  });

  async function load() {
    loading = true;
    try {
      presets = await listPresets();
      loadFailed = false;
    } catch (e) {
      console.error(e);
      loadFailed = true;
    } finally {
      loading = false;
    }
  }

  function flash(message: string) {
    notice = message;
    setTimeout(() => (notice = ""), 3200);
  }

  function openAdd() {
    editing = null;
    formError = "";
    formOpen = true;
  }

  function openEdit(record: PresetRecord) {
    if (record.imported) return;
    editing = record;
    formError = "";
    formOpen = true;
  }

  function openFork(record: PresetRecord) {
    const forked = {
      ...record,
      id: slugify(`${record.name} copy`),
      name: `${record.name} (copy)`,
      version: "1",
      imported: false,
    };
    editing = forked;
    formError = "";
    formOpen = true;
  }

  async function doImport(path: string, successMessage: string) {
    importing = true;
    try {
      const result = await importPreset(path);
      importWarning = result.warning ?? "";
      flash(
        `${successMessage} ${result.preset.name} v${result.preset.version} is in your library. Imported presets are stored as authored; fork it to edit.`,
      );
      animateRack = true;
      await load();
    } catch (e) {
      console.error(e);
      importWarning = "";
      // Import rejections are authored messages (schema version, platform,
      // duplicate product — spec user stories 21–23) and stay readable.
      error = String(e);
    } finally {
      importing = false;
    }
  }

  async function importViaDialog() {
    const picked = await open({
      title: "Import a Sprout preset",
      multiple: false,
      directory: false,
      filters: [{ name: "Sprout preset", extensions: ["sprout.json", "json"] }],
    });
    if (typeof picked !== "string") return;
    await doImport(picked, "Imported.");
  }

  async function exportViaDialog(record: PresetRecord) {
    const path = await saveDialog({
      title: `Export ${record.name} as .sprout.json`,
      defaultPath: `${record.name}.sprout.json`,
      filters: [{ name: "Sprout preset", extensions: ["sprout.json"] }],
    });
    if (!path) return;
    try {
      await exportPreset(path, record.id);
      flash(`Exported ${record.name} v${record.version} to ${path}`);
    } catch (e) {
      console.error(e);
      error = `Couldn't export ${record.name}. Please try again.`;
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

  async function save(record: PresetRecord) {
    try {
      const isEdit = !!editing && presets.some((p) => p.id === editing?.id);
      if (isEdit) {
        await updatePreset(record);
        flash(`Saved ${record.name}.`);
      } else {
        await createPreset(record);
        flash(`Added ${record.name} to your library.`);
        animateRack = true;
      }
      formOpen = false;
      await load();
    } catch (e) {
      // Domain validation and constraint messages are authored copy — they
      // tell the composer what to fix; raw infrastructure failures are rare.
      formError = String(e);
    }
  }

  async function confirmDelete() {
    if (!deleting) return;
    const name = deleting.name;
    try {
      await deletePreset(deleting.id);
      flash(`Removed ${name} from your library.`);
      deleting = null;
      await load();
    } catch (e) {
      console.error(e);
      error = `Couldn't remove ${name}. Please try again.`;
    }
  }

  function closeMenu() {
    menu = null;
  }

  function openPresetMenu(record: PresetRecord, request: MenuRequest) {
    // The ⋯ button and Enter on a focused card toggle; right-click re-positions.
    if (request.kind === "anchor" && menu?.presetId === record.id) {
      closeMenu();
      return;
    }
    const items: ContextMenuItem[] = [
      {
        label: "Plan with this",
        icon: "play",
        onselect: () => goto(`/plan?presets=${encodeURIComponent(record.name)}`),
      },
    ];
    if (!record.imported) {
      items.push({ label: "Edit", icon: "pencil", onselect: () => openEdit(record) });
    }
    items.push({
      label: record.imported ? "Fork to edit" : "Fork",
      icon: "copy",
      onselect: () => openFork(record),
    });
    items.push({ label: "Export", icon: "download", onselect: () => exportViaDialog(record) });
    // Ticket 106's ordering standard: destruction last, separated.
    items.push({ label: "", separator: true, onselect: () => {} });
    items.push({
      label: "Remove",
      icon: "trash",
      danger: true,
      onselect: () => (deleting = record),
    });
    menu = {
      presetId: record.id,
      open: true,
      label: `Actions for ${record.name}`,
      focusFirst: request.kind === "anchor" ? request.focusFirst : false,
      returnTo: request.returnTo,
      ...(request.kind === "cursor"
        ? { x: request.x, y: request.y }
        : { anchor: request.anchor }),
      items,
    };
  }
</script>

<section class="presets" aria-labelledby="presets-title">
  <PageHeader titleId="presets-title" title="Presets">
    {#snippet actions()}
      <Button variant="secondary" onclick={importViaDialog} disabled={importing}>
        <Icon name="folder" size={15} /> Import
      </Button>
      <Button onclick={openAdd}>
        <Icon name="plus" size={15} />
        Compose preset
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {presets.length} preset{presets.length === 1 ? "" : "s"}.
      Right-click a card or open its ⋯ menu to edit, fork, export, or remove.
    {/snippet}
  </PageHeader>

  {#if notice}
    <Notice tone="ok">{notice}</Notice>
  {/if}
  {#if importWarning}
    <Notice tone="warn">{importWarning}</Notice>
  {/if}
  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}

  {#if loading && presets.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <EmptyState icon="x" title="Couldn't read the library">
      <p>Couldn't read the presets from
        <span class="mono">%LOCALAPPDATA%\Sprout\sprout.db</span>.</p>
      <p>The file may be missing or locked by another process. Close the app, check the file,
        then relaunch.</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={load}>Try again</Button>
      </div>
    </EmptyState>
  {:else if presets.length === 0}
    <EmptyState title="No presets yet">
      <p>A preset is a named set of requirements that describes a machine setup. Compose one from
        the products in your library, then plan it against this machine.</p>
      <div class="empty-cta">
        <Button onclick={openAdd}><span aria-hidden="true">+</span> Compose your first preset</Button>
      </div>
    </EmptyState>
  {:else}
    <ul class="rack" class:animate={animateRack}>
      {#each presets as record, i (record.id)}
        <li class="rack__cell">
          <PresetPacket
            {record}
            index={i}
            animate={animateRack}
            expanded={menu?.presetId === record.id}
            onmenu={(request) => openPresetMenu(record, request)}
          />
        </li>
      {/each}
    </ul>
  {/if}
</section>

<PresetFormDialog
  open={formOpen}
  preset={editing}
  error={formError}
  onsave={save}
  oncancel={() => (formOpen = false)}
  onerror={(message) => (formError = message)}
/>

<ConfirmDialog
  open={deleting !== null}
  title="Remove preset?"
  confirmLabel="Remove"
  danger
  onconfirm={confirmDelete}
  oncancel={() => (deleting = null)}
>
  <p>
    <strong>{deleting?.name}</strong> (v{deleting?.version}) will be removed from the library.
    To get it back, compose it again or import its .sprout.json file.
  </p>
</ConfirmDialog>

<ContextMenu ctx={menu} onclose={closeMenu} />

<style>
  .presets {
    max-width: 1080px;
    margin: 0 auto;
  }

  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .rack {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: var(--space-4);
  }

  .rack__cell {
    min-width: 0;
  }

  .empty-cta {
    margin-top: var(--space-4);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }
</style>