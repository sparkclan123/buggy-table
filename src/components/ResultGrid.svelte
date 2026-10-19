<script lang="ts">
  import { tick } from 'svelte';
  import { KeyRound, ArrowUp, ArrowDown } from '@lucide/svelte';
  import DateTimeInput from './DateTimeInput.svelte';
  import RowInspector from './RowInspector.svelte';
  import { temporalKind } from '../lib/temporal';
  import { readColumnOrder, saveColumnOrder } from '../lib/column-layout';
  import type { CellEdits } from '../lib/workspace';
  import type { QueryResult, TableInfo } from '../lib/types';

  let {
    result,
    table,
    busy = false,
    editable = false,
    edits = {},
    deletedRows = [],
    selectedRows = $bindable([]),
    fullValues = false,
    inspectorEnabled = false,
    inspectorWidth = 10,
    oninspectorresize = () => {},
    layoutKey = '',
    rowOffset = 0,
    top = $bindable(0),
    widths = $bindable({}),
    sortColumn = null,
    descending = false,
    onsort,
    onchange = () => {},
    ondelete = () => {},
    onlayoutwarning = () => {},
  }: {
    result: QueryResult | null;
    table: TableInfo | undefined;
    busy?: boolean;
    editable?: boolean;
    edits?: CellEdits;
    deletedRows?: number[];
    selectedRows?: number[];
    fullValues?: boolean;
    inspectorEnabled?: boolean;
    inspectorWidth?: number;
    oninspectorresize?: (percent: number) => void;
    layoutKey?: string;
    rowOffset?: number;
    top?: number;
    widths?: Record<string, number>;
    sortColumn?: string | null;
    descending?: boolean;
    onsort?: (column: string) => void;
    onchange?: (row: number, column: string, value: string | null) => void;
    ondelete?: (rows: number[]) => void;
    onlayoutwarning?: () => void;
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
  let order = $state<string[]>([]);
  let dragging = $state<string | null>(null);
  let draggedAt = 0;
  let selectionAnchor = 0;
  let inspectedRow = $state<number | null>(null);
  let inspectorDismissed = $state(false);
  let heights = $state<Record<number, number>>({});
  let viewportHeight = $state(700);
  let scroller: HTMLDivElement;
  let loadedResult: QueryResult | null = null;
  let loadedLayout = '';
  const metadata = $derived(Object.fromEntries((table?.columns ?? []).map((c) => [c.name, c])));
  const indices = $derived(Object.fromEntries((result?.columns ?? []).map((name, i) => [name, i])));
  const columns = $derived([
    ...new Set([...order.filter((c) => c in indices), ...(result?.columns ?? [])]),
  ]);
  const totalWidth = $derived(42 + columns.reduce((sum, column) => sum + width(column), 0));
  const activeRow = $derived(
    inspectedRow !== null && selectedRows.includes(inspectedRow)
      ? inspectedRow
      : selectedRows.at(-1),
  );
  const inspectorRow = $derived(
    inspectorEnabled && !inspectorDismissed && activeRow !== undefined
      ? result?.rows[activeRow]
      : undefined,
  );
  const inspectorFields = $derived(
    inspectorRow && activeRow !== undefined
      ? columns.map((name) => ({
          name,
          column: metadata[name],
          value: display(activeRow, name, inspectorRow[indices[name]]),
          edited: !!edits[activeRow] && name in edits[activeRow],
        }))
      : [],
  );
  const offsets = $derived.by(() => {
    const values = [0];
    for (let i = 0; i < (result?.rows.length ?? 0); i++)
      values.push(values[i] + (fullValues ? (heights[i] ?? 56) : 28));
    return values;
  });
  function rowAt(position: number) {
    let lo = 0,
      hi = Math.max(0, offsets.length - 1);
    while (lo < hi) {
      const mid = Math.ceil((lo + hi) / 2);
      if (offsets[mid] <= position) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  }
  const start = $derived(Math.max(0, rowAt(top) - 4));
  const end = $derived(Math.min(result?.rows.length ?? 0, rowAt(top + viewportHeight) + 5));
  $effect(() => {
    if (busy || result !== loadedResult) editing = null;
    if (result !== loadedResult) {
      if (loadedResult) selectedRows = [];
      inspectedRow = null;
      inspectorDismissed = false;
      loadedResult = result;
    }
  });
  $effect(() => {
    if (inspectorEnabled) inspectorDismissed = false;
  });
  $effect(() => {
    // Re-measure wrapped rows when results, wrapping or column widths change.
    void result;
    void fullValues;
    JSON.stringify(widths);
    heights = {};
  });
  $effect(() => {
    if (layoutKey !== loadedLayout) {
      loadedLayout = layoutKey;
      order = layoutKey ? readColumnOrder(layoutKey) : [];
    }
  });
  $effect(() => {
    if (scroller && scroller.scrollTop !== top) scroller.scrollTop = top;
  });
  function observeViewport(node: HTMLDivElement) {
    const observer = new ResizeObserver(() => {
      viewportHeight = node.clientHeight;
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }
  function measureRow(node: HTMLTableRowElement, row: number) {
    const observer = new ResizeObserver(() => {
      const height = Math.max(28, Math.ceil(node.getBoundingClientRect().height));
      if (fullValues && heights[row] !== height) heights[row] = height;
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
  function input(value: string) {
    if (editing) {
      editing.draft = value;
      onchange(editing.row, editing.column, value);
    }
  }
  function blur(event: FocusEvent) {
    if (!editing?.cell.contains(event.relatedTarget as Node | null)) void finish();
  }
  function startResize(event: PointerEvent, column: string) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    resizing = { column, start: event.clientX, width: width(column) };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }
  function moveColumn(from: string, to: string) {
    if (busy || from === to) return;
    const next = [...columns];
    const before = next.indexOf(from),
      after = next.indexOf(to);
    if (before < 0 || after < 0) return;
    next.splice(before, 1);
    next.splice(after, 0, from);
    order = next;
    if (layoutKey && !saveColumnOrder(layoutKey, next)) onlayoutwarning();
  }
  function selectRow(event: MouseEvent, row: number) {
    if (busy) return;
    // Safari does not focus buttons on click; keep Delete/Backspace on this row.
    (event.currentTarget as HTMLButtonElement).focus();
    if (event.shiftKey)
      selectedRows = Array.from(
        { length: Math.abs(row - selectionAnchor) + 1 },
        (_, i) => Math.min(row, selectionAnchor) + i,
      );
    else if (event.metaKey || event.ctrlKey)
      selectedRows = selectedRows.includes(row)
        ? selectedRows.filter((v) => v !== row)
        : [...selectedRows, row];
    else selectedRows = [row];
    if (!event.shiftKey) selectionAnchor = row;
    inspectedRow = row;
    inspectorDismissed = false;
  }
  function canEdit(row: number, column: string) {
    return editable && metadata[column]?.editable && !deletedRows.includes(row);
  }
  function activateCell(event: MouseEvent, row: number, column: string, value: string | null) {
    if (busy) return;
    const wasSelected = selectedRows.includes(row);
    selectRow(event, row);
    if (wasSelected && !event.shiftKey && !event.metaKey && !event.ctrlKey && canEdit(row, column))
      editing = {
        row,
        column,
        original: value,
        draft: value ?? '',
        cell: (event.currentTarget as HTMLElement).closest('td')!,
      };
  }
  function deleteKey(event: KeyboardEvent, row: number) {
    if (!busy && selectedRows.includes(row) && ['Backspace', 'Delete'].includes(event.key)) {
      event.preventDefault();
      event.stopPropagation();
      ondelete(selectedRows);
    }
  }
  function cellKey(event: KeyboardEvent, row: number) {
    // A space in accidental typing must not activate the focused cell button.
    // Enter remains an explicit, keyboard-accessible way to select/edit a cell.
    if (event.key === ' ') event.preventDefault();
    deleteKey(event, row);
  }
  async function closeInspector() {
    inspectorDismissed = true;
    await tick();
    scroller
      .querySelector<HTMLButtonElement>('.row-select[aria-pressed="true"]')
      ?.focus({ preventScroll: true });
  }
</script>

<div class="result-area">
  <div
    class="grid-scroll"
    class:full-values={fullValues}
    bind:this={scroller}
    use:observeViewport
    onscroll={() => (top = scroller.scrollTop)}
    aria-busy={busy}
  >
    {#if result && columns.length}
      <table
        class="data-table"
        aria-label="Query results"
        aria-rowcount={result.rows.length + 1}
        style:width={`${totalWidth}px`}
      >
        <colgroup
          ><col style:width="42px" />{#each columns as column}<col
              style:width={`${width(column)}px`}
            />{/each}</colgroup
        >
        <thead
          ><tr
            ><th class="row-number" scope="col">#</th>
            {#each columns as column (column)}
              <th
                scope="col"
                class:dragging={dragging === column}
                aria-sort={sortColumn === column
                  ? descending
                    ? 'descending'
                    : 'ascending'
                  : 'none'}
              >
                <button
                  class="column-heading"
                  aria-label={`Sort by ${column}`}
                  disabled={busy}
                  draggable={!!layoutKey && !busy}
                  ondragstart={(event) => {
                    dragging = column;
                    event.dataTransfer?.setData('text/plain', column);
                    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
                  }}
                  ondragover={(event) => {
                    if (dragging) {
                      event.preventDefault();
                      if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
                    }
                  }}
                  ondrop={(event) => {
                    event.preventDefault();
                    if (dragging) moveColumn(dragging, column);
                    dragging = null;
                    draggedAt = Date.now();
                  }}
                  ondragend={() => {
                    dragging = null;
                    draggedAt = Date.now();
                  }}
                  onkeydown={(event) => {
                    if (
                      layoutKey &&
                      event.altKey &&
                      ['ArrowLeft', 'ArrowRight'].includes(event.key)
                    ) {
                      event.preventDefault();
                      const target =
                        columns[columns.indexOf(column) + (event.key === 'ArrowLeft' ? -1 : 1)];
                      if (target) moveColumn(column, target);
                    }
                  }}
                  onclick={() => {
                    if (Date.now() - draggedAt > 200) onsort?.(column);
                  }}
                  title={`${column} · ${metadata[column]?.dataType ?? 'text'}${layoutKey ? ' · Drag or Alt+←/→ to reorder on this device' : ''}`}
                >
                  {#if metadata[column]?.primaryKey}<KeyRound size={11} />{/if}<span>{column}</span
                  ><small>{metadata[column]?.dataType ?? 'text'}</small>
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
              ><td colspan={columns.length + 1} style:height={`${offsets[start]}px`}></td></tr
            >{/if}
          {#each result.rows.slice(start, end) as row, i (start + i)}
            {@const index = start + i}
            <tr
              aria-rowindex={index + 2}
              aria-selected={selectedRows.includes(index)}
              class:row-selected={selectedRows.includes(index)}
              class:striped-row={(index + rowOffset) % 2 === 1}
              class:pending-delete={deletedRows.includes(index)}
              use:measureRow={index}
            >
              <td class="row-number"
                ><button
                  class="row-select"
                  aria-label={`Select row ${index + 1}`}
                  aria-pressed={selectedRows.includes(index)}
                  disabled={busy}
                  title={deletedRows.includes(index)
                    ? 'Pending deletion · save or discard'
                    : 'Select row · Delete/Backspace stages deletion'}
                  onclick={(event) => selectRow(event, index)}
                  onkeydown={(event) => deleteKey(event, index)}
                >
                  {rowOffset + index + 1}
                </button></td
              >
              {#each columns as column (column)}
                {@const original = row[indices[column]]}
                {@const cell = display(index, column, original)}
                <td
                  title={cell ?? 'NULL'}
                  class:edited={!!edits[index] && column in edits[index]}
                  class:editing={editing?.row === index && editing.column === column}
                >
                  {#if editing?.row === index && editing.column === column}
                    <div class="cell-input-wrap">
                      {#if temporalKind(metadata[column]?.dataType)}
                        <DateTimeInput
                          value={editing.draft}
                          type={metadata[column].dataType}
                          label={`Cell value row ${index + 1} ${column}`}
                          cell
                          autofocus
                          maxlength={1048576}
                          onchange={input}
                          onblur={blur}
                          oncommit={() => void finish(true)}
                          onrevert={() => void finish(true, true)}
                        />
                      {:else}
                        <textarea
                          class="cell-input"
                          aria-label={`Cell value row ${index + 1} ${column}`}
                          rows="1"
                          maxlength="1048576"
                          spellcheck="false"
                          use:focusEditor
                          value={editing.draft}
                          oninput={(event) => input(event.currentTarget.value)}
                          onblur={blur}
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
                      {/if}
                      {#if metadata[column]?.nullable}<button
                          class="set-null"
                          aria-label={`Set row ${index + 1} ${column} to NULL`}
                          title="Set to NULL"
                          onpointerdown={(event) => event.preventDefault()}
                          onclick={() => {
                            onchange(index, column, null);
                            void finish(true);
                          }}>NULL</button
                        >{/if}
                    </div>
                  {:else}
                    <button
                      class={canEdit(index, column) ? 'editable-cell' : 'cell-text'}
                      aria-label={`${canEdit(index, column) ? 'Edit' : 'Select'} row ${index + 1} ${column}`}
                      title={canEdit(index, column)
                        ? 'Select row; activate again to edit'
                        : 'Select row to view details'}
                      disabled={busy}
                      onclick={(event) => activateCell(event, index, column, cell)}
                      onkeydown={(event) => cellKey(event, index)}
                    >
                      {#if cell === null}<span class="null-value">NULL</span>{:else}{cell === ''
                          ? '""'
                          : cell}{/if}
                    </button>
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
          {#if end < result.rows.length}<tr aria-hidden="true" class="spacer"
              ><td
                colspan={columns.length + 1}
                style:height={`${offsets[offsets.length - 1] - offsets[end]}px`}
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
  {#if inspectorRow && activeRow !== undefined}
    <RowInspector
      fields={inspectorFields}
      rowNumber={rowOffset + activeRow + 1}
      deleted={deletedRows.includes(activeRow)}
      widthPercent={inspectorWidth}
      onresize={oninspectorresize}
      onclose={closeInspector}
    />
  {/if}
</div>
