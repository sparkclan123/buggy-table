import type { Filter, FilterMatch, IndexInfo, QueryResult, RowChange, TableInfo } from './types';
import { quoteIdentifier } from './api';

export const tableKey = (schema: string, table: string) => JSON.stringify([schema, table]);

export type TableView = 'data' | 'structure' | 'indexes' | 'sql';
export type CellEdits = Record<number, Record<string, string | null>>;
export type DraftRow = Record<string, string | null>;
export type CellValue = string | null | undefined;
type DraftAction =
  | { kind: 'cell'; row: number; column: string; before: CellValue; after: CellValue }
  | { kind: 'delete'; before: number[]; after: number[] }
  | { kind: 'insert'; rows: DraftRow[] };
export function createWorkspace(table?: TableInfo, engine = 'postgres') {
  return {
    id: table ? tableKey(table.schema, table.name) : 'empty',
    schema: table?.schema ?? '',
    selectedName: table?.name ?? '',
    tab: (table?.readBlockedReason ? 'structure' : table ? 'data' : 'sql') as TableView,
    showFilter: false,
    filters: [{ column: table?.columns[0]?.name ?? '', operator: 'equals', value: '' }] as Filter[],
    filterMatch: 'all' as FilterMatch,
    appliedFilters: [] as Filter[],
    appliedFilterMatch: 'all' as FilterMatch,
    sortColumn: null as string | null,
    descending: false,
    columnWidths: {} as Record<string, number>,
    pageNumber: 0,
    pageSize: 100,
    fullValues: false,
    collapsed: false,
    selectedRows: [] as number[],
    deletedRows: [] as number[],
    exactCount: null as string | null,
    countKey: '',
    countNotice: '',

    result: null as QueryResult | null,
    queryResult: null as QueryResult | null,
    indexes: [] as IndexInfo[],
    sql: table
      ? `SELECT *\nFROM ${quoteIdentifier(table.schema, engine)}.${quoteIdentifier(table.name, engine)}\nLIMIT 200;`
      : 'SELECT 1;',
    plan: null as string | null,
    edits: {} as CellEdits,
    insertedRows: [] as DraftRow[],
    undo: [] as DraftAction[],
    redo: [] as DraftAction[],
    pendingCell: null as { row: number; column: string; before: CellValue } | null,
    draftRevision: 0,
    scrollTop: 0,
    lastAccess: Date.now(),
    uncertain: false,
    notice: '',
  };
}
export type TableWorkspace = ReturnType<typeof createWorkspace>;

export function pendingRows(workspace: TableWorkspace) {
  return new Set([
    ...Object.keys(workspace.edits).map(Number),
    ...workspace.deletedRows,
    ...workspace.insertedRows.map((_, i) => (workspace.result?.rows.length ?? 0) + i),
  ]).size;
}

export function changedRows(
  result: QueryResult | null,
  edits: CellEdits,
  deleted: number[] = [],
  inserted: DraftRow[] = [],
): RowChange[] {
  if (!result) return [];
  return [...new Set([...Object.keys(edits).map(Number), ...deleted])]
    .filter((row) => row < result.rows.length)
    .map((row) => ({
      original: Object.fromEntries(
        result.columns.map((column, index) => [column, result.rows[row][index]]),
      ),
      changes: deleted.includes(row) ? {} : { ...edits[row] },
      ...(deleted.includes(row) ? { delete: true } : {}),
    }))
    .concat(
      inserted.flatMap((changes, i): RowChange[] =>
        deleted.includes(result.rows.length + i)
          ? []
          : [{ insert: true, original: {}, changes: { ...changes } }],
      ),
    );
}

export function changeCell(
  workspace: TableWorkspace,
  row: number,
  column: string,
  value: CellValue,
) {
  if (!workspace.result || workspace.deletedRows.includes(row)) return;
  if (row >= workspace.result.rows.length) {
    const draft = workspace.insertedRows[row - workspace.result.rows.length];
    if (!draft) return;
    if (value === undefined) delete draft[column];
    else workspace.insertedRows[row - workspace.result.rows.length] = { ...draft, [column]: value };
    workspace.notice = '';
    return;
  }
  if (value === undefined) return;
  const original = workspace.result.rows[row][workspace.result.columns.indexOf(column)];
  if (value === original) {
    delete workspace.edits[row]?.[column];
    if (workspace.edits[row] && !Object.keys(workspace.edits[row]).length)
      delete workspace.edits[row];
  } else {
    workspace.edits[row] = { ...workspace.edits[row], [column]: value };
  }
  workspace.notice = '';
}

export function cellValue(w: TableWorkspace, row: number, column: string): CellValue {
  if (!w.result) return undefined;
  if (row >= w.result.rows.length) {
    const draft = w.insertedRows[row - w.result.rows.length];
    return draft && Object.hasOwn(draft, column) ? draft[column] : undefined;
  }
  return Object.hasOwn(w.edits[row] ?? {}, column)
    ? w.edits[row][column]
    : w.result.rows[row][w.result.columns.indexOf(column)];
}
function record(w: TableWorkspace, action: DraftAction) {
  w.undo.push(action);
  w.redo = [];
  // Bound history independently of the result size; retain only changed values.
  let bytes = w.undo.reduce((n, a) => n + JSON.stringify(a).length * 2, 0);
  while (w.undo.length > 100 || bytes > 8 * 1024 * 1024)
    bytes -= JSON.stringify(w.undo.shift()!).length * 2;
}
export function beginCellEdit(w: TableWorkspace, row: number, column: string) {
  endCellEdit(w);
  w.pendingCell = { row, column, before: cellValue(w, row, column) };
}
export function endCellEdit(w: TableWorkspace) {
  const edit = w.pendingCell;
  w.pendingCell = null;
  if (!edit) return;
  const after = cellValue(w, edit.row, edit.column);
  if (edit.before !== after) record(w, { kind: 'cell', ...edit, after });
}
export function setDeletedRows(w: TableWorkspace, rows: number[]) {
  endCellEdit(w);
  if (JSON.stringify(w.deletedRows) === JSON.stringify(rows)) return;
  record(w, { kind: 'delete', before: [...w.deletedRows], after: [...rows] });
  w.deletedRows = [...rows];
}
export function appendRows(w: TableWorkspace, rows: DraftRow[]) {
  endCellEdit(w);
  record(w, { kind: 'insert', rows: rows.map((r) => ({ ...r })) });
  w.insertedRows.push(...rows.map((r) => ({ ...r })));
  const end = (w.result?.rows.length ?? 0) + w.insertedRows.length;
  w.selectedRows = rows.map((_, i) => end - rows.length + i);
}
export function undoDraft(w: TableWorkspace, redo = false) {
  if (w.uncertain) return;
  endCellEdit(w);
  const action = (redo ? w.redo : w.undo).pop();
  if (!action) return;
  if (action.kind === 'cell')
    changeCell(w, action.row, action.column, redo ? action.after : action.before);
  else if (action.kind === 'delete') w.deletedRows = [...(redo ? action.after : action.before)];
  else if (redo) w.insertedRows.push(...action.rows.map((r) => ({ ...r })));
  else w.insertedRows.splice(w.insertedRows.length - action.rows.length, action.rows.length);
  (redo ? w.undo : w.redo).push(action);
  w.selectedRows = w.selectedRows.filter(
    (i) => i < (w.result?.rows.length ?? 0) + w.insertedRows.length,
  );
  w.draftRevision++;
  w.notice = '';
}
export function clearHistory(w: TableWorkspace) {
  w.undo = [];
  w.redo = [];
  w.pendingCell = null;
  w.draftRevision++;
}
