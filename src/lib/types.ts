export type Engine = 'postgres' | 'mysql' | 'redis';
export type TlsMode = 'verifyFull' | 'localOnly';
export interface ConnectRequest {
  engine: Engine;
  name: string;
  host: string;
  port: number;
  database: string;
  username: string;
  password: string;
  schema: string;
  tls: TlsMode;
  manualEdits: boolean;
}
export interface ColumnInfo {
  name: string;
  dataType: string;
  nullable: boolean;
  primaryKey: boolean;
  editable: boolean;
}
export interface TableInfo {
  schema: string;
  name: string;
  columns: ColumnInfo[];
  estimatedRows: number | null;
}
export interface ConnectionInfo {
  engine: Engine;
  id: string;
  name: string;
  host: string;
  port: number;
  database: string;
  username: string;
  schema: string;
  tls: TlsMode;
  manualEdits: boolean;
  serverVersion: string;
  tables: TableInfo[];
}
export interface QueryResult {
  columns: string[];
  rows: (string | null)[][];
  elapsedMs: number;
  truncated: boolean;
  sql: string;
}
export type FilterOperator =
  'equals' | 'notEquals' | 'greaterThan' | 'lessThan' | 'contains' | 'isNull' | 'isNotNull';
export type FilterMatch = 'all' | 'any';
export interface Filter {
  column: string;
  operator: FilterOperator;
  value: string;
}
export interface BrowseRequest {
  connectionId: string;
  requestId: string;
  table: string;
  filters: Filter[];
  filterMatch: FilterMatch;
  sortColumn: string | null;
  descending: boolean;
  page: number;
}
export interface IndexInfo {
  name: string;
  definition: string;
  scans: string;
  size: string;
  unique: boolean;
  primary: boolean;
}
export interface Overview {
  database: string;
  serverVersion: string;
  databaseSize: string;
  connections: number;
  activeConnections: number;
  transactionsCommitted: string;
  transactionsRolledBack: string;
  cacheHitPercent: number | null;
  statsReset: string | null;
  observedAt: string;
}
export interface AppError {
  code: string;
  message: string;
}

export interface RowChange {
  original: Record<string, string | null>;
  changes: Record<string, string | null>;
}
export interface SaveRequest {
  connectionId: string;
  requestId: string;
  table: string;
  rows: RowChange[];
}
export interface SaveResult {
  updatedRows: number;
}

export interface RedisResult {
  value: unknown;
  elapsedMs: number;
}
