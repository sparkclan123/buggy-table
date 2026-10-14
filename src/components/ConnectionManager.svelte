<script lang="ts">
  import { onMount } from 'svelte';
  import { Plus, X, Star, PlugZap, Search, Settings2, Trash2 } from '@lucide/svelte';
  import { engineLabel, hasEdits, type ConnectionWorkspace } from '../lib/connections';
  let {
    entries,
    activeKey,
    busy,
    onselect,
    onreconnect,
    onadd,
    onedit,
    onpin,
    onremove,
    onclose,
  }: {
    entries: ConnectionWorkspace[];
    activeKey: string;
    busy: boolean;
    onselect: (entry: ConnectionWorkspace) => void;
    onreconnect: (entry: ConnectionWorkspace) => void;
    onadd: () => void;
    onedit: (entry: ConnectionWorkspace) => void;
    onpin: (entry: ConnectionWorkspace) => void;
    onremove: (entry: ConnectionWorkspace) => void;
    onclose: () => void;
  } = $props();
  let search = $state('');
  let engine = $state('');
  let group = $state('');
  let page = $state(0);
  let removeKey = $state('');
  let dialog: HTMLDialogElement;
  const groups = $derived(
    [...new Set(entries.map((e) => e.saved?.group ?? '').filter(Boolean))].sort(),
  );
  const visible = $derived(
    entries
      .filter(
        (e) =>
          e.key !== 'sample' &&
          (!engine || e.info.engine === engine) &&
          (!group || e.saved?.group === group) &&
          [
            e.info.name,
            e.info.host,
            e.info.database,
            e.info.username,
            e.saved?.group,
            engineLabel(e.info.engine),
          ]
            .join(' ')
            .toLowerCase()
            .includes(search.toLowerCase()),
      )
      .sort(
        (a, b) =>
          Number(b.saved?.pinned ?? false) - Number(a.saved?.pinned ?? false) ||
          (b.saved?.lastUsed ?? 0) - (a.saved?.lastUsed ?? 0),
      ),
  );
  onMount(() => dialog.showModal());
</script>

<dialog
  bind:this={dialog}
  class="connection-manager"
  aria-label="Connections"
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) onclose();
  }}
>
  <div class="manager-heading">
    <h2>Connections</h2>
    <button class="secondary-button" onclick={onadd} disabled={busy}><Plus size={14} />Add</button
    ><button class="icon-button" aria-label="Close connections" onclick={onclose} disabled={busy}
      ><X size={18} /></button
    >
  </div>
  <div class="manager-search">
    <Search size={15} /><input
      aria-label="Search connections"
      placeholder="Name, host, database or group…"
      bind:value={search}
      oninput={() => (page = 0)}
    />
  </div>
  <div class="manager-filters">
    <select aria-label="Filter connection engine" bind:value={engine} onchange={() => (page = 0)}
      ><option value="">All engines</option><option value="postgres">PostgreSQL</option><option
        value="mysql">MySQL</option
      ><option value="redis">Redis</option></select
    ><select aria-label="Filter connection group" bind:value={group} onchange={() => (page = 0)}
      ><option value="">All groups</option>{#each groups as name}<option>{name}</option
        >{/each}</select
    >
  </div>
  <div class="manager-list">
    {#each visible.slice(page * 12, page * 12 + 12) as entry (entry.key)}
      <div class="manager-item" class:current={entry.key === activeKey}>
        <button class="manager-select" onclick={() => onselect(entry)} disabled={busy}>
          <span class="engine-code"
            >{entry.info.engine === 'postgres'
              ? 'PG'
              : entry.info.engine === 'mysql'
                ? 'MY'
                : 'RD'}</span
          >
          <span
            ><strong>{entry.info.name}{hasEdits(entry) ? ' •' : ''}</strong><small
              >{entry.saved?.group ? `${entry.saved.group} · ` : ''}{entry.info.host}:{entry.info
                .port} / {entry.info.database} · {entry.info.username || 'default'}</small
            ></span
          >
          <small>{entry.online ? 'Ready' : 'Disconnected'}</small>
        </button>
        {#if !entry.online}<button
            class="icon-button"
            aria-label={`Reconnect ${entry.info.name}`}
            onclick={() => onreconnect(entry)}
            disabled={busy}><PlugZap size={16} /></button
          >{/if}
        {#if entry.saved}<button
            class="icon-button"
            aria-label={`${entry.saved.pinned ? 'Unpin' : 'Pin'} ${entry.info.name}`}
            onclick={() => onpin(entry)}
            disabled={busy}
            ><Star size={15} fill={entry.saved.pinned ? 'currentColor' : 'none'} /></button
          >
          <button
            class="icon-button"
            aria-label={`Edit ${entry.info.name}`}
            onclick={() => onedit(entry)}
            disabled={busy || entry.online}><Settings2 size={15} /></button
          >
          <button
            class="icon-button"
            aria-label={`Remove ${entry.info.name}`}
            onclick={() => (removeKey = entry.key)}
            disabled={busy || entry.online || hasEdits(entry)}><Trash2 size={15} /></button
          >{/if}
      </div>
      {#if removeKey === entry.key}<div class="manager-confirm">
          <span
            >Remove saved settings for {entry.info.name}? This does not change your database.</span
          ><button
            class="secondary-button"
            onclick={() => {
              onremove(entry);
              removeKey = '';
            }}
            disabled={busy}>Remove bookmark</button
          ><button class="secondary-button" onclick={() => (removeKey = '')}>Keep</button>
        </div>{/if}
    {/each}
    {#if !visible.length}<p class="muted">No matching connections.</p>{/if}
  </div>
  <div class="manager-footer">
    <span
      >{visible.length} connections · {entries.filter((e) => e.online && e.key !== 'sample')
        .length}/8 ready</span
    ><button class="secondary-button" disabled={page === 0} onclick={() => page--}>Previous</button
    ><button
      class="secondary-button"
      disabled={(page + 1) * 12 >= visible.length}
      onclick={() => page++}>Next</button
    >
  </div>
  <p class="muted small">
    Disconnect before editing or removing saved settings. Passwords never appear in this list.
  </p>
</dialog>
