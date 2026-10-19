<script lang="ts">
  import { onMount } from 'svelte';
  let {
    name,
    onsave,
    ondiscard,
    oncancel,
    busy,
  }: {
    name: string;
    onsave: () => void;
    ondiscard: () => void;
    oncancel: () => void;
    busy: boolean;
  } = $props();
  let dialog: HTMLDialogElement;
  onMount(() => dialog.showModal());
</script>

<dialog
  bind:this={dialog}
  class="connection-dialog close-dialog"
  aria-labelledby="close-title"
  oncancel={(event) => {
    event.preventDefault();
    if (!busy) oncancel();
  }}
>
  <h2 id="close-title">Unsaved changes in {name}</h2>
  <p>Save your edits and staged row deletions before closing this tab, or discard them.</p>
  <div class="dialog-footer">
    <button class="secondary-button" onclick={oncancel} disabled={busy}>Keep editing</button>
    <button class="secondary-button" onclick={ondiscard} disabled={busy}>Discard & close</button>
    <button class="primary-button" onclick={onsave} disabled={busy}>Save & close</button>
  </div>
</dialog>
