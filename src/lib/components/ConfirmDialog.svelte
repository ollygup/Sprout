<script lang="ts">
  import type { Snippet } from "svelte";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";

  let {
    open,
    title,
    confirmLabel,
    onconfirm,
    oncancel,
    children,
    danger = false,
    confirmDisabled = false,
  }: {
    open: boolean;
    title: string;
    confirmLabel: string;
    onconfirm: () => void;
    oncancel: () => void;
    children: Snippet;
    danger?: boolean;
    /** Disables the confirm action while its precondition is unmet —
     *  e.g. an export dialog with every collection unticked. */
    confirmDisabled?: boolean;
  } = $props();
</script>

<Dialog
  {open}
  {title}
  onclose={oncancel}
  width={400}
  focusTarget={danger ? ".confirm__danger" : undefined}
>
  <div class="confirm">
    <div class="confirm__body">
      {@render children()}
    </div>
    <div class="confirm__actions">
      <Button variant="secondary" onclick={oncancel}>Cancel</Button>
      <Button
        class="confirm__danger"
        variant={danger ? "danger" : "primary"}
        onclick={onconfirm}
        disabled={confirmDisabled}
      >
        {confirmLabel}
      </Button>
    </div>
  </div>
</Dialog>

<style>
  .confirm {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .confirm__body {
    font-size: var(--text-sm);
    color: var(--text);
  }

  .confirm__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
