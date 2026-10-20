<script lang="ts">
  import { Database, Table2, Plus, Minus, RefreshCw } from '@lucide/svelte';
  import type { TableInfo } from '../lib/types';
  let {
    names,
    tables,
    expanded,
    loaded,
    selectedSchema,
    selectedTable,
    search,
    disabled,
    ontoggle,
    onrefresh,
    onselect,
    onreload,
  }: {
    names: string[];
    tables: TableInfo[];
    expanded: string[];
    loaded: string[];
    selectedSchema: string;
    selectedTable: string;
    search: string;
    disabled: boolean;
    ontoggle: (name: string) => void;
    onrefresh: (name: string) => void;
    onselect: (table: TableInfo) => void;
    onreload: () => void;
  } = $props();
  const needle = $derived(search.toLowerCase());
  const visible = $derived(
    names.filter(
      (name) =>
        !needle ||
        name.toLowerCase().includes(needle) ||
        !loaded.includes(name) ||
        tables.some((t) => t.schema === name && t.name.toLowerCase().includes(needle)),
    ),
  );
  function children(name: string) {
    return tables.filter(
      (t) =>
        t.schema === name &&
        (!needle || name.toLowerCase().includes(needle) || t.name.toLowerCase().includes(needle)),
    );
  }
</script>

<div class="database-tree-heading">
  <span>Databases <small>{names.length}</small></span>
  <button
    class="icon-button"
    aria-label="Refresh databases"
    title="Refresh databases"
    onclick={onreload}
    {disabled}><RefreshCw size={13} /></button
  >
</div>
<div class="table-list database-tree" aria-label="MySQL databases">
  {#each visible as name (name)}
    <div class="database-node">
      <div class="database-node-heading">
        <button
          class="database-toggle"
          class:current={selectedSchema === name}
          aria-label={`${expanded.includes(name) ? 'Collapse' : 'Expand'} database ${name}`}
          aria-expanded={expanded.includes(name)}
          title={name}
          onclick={() => ontoggle(name)}
          {disabled}
        >
          {#if expanded.includes(name)}<Minus size={12} />{:else}<Plus size={12} />{/if}
          <Database size={13} /><span>{name}</span>
        </button>
        <button
          class="database-refresh"
          aria-label={`Refresh database ${name}`}
          title="Refresh tables"
          onclick={() => onrefresh(name)}
          {disabled}><RefreshCw size={12} /></button
        >
      </div>
      {#if expanded.includes(name)}
        <div class="database-tables" aria-label={`Tables in ${name}`}>
          {#each children(name) as table (table.name)}
            <button
              class:selected={selectedSchema === name && selectedTable === table.name}
              title={table.readBlockedReason ?? table.relationKind ?? 'table'}
              onclick={() => onselect(table)}
              {disabled}><Table2 size={13} /><span>{table.name}</span></button
            >
          {/each}
          {#if !children(name).length}
            <p class="sidebar-empty">{needle ? 'No matching tables' : 'No accessible tables'}</p>
          {/if}
        </div>
      {/if}
    </div>
  {/each}
  {#if !visible.length}<p class="sidebar-empty">
      {needle ? 'No matches' : 'No accessible databases'}
    </p>{/if}
</div>
