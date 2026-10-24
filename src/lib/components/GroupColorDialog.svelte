<script lang="ts">
  import type { Group } from "$lib/types";
  import { GROUP_COLORS } from "$lib/groupColor";
  import { t } from "$lib/copy";
  import Button from "./Button.svelte";
  import Dialog from "./Dialog.svelte";
  import Notice from "./Notice.svelte";
  import TextInput from "./TextInput.svelte";

  /** The group color dialog (ticket 225): ten curated swatches, a custom
   *  #RRGGBB field, and "No color". Choosing a swatch or No color applies
   *  immediately and closes; the custom field applies on submit. State lives
   *  in the page's collectionGroups manager — this component only renders it
   *  inside the shared Dialog contract (focus trap, Escape cancels). */
  let {
    group,
    draft,
    error,
    saving,
    inputId,
    onpick,
    ondraft,
    onsubmit,
    onclose,
  }: {
    group: Group | null;
    draft: string;
    error: string;
    saving: boolean;
    inputId: string;
    onpick: (color: string | null) => void;
    ondraft: (value: string) => void;
    onsubmit: () => void;
    onclose: () => void;
  } = $props();
</script>

<Dialog
  open={group !== null}
  title={t("groups.colorTitle")}
  {onclose}
  width={360}
>
  <div class="color-form">
    <p class="color-form__hint">{t("groups.colorHint")}</p>
    <div class="swatches" role="group" aria-label={t("groups.colorSwatches")}>
      {#each GROUP_COLORS as swatch (swatch.hex)}
        <button
          type="button"
          class="swatch"
          class:swatch--current={group?.color === swatch.hex}
          aria-label={t(swatch.labelKey)}
          aria-pressed={group?.color === swatch.hex}
          style:--swatch-color={swatch.hex}
          disabled={saving}
          onclick={() => onpick(swatch.hex)}
        ></button>
      {/each}
    </div>
    <form
      class="color-form__custom"
      onsubmit={(e) => {
        e.preventDefault();
        onsubmit();
      }}
    >
      <TextInput
        label={t("groups.colorCustom")}
        id={inputId}
        value={draft}
        placeholder="#1a7f64"
        hint={t("groups.colorCustomHint")}
        onchange={ondraft}
      />
      {#if error}
        <Notice tone="error">{error}</Notice>
      {/if}
      <div class="color-form__buttons">
        <Button variant="secondary" disabled={saving} onclick={() => onpick(null)}>
          {t("groups.colorNone")}
        </Button>
        <Button kind="submit" disabled={saving}>
          {t("groups.colorApply")}
        </Button>
      </div>
    </form>
  </div>
</Dialog>

<style>
  .color-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .color-form__hint {
    margin: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-body);
    color: var(--text-muted);
  }

  /* A 5 × 2 grid keeps all ten hues on one screen at the dialog's width. */
  .swatches {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: var(--space-2);
  }

  .swatch {
    width: 100%;
    aspect-ratio: 1;
    min-height: 32px;
    border: none;
    border-radius: var(--radius);
    background: var(--swatch-color);
    cursor: pointer;
    /* The current choice gets a neutral token ring, never a second color:
       the hue itself is the state, the ring is the selection grammar. */
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }

  .swatch--current {
    box-shadow:
      inset 0 0 0 1px var(--border-strong),
      0 0 0 2px var(--bg-surface),
      0 0 0 4px var(--text);
  }

  .swatch:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .color-form__custom {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .color-form__buttons {
    display: flex;
    justify-content: space-between;
    gap: var(--space-2);
  }
</style>
