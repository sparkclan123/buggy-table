<script lang="ts">
  import { Database, Layers, Table2, Plus, Minus, RefreshCw } from '@lucide/svelte';
  import type { TableInfo } from '../lib/types';
  import { schemaKey, tableKey } from '../lib/workspace';

  let {
    names,
    schemas,
    tables,
    expanded,
    expandedSchemas,
    selectedDatabase,
    selectedSchema,
    selectedTable,
    search,
    disabled,
    ontoggle,
    onschema,
    onrefresh,
    onselect,
    onreload,
  }: {
    names: string[];
    schemas: Record<string, string[]>;
    tables: TableInfo[];
    expanded: string[];
    expandedSchemas: string[];
    selectedDatabase: string;
    selectedSchema: string;
    selectedTable: string;
    search: string;
    disabled: boolean;
    ontoggle: (database: string) => void;
    onschema: (database: string, schema: string, refresh?: boolean) => void;
    onrefresh: (database: string) => void;
    onselect: (table: TableInfo) => void;
    onreload: () => void;
  } = $props();
  const needle = $derived(search.toLowerCase());
  function matches(value: string) {
    return value.toLowerCase().includes(needle);
  }
  function children(database: string, schema: string) {
    return tables.filter(
      (t) =>
        t.database === database &&
        t.schema === schema &&
        (!needle || matches(database) || matches(schema) || matches(t.name)),
    );
  }
  function visibleSchemas(database: string) {
    return (schemas[database] ?? []).filter(
      (schema) =>
        !needle ||
        matches(database) ||
        matches(schema) ||
        !expandedSchemas.includes(schemaKey(schema, database)) ||
        children(database, schema).length,
    );
  }
  const visible = $derived(
    names.filter(
      (database) =>
        !needle || matches(database) || !schemas[database] || visibleSchemas(database).length,
    ),
  );
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
<div class="table-list database-tree" aria-label="PostgreSQL databases">
  {#each visible as database (database)}
    <div class="database-node">
      <div class="database-node-heading">
        <button
          class="database-toggle"
          class:current={selectedDatabase === database}
          aria-label={`${expanded.includes(database) ? 'Collapse' : 'Expand'} database ${database}`}
          aria-expanded={expanded.includes(database)}
          title={database}
          onclick={() => ontoggle(database)}
          {disabled}
        >
          {#if expanded.includes(database)}<Minus size={12} />{:else}<Plus size={12} />{/if}
          <Database size={13} /><span>{database}</span>
        </button>
        <button
          class="database-refresh"
          aria-label={`Refresh database ${database}`}
          title="Refresh schemas and expanded tables"
          onclick={() => onrefresh(database)}
          {disabled}><RefreshCw size={12} /></button
        >
      </div>
      {#if expanded.includes(database)}
        <div class="pg-schemas" aria-label={`Schemas in ${database}`}>
          {#each visibleSchemas(database) as schema (schema)}
            {@const key = schemaKey(schema, database)}
            <div class="database-node-heading">
              <button
                class="database-toggle"
                class:current={selectedDatabase === database && selectedSchema === schema}
                aria-label={`${expandedSchemas.includes(key) ? 'Collapse' : 'Expand'} schema ${database}.${schema}`}
                aria-expanded={expandedSchemas.includes(key)}
                title={schema}
                onclick={() => onschema(database, schema)}
                {disabled}
              >
                {#if expandedSchemas.includes(key)}<Minus size={11} />{:else}<Plus size={11} />{/if}
                <Layers size={12} /><span>{schema}</span>
              </button>
              <button
                class="database-refresh"
                aria-label={`Refresh schema ${database}.${schema}`}
                title="Refresh tables"
                onclick={() => onschema(database, schema, true)}
                {disabled}><RefreshCw size={11} /></button
              >
            </div>
            {#if expandedSchemas.includes(key)}
              <div class="database-tables" aria-label={`Tables in ${database}.${schema}`}>
                {#each children(database, schema) as table (tableKey(schema, table.name, database))}
                  <button
                    class:selected={selectedDatabase === database &&
                      selectedSchema === schema &&
                      selectedTable === table.name}
                    title={table.readBlockedReason ?? table.relationKind ?? 'table'}
                    onclick={() => onselect(table)}
                    {disabled}><Table2 size={13} /><span>{table.name}</span></button
                  >
                {/each}
                {#if !children(database, schema).length}<p class="sidebar-empty">
                    {needle ? 'No matching tables' : 'No accessible tables'}
                  </p>{/if}
              </div>
            {/if}
          {/each}
          {#if !visibleSchemas(database).length}<p class="sidebar-empty">
              {needle ? 'No matches' : 'No accessible schemas'}
            </p>{/if}
        </div>
      {/if}
    </div>
  {/each}
  {#if !visible.length}<p class="sidebar-empty">
      {needle ? 'No matches' : 'No accessible databases'}
    </p>{/if}
</div>

<style>
  .pg-schemas {
    margin-left: 12px;
    border-left: 1px solid var(--border);
    padding-left: 3px;
  }
</style>
