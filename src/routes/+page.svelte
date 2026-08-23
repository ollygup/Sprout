<script lang="ts">
  import type { Product } from "$lib/types";
  import { listProducts, createProduct, updateProduct, deleteProduct, productPresetImpact } from "$lib/api";
  import { goto } from "$app/navigation";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";
  import ProductPacket from "$lib/components/ProductPacket.svelte";
  import ProductFormDialog from "$lib/components/ProductFormDialog.svelte";
  import ProductDetailsDialog from "$lib/components/ProductDetailsDialog.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import ContextMenu, {
    type ContextMenuState,
    type MenuRequest,
  } from "$lib/components/ContextMenu.svelte";

  let products = $state<Product[]>([]);
  let query = $state("");
  let loading = $state(true);
  let loadFailed = $state(false);
  let error = $state("");
  let notice = $state("");
  let formOpen = $state(false);
  let editing: Product | null = $state(null);
  let deleting: Product | null = $state(null);
  let deletingImpact = $state(0);
  let details: Product | null = $state(null);
  let menu: (ContextMenuState & { productId: string }) | null = $state(null);
  let animateRack = $state(false);

  let debounce: ReturnType<typeof setTimeout>;

  $effect(() => {
    const q = query;
    clearTimeout(debounce);
    debounce = setTimeout(() => load(q), 120);
  });

  async function load(q: string) {
    loading = true;
    try {
      const rows = await listProducts(q.trim() ? q.trim() : null);
      products = rows;
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
    error = "";
    formOpen = true;
  }

  function openEdit(product: Product) {
    editing = product;
    error = "";
    formOpen = true;
  }

  async function save(product: Product) {
    error = "";
    try {
      if (editing) {
        await updateProduct(product);
        flash(`Saved ${product.name}.`);
      } else {
        await createProduct(product);
        flash(`Added ${product.name} to the library.`);
        animateRack = true;
      }
      formOpen = false;
      await load(query);
    } catch (e) {
      console.error(e);
      error =
        String(e) ||
        (editing
          ? `Couldn't save ${product.name}. Please try again.`
          : "Couldn't add that product. Please try again.");
    }
  }

  async function confirmDelete() {
    if (!deleting) return;
    const name = deleting.name;
    try {
      await deleteProduct(deleting.id);
      flash(`Removed ${name} from the library.`);
      deleting = null;
      await load(query);
    } catch (e) {
      console.error(e);
      error = `Couldn't remove ${name}. Please try again.`;
    }
  }

  function closeMenu() {
    menu = null;
  }

  function openProductMenu(product: Product, request: MenuRequest) {
    // The ⋯ button and Enter on a focused card toggle; right-click re-positions.
    if (request.kind === "anchor" && menu?.productId === product.id) {
      closeMenu();
      return;
    }
    menu = {
      productId: product.id,
      open: true,
      label: `Actions for ${product.name}`,
      focusFirst: request.kind === "anchor" ? request.focusFirst : false,
      returnTo: request.returnTo,
      ...(request.kind === "cursor"
        ? { x: request.x, y: request.y }
        : { anchor: request.anchor }),
      items: [
        {
          label: "Install now",
          icon: "play",
          onselect: () =>
            goto(`/plan?quick=${encodeURIComponent(product.id)}`),
        },
        { label: "Edit", icon: "pencil", onselect: () => openEdit(product) },
        { label: "More info", icon: "info", onselect: () => (details = product) },
        {
          label: "Remove",
          icon: "trash",
          danger: true,
          onselect: () => {
            deleting = product;
            deletingImpact = 0;
            productPresetImpact(product.id)
              .then((impact) => (deletingImpact = impact.preset_count))
              .catch((e) => console.error(e));
          },
        },
      ],
    };
  }
</script>

<section class="library" aria-labelledby="library-title">
  <PageHeader titleId="library-title" title="Products">
    {#snippet actions()}
      <Button onclick={openAdd}>
        <Icon name="plus" size={15} />
        Add product
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {products.length} product{products.length === 1 ? "" : "s"}
      {query.trim() ? (products.length === 1 ? " matches your search." : " match your search.") : "."}
      Right-click a card or open its ⋯ menu for actions.
    {/snippet}
    {#snippet toolbar()}
      <SearchInput
        value={query}
        placeholder="Filter products…"
        onchange={(v) => (query = v)}
      />
    {/snippet}
  </PageHeader>

  {#if notice}
    <Notice tone="ok">{notice}</Notice>
  {/if}
  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}

  {#if loading && products.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <EmptyState icon="x" title="Couldn't read the library">
      <p>Couldn't read the product library from
        <span class="mono">%LOCALAPPDATA%\Sprout\sprout.db</span>.</p>
      <p>The file may be missing or locked by another process. Close the app, check the file,
        then relaunch.</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={() => load(query)}>Try again</Button>
      </div>
    </EmptyState>
  {:else if products.length === 0 && !query.trim()}
    <EmptyState title="No products yet">
      <p>Add the first product to start composing presets. Products come from the live winget
        registry search or a custom install step.</p>
      <div class="empty-cta">
        <Button onclick={openAdd}>
          <Icon name="plus" size={15} />
          Add product
        </Button>
      </div>
    </EmptyState>
  {:else if products.length === 0}
    <EmptyState title={`Nothing matches “${query.trim()}”`}>
      <p>Try a different name or winget ID, or clear the search to browse the whole library.</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={() => (query = "")}>Clear search</Button>
      </div>
    </EmptyState>
  {:else}
    <ul class="rack" class:animate={animateRack}>
      {#each products as product, i (product.id)}
        <li class="rack__cell">
          <ProductPacket
            {product}
            index={i}
            animate={animateRack}
            expanded={menu?.productId === product.id}
            onmenu={(request) => openProductMenu(product, request)}
            oninfo={() => (details = product)}
          />
        </li>
      {/each}
    </ul>
  {/if}
</section>

<ProductFormDialog
  open={formOpen}
  product={editing}
  error={error}
  onsave={save}
  oncancel={() => (formOpen = false)}
  onerror={(message) => (error = message)}
/>

<ProductDetailsDialog open={details !== null} product={details} onclose={() => (details = null)} />

<ConfirmDialog
  open={deleting !== null}
  title="Remove product?"
  confirmLabel="Remove"
  danger
  onconfirm={confirmDelete}
  oncancel={() => (deleting = null)}
>
  <p>
    <strong>{deleting?.name}</strong> ({deleting?.winget_id ?? "custom install step"}) will be
    removed from the library.
    {#if deletingImpact > 0}
      It will also be removed from {deletingImpact} preset{deletingImpact === 1 ? "" : "s"} that
      contain it. Imported presets keep their own embedded copy.
    {:else}
      No preset references it.
    {/if}
  </p>
</ConfirmDialog>

<ContextMenu ctx={menu} onclose={closeMenu} />

<style>
  .library {
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