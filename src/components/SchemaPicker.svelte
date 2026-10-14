<script lang="ts">
  import { onMount } from 'svelte';
  import { X } from '@lucide/svelte';
  let {
    schemas,
    current,
    busy,
    onselect,
    onclose,
  }: {
    schemas: string[];
    current: string;
    busy: boolean;
    onselect: (schema: string) => void;
    onclose: () => void;
  } = $props();
  let search = $state('');
  let dialog: HTMLDialogElement;
  onMount(() => {
    dialog.showModal();
    dialog.querySelector('input')?.focus();
  });
</script>

<dialog
  bind:this={dialog}
  class="connection-dialog schema-picker"
  aria-label="Select schema"
  oncancel={(e) => {
    e.preventDefault();
    onclose();
  }}
>
  <div class="manager-heading">
    <h2>Select schema</h2>
    <button class="icon-button" aria-label="Close schema picker" onclick={onclose}
      ><X size={18} /></button
    >
  </div>
  <input
    class="schema-search"
    aria-label="Search schemas"
    placeholder="Find a schema…"
    bind:value={search}
  />
  <div class="schema-options">
    {#each schemas.filter((s) => s.toLowerCase().includes(search.toLowerCase())) as schema}<button
        class="secondary-button"
        class:current={schema === current}
        onclick={() => onselect(schema)}
        disabled={busy}>{schema}{schema === current ? ' ✓' : ''}</button
      >{/each}
  </div>
  {#if !schemas.length}<p class="muted">No accessible application schemas.</p>{/if}
</dialog>
