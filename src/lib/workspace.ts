import type { Filter, FilterMatch, IndexInfo, QueryResult, RowChange, TableInfo } from './types';
import { quoteIdentifier } from './api';

export const tableKey = (schema: string, table: string) => JSON.stringify([schema, table]);

export type TableView = 'data' | 'structure' | 'indexes' | 'sql';
export type CellEdits = Record<number, Record<string, string | null>>;
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
    scrollTop: 0,
    lastAccess: Date.now(),
    uncertain: false,
    notice: '',
  };
}
export type TableWorkspace = ReturnType<typeof createWorkspace>;

export function pendingRows(workspace: TableWorkspace) {
  return new Set([...Object.keys(workspace.edits).map(Number), ...workspace.deletedRows]).size;
}

export function changedRows(
  result: QueryResult | null,
  edits: CellEdits,
  deleted: number[] = [],
): RowChange[] {
  if (!result) return [];
  return [...new Set([...Object.keys(edits).map(Number), ...deleted])].map((row) => ({
    original: Object.fromEntries(
      result.columns.map((column, index) => [column, result.rows[row][index]]),
    ),
    changes: deleted.includes(row) ? {} : { ...edits[row] },
    ...(deleted.includes(row) ? { delete: true } : {}),
  }));
}

export function changeCell(
  workspace: TableWorkspace,
  row: number,
  column: string,
  value: string | null,
) {
  if (!workspace.result || workspace.deletedRows.includes(row)) return;
  const original = workspace.result.rows[row][workspace.result.columns.indexOf(column)];
  if (value === original) {
    delete workspace.edits[row]?.[column];
    if (workspace.edits[row] && !Object.keys(workspace.edits[row]).length)
      delete workspace.edits[row];
  } else {
    workspace.edits[row] ??= {};
    workspace.edits[row][column] = value;
  }
  workspace.notice = '';
}
