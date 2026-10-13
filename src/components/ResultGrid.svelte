<script lang="ts">
  import { tick } from 'svelte';
  import { KeyRound, ArrowUp, ArrowDown } from '@lucide/svelte';
  import type { CellEdits } from '../lib/workspace';
  import type { QueryResult, TableInfo } from '../lib/types';

  let {
    result,
    table,
    busy = false,
    editable = false,
    edits = {},
    top = $bindable(0),
    widths = $bindable({}),
    sortColumn = null,
    descending = false,
    onsort,
    onchange = () => {},
  }: {
    result: QueryResult | null;
    table: TableInfo | undefined;
    busy?: boolean;
    editable?: boolean;
    edits?: CellEdits;
    top?: number;
    widths?: Record<string, number>;
    sortColumn?: string | null;
    descending?: boolean;
    onsort?: (column: string) => void;
    onchange?: (row: number, column: string, value: string | null) => void;
  } = $props();

  type Editing = {
    row: number;
    column: string;
    original: string | null;
    draft: string;
    cell: HTMLTableCellElement;
  };
  let editing = $state<Editing | null>(null);
  let resizing: { column: string; start: number; width: number } | null = null;
  const rowHeight = 28;
  let viewportRows = $state(32);
  let scroller: HTMLDivElement;
  const start = $derived(
    Math.max(
      0,
      Math.min(
        Math.floor(top / rowHeight) - 4,
        Math.max(0, (result?.rows.length ?? 0) - viewportRows),
      ),
    ),
  );
  const end = $derived(Math.min(result?.rows.length ?? 0, start + viewportRows));
  const totalWidth = $derived(
    36 + (result?.columns.reduce((sum, column) => sum + width(column), 0) ?? 0),
  );
  const metadata = $derived(Object.fromEntries((table?.columns ?? []).map((c) => [c.name, c])));

  let loadedResult: QueryResult | null = null;
  $effect(() => {
    // New results invalidate an open editor, including synchronous sample saves.
    if (busy || result !== loadedResult) {
      editing = null;
      loadedResult = result;
    }
  });
  $effect(() => {
    if (scroller) scroller.scrollTop = top;
  });
  function observeViewport(node: HTMLDivElement) {
    const observer = new ResizeObserver(() => {
      viewportRows = Math.ceil(node.clientHeight / rowHeight) + 8;
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }
  function focusEditor(node: HTMLTextAreaElement) {
    node.focus();
    node.select();
  }
  function width(column: string) {
    return widths[column] ?? (column.endsWith('_at') ? 230 : column === 'id' ? 90 : 170);
  }
  function setWidth(column: string, value: number) {
    widths[column] = Math.max(72, Math.min(800, Math.round(value)));
  }
  function display(row: number, column: string, original: string | null) {
    return edits[row] && column in edits[row] ? edits[row][column] : original;
  }
  async function finish(focusCell = false, revert = false) {
    const current = editing;
    if (!current) return;
    if (revert) onchange(current.row, current.column, current.original);
    editing = null;
    if (focusCell) {
      await tick();
      current.cell.querySelector<HTMLButtonElement>('.editable-cell')?.focus();
    }
  }
  function startResize(event: PointerEvent, column: string) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    resizing = { column, start: event.clientX, width: width(column) };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }
</script>

<div
  class="grid-scroll"
  bind:this={scroller}
  use:observeViewport
  onscroll={() => (top = scroller.scrollTop)}
  aria-busy={busy}
>
  {#if result && result.columns.length}
    <table
      class="data-table"
      aria-label="Query results"
      aria-rowcount={result.rows.length + 1}
      style:width={`${totalWidth}px`}
    >
      <colgroup
        ><col style:width="36px" />{#each result.columns as column}<col
            style:width={`${width(column)}px`}
          />{/each}</colgroup
      >
      <thead
        ><tr
          ><th class="row-number" scope="col">#</th>
          {#each result.columns as column}
            <th
              scope="col"
              aria-sort={sortColumn === column ? (descending ? 'descending' : 'ascending') : 'none'}
            >
              <button
                class="column-heading"
                aria-label={`Sort by ${column}`}
                disabled={!onsort || busy}
                onclick={() => onsort?.(column)}
                title={`${column} · ${metadata[column]?.dataType ?? 'text'}`}
              >
                {#if metadata[column]?.primaryKey}<KeyRound size={11} />{/if}
                <span>{column}</span><small>{metadata[column]?.dataType ?? 'text'}</small>
                {#if sortColumn === column}{#if descending}<ArrowDown size={12} />{:else}<ArrowUp
                      size={12}
                    />{/if}{/if}
              </button>
              <div
                class="column-resizer"
                role="slider"
                tabindex="0"
                aria-label={`Resize ${column} column`}
                aria-orientation="horizontal"
                aria-valuemin="72"
                aria-valuemax="800"
                aria-valuenow={width(column)}
                onpointerdown={(event) => startResize(event, column)}
                onpointermove={(event) => {
                  if (resizing?.column === column)
                    setWidth(column, resizing.width + event.clientX - resizing.start);
                }}
                onpointerup={() => (resizing = null)}
                onpointercancel={() => (resizing = null)}
                onlostpointercapture={() => (resizing = null)}
                ondblclick={() => {
                  delete widths[column];
                }}
                onkeydown={(event) => {
                  if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
                    event.preventDefault();
                    setWidth(
                      column,
                      width(column) +
                        (event.key === 'ArrowRight' ? 1 : -1) * (event.shiftKey ? 50 : 10),
                    );
                  }
                }}
              ></div>
            </th>
          {/each}
        </tr></thead
      >
      <tbody>
        {#if start > 0}<tr aria-hidden="true" class="spacer"
            ><td colspan={result.columns.length + 1} style:height={`${start * rowHeight}px`}
            ></td></tr
          >{/if}
        {#each result.rows.slice(start, end) as row, i (start + i)}
          <tr aria-rowindex={start + i + 2}
            ><td class="row-number">{start + i + 1}</td>
            {#each row as original, j}
              {@const column = result.columns[j]}
              {@const cell = display(start + i, column, original)}
              <td
                title={cell ?? 'NULL'}
                class:edited={!!edits[start + i] && column in edits[start + i]}
              >
                {#if editing?.row === start + i && editing.column === column}
                  <div class="cell-input-wrap">
                    <textarea
                      class="cell-input"
                      aria-label={`Cell value row ${start + i + 1} ${column}`}
                      rows="1"
                      maxlength="1048576"
                      spellcheck="false"
                      use:focusEditor
                      value={editing.draft}
                      oninput={(event) => {
                        if (editing) {
                          editing.draft = event.currentTarget.value;
                          onchange(editing.row, editing.column, editing.draft);
                        }
                      }}
                      onblur={(event) => {
                        if (!editing?.cell.contains(event.relatedTarget as Node | null))
                          void finish();
                      }}
                      onkeydown={(event) => {
                        if (event.isComposing) return;
                        if (event.key === 'Escape') {
                          event.preventDefault();
                          event.stopPropagation();
                          void finish(true, true);
                        } else if (event.key === 'Enter' && !event.shiftKey) {
                          event.preventDefault();
                          void finish(true);
                        }
                      }}></textarea>
                    {#if metadata[column]?.nullable}<button
                        class="set-null"
                        aria-label={`Set row ${start + i + 1} ${column} to NULL`}
                        title="Set to NULL"
                        onpointerdown={(event) => event.preventDefault()}
                        onclick={() => {
                          onchange(start + i, column, null);
                          void finish(true);
                        }}>NULL</button
                      >{/if}
                  </div>
                {:else if editable && metadata[column]?.editable}
                  <button
                    class="editable-cell"
                    aria-label={`Edit row ${start + i + 1} ${column}`}
                    disabled={busy}
                    onclick={(event) =>
                      (editing = {
                        row: start + i,
                        column,
                        original: cell,
                        draft: cell ?? '',
                        cell: event.currentTarget.closest('td')!,
                      })}
                  >
                    {#if cell === null}<span class="null-value">NULL</span>{:else}{cell === ''
                        ? '""'
                        : cell}{/if}
                  </button>
                {:else}<span class="cell-text"
                    >{#if cell === null}<span class="null-value">NULL</span>{:else}{cell}{/if}</span
                  >{/if}
              </td>
            {/each}
          </tr>
        {/each}
        {#if end < result.rows.length}<tr aria-hidden="true" class="spacer"
            ><td
              colspan={result.columns.length + 1}
              style:height={`${(result.rows.length - end) * rowHeight}px`}
            ></td></tr
          >{/if}
      </tbody>
    </table>
    {#if !result.rows.length}<div class="empty-result">
        <h3>No matching rows</h3>
        <p>Adjust your filters or query.</p>
      </div>{/if}
  {:else}<div class="empty-result">
      <h3>{busy ? 'Reading your database…' : 'Select a table or run a query'}</h3>
    </div>{/if}
</div>
