import type { Filter, FilterMatch, IndexInfo, QueryResult, RowChange, TableInfo } from './types';
import { quoteIdentifier } from './api';

export type TableView = 'data' | 'structure' | 'indexes' | 'sql';
export type CellEdits = Record<number, Record<string, string | null>>;
export function createWorkspace(table?: TableInfo, engine = 'postgres') {
  return {
    id: table ? `${table.schema}.${table.name}` : 'empty',
    selectedName: table?.name ?? '',
    tab: (table ? 'data' : 'sql') as TableView,
    showFilter: false,
    filters: [{ column: table?.columns[0]?.name ?? '', operator: 'equals', value: '' }] as Filter[],
    filterMatch: 'all' as FilterMatch,
    appliedFilters: [] as Filter[],
    appliedFilterMatch: 'all' as FilterMatch,
    sortColumn: null as string | null,
    descending: false,
    columnWidths: {} as Record<string, number>,
    pageNumber: 0,
    result: null as QueryResult | null,
    queryResult: null as QueryResult | null,
    indexes: [] as IndexInfo[],
    sql: table
      ? `SELECT *\nFROM ${quoteIdentifier(table.schema, engine)}.${quoteIdentifier(table.name, engine)}\nLIMIT 200;`
      : 'SELECT 1;',
    plan: null as string | null,
    edits: {} as CellEdits,
    scrollTop: 0,
    uncertain: false,
    notice: '',
  };
}
export type TableWorkspace = ReturnType<typeof createWorkspace>;

export function changedRows(result: QueryResult | null, edits: CellEdits): RowChange[] {
  if (!result) return [];
  return Object.entries(edits).map(([row, changes]) => ({
    original: Object.fromEntries(
      result.columns.map((column, index) => [column, result.rows[Number(row)][index]]),
    ),
    changes: { ...changes },
  }));
}

export function changeCell(
  workspace: TableWorkspace,
  row: number,
  column: string,
  value: string | null,
) {
  if (!workspace.result) return;
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
