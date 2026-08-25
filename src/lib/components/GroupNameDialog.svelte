<script lang="ts">
  import type { GroupNaming } from "$lib/collectionGroups.svelte";
  import Button from "./Button.svelte";
  import Dialog from "./Dialog.svelte";
  import Notice from "./Notice.svelte";
  import TextInput from "./TextInput.svelte";

  /** The create/rename group dialog (ticket 95): one markup for every
   *  collection page — title flips with the mode, the input id stays unique
     per page so labels never collide. State lives in the page's
     collectionGroups manager; this component only renders it. */
  let {
    naming,
    draft,
    error,
    saving,
    inputId,
    placeholder,
    ondraft,
    onsubmit,
    onclose,
  }: {
    naming: GroupNaming | null;
    draft: string;
    error: string;
    saving: boolean;
    inputId: string;
    placeholder: string;
    ondraft: (value: string) => void;
    onsubmit: () => void;
    onclose: () => void;
  } = $props();
</script>

<Dialog
  open={naming !== null}
  title={naming?.mode === "rename" ? "Rename group" : "New group"}
  {onclose}
  width={380}
>
  <form
    class="name-form"
    onsubmit={(e) => {
      e.preventDefault();
      onsubmit();
    }}
  >
    <TextInput
      label="Name"
      id={inputId}
      value={draft}
      {placeholder}
      required
      onchange={ondraft}
    />
    {#if error}
      <Notice tone="error">{error}</Notice>
    {/if}
    <div class="name-form__buttons">
      <Button variant="secondary" onclick={onclose}>
        Cancel
      </Button>
      <Button kind="submit" disabled={saving}>
        {naming?.mode === "rename" ? "Rename" : "Create"}
      </Button>
    </div>
  </form>
</Dialog>

<style>
  .name-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .name-form__buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
