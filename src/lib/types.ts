export type Engine = 'postgres' | 'mysql' | 'redis';
export type TlsMode = 'verifyFull' | 'require' | 'localOnly';
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
  rootCaPem?: string;
  manualEdits: boolean;
}
export interface ColumnInfo {
  name: string;
  dataType: string;
  typeSchema?: string;
  nullable: boolean;
  primaryKey: boolean;
  editable: boolean;
}
export interface TableInfo {
  schema: string;
  name: string;
  columns: ColumnInfo[];
  estimatedRows: number | null;
  deletable?: boolean;
  relationKind?: string;
  readBlockedReason?: string | null;
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
  timings?: {
    connectionMs: number;
    connectionReused?: boolean;
    cleanupMs?: number;
    validationMs: number;
    executionMs: number;
  };
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
  schema?: string;
  connectionId: string;
  requestId: string;
  table: string;
  filters: Filter[];
  filterMatch: FilterMatch;
  sortColumn: string | null;
  descending: boolean;
  page: number;
  pageSize: number;
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
  databaseSizeBytes?: string;
  connections: number | null;
  activeConnections: number | null;
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

export interface RowCount {
  rows: string;
  elapsedMs: number;
}

export interface RowChange {
  delete?: boolean;
  original: Record<string, string | null>;
  changes: Record<string, string | null>;
}
export interface SaveRequest {
  schema?: string;
  connectionId: string;
  requestId: string;
  table: string;
  rows: RowChange[];
}
export interface SaveResult {
  updatedRows: number;
  deletedRows?: number;
}

export interface RedisResult {
  value: unknown;
  elapsedMs: number;
}

export interface ProfileLayout {
  selectedSchema: string;
  tabs: { schema: string; table: string }[];
  activeTab: string;
}
export interface SavedConnection extends Omit<ConnectRequest, 'password'> {
  id: string;
  group: string;
  pinned: boolean;
  lastUsed: number;
  hasPassword: boolean;
  layout: ProfileLayout;
}
export interface ConnectedProfile {
  connection: ConnectionInfo;
  profile: SavedConnection | null;
  warning: string | null;
}
