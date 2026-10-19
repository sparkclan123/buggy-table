<script lang="ts">
  import DateTimeInput from './DateTimeInput.svelte';
  import { Plus, X } from '@lucide/svelte';
  import type { Filter, FilterMatch, TableInfo } from '../lib/types';
  let {
    table,
    filters = $bindable(),
    match = $bindable(),
    busy,
    onapply,
    onclear,
  }: {
    table: TableInfo;
    filters: Filter[];
    match: FilterMatch;
    busy: boolean;
    onapply: (event: SubmitEvent) => void;
    onclear: () => void;
  } = $props();
  const operators = [
    ['equals', '=', 'Equals'],
    ['notEquals', '≠', 'Not equal'],
    ['contains', '∋', 'Contains'],
    ['greaterThan', '>', 'Greater than'],
    ['lessThan', '<', 'Less than'],
    ['isNull', '∅', 'Is NULL'],
    ['isNotNull', '≠ ∅', 'Is not NULL'],
  ];
</script>

<form class="filter-builder" onsubmit={onapply}>
  <div class="filter-actions">
    <select aria-label="Match filters" bind:value={match}
      ><option value="all">Match all (AND)</option><option value="any">Match any (OR)</option
      ></select
    >
    <button
      type="button"
      class="secondary-button"
      disabled={busy || filters.length >= 20}
      onclick={() =>
        filters.push({ column: table.columns[0]?.name ?? '', operator: 'equals', value: '' })}
      ><Plus size={12} />Add filter</button
    >
    <span class="muted small">{filters.length}/20</span><span class="tab-spacer"></span>
    <button type="button" class="secondary-button" onclick={onclear} disabled={busy}
      >Clear filters</button
    >
    <button class="primary-button" disabled={busy}>Apply filters</button>
  </div>
  <div class="filter-rows">
    {#each filters as filter, index}
      <div class="filter-row">
        <span class="muted filter-number">{index + 1}</span>
        <select aria-label={`Filter ${index + 1} column`} bind:value={filter.column}>
          {#each table.columns as column}<option value={column.name}>{column.name}</option>{/each}
        </select>
        <select
          aria-label={`Filter ${index + 1} operator`}
          title={operators.find(([value]) => value === filter.operator)?.[2]}
          bind:value={filter.operator}
        >
          {#each operators as [value, symbol, description]}<option
              {value}
              title={description}
              aria-label={description}>{symbol}</option
            >{/each}
        </select>
        <DateTimeInput
          label={`Filter ${index + 1} value`}
          value={filter.value}
          type={filter.operator === 'contains'
            ? ''
            : (table.columns.find((c) => c.name === filter.column)?.dataType ?? '')}
          onchange={(value) => (filter.value = value)}
          disabled={busy || filter.operator === 'isNull' || filter.operator === 'isNotNull'}
        />
        <button
          class="icon-button"
          type="button"
          aria-label={`Remove filter ${index + 1}`}
          onclick={() => filters.splice(index, 1)}
          disabled={busy}><X size={13} /></button
        >
      </div>
    {/each}
  </div>
</form>
