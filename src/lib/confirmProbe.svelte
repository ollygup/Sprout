<script lang="ts">
  import ConfirmDialog from "./components/ConfirmDialog.svelte";

  let {
    onconfirm,
    oncancel,
  }: {
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();
</script>

<ConfirmDialog open title="Remove?" confirmLabel="Remove" {onconfirm} {oncancel}>
  <p>Body text</p>
</ConfirmDialog>
