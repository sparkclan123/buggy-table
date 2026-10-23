import { createRedisWorkspace, type RedisView } from './redis-workspace';
import type { ConnectionInfo, SavedConnection, Overview, ProfileLayout } from './types';
import {
  createWorkspace,
  pendingRows,
  schemaKey,
  tableKey,
  type TableWorkspace,
} from './workspace';
import type { VpnStatus } from './types';

export type Page = 'explorer' | 'overview' | 'privacy' | 'assistant';
export interface ConnectionWorkspace {
  key: string;
  info: ConnectionInfo;
  saved: SavedConnection | null;
  online: boolean;
  tabs: TableWorkspace[];
  activeId: string;
  page: Page;
  search: string;
  navSchema: string;
  navDatabase: string;
  databases: string[];
  databaseSchemas: Record<string, string[]>;
  expandedDatabases: string[];
  schemas: string[];
  loadedSchemas: string[];
  expandedSchemas: string[];
  overview: Overview | null;
  snapshots: Overview[];
  redis: RedisView;
  vpnStatus: VpnStatus | null;
}
export function connectionWorkspace(
  info: ConnectionInfo,
  saved: SavedConnection | null = null,
  online = true,
): ConnectionWorkspace {
  const postgres = info.engine === 'postgres' && info.id !== 'sample';
  const database = postgres ? info.database || undefined : undefined;
  if (postgres) info = { ...info, tables: info.tables.map((t) => ({ ...t, database })) };
  const tabs =
    saved?.layout.tabs.map((t) =>
      createWorkspace(
        {
          database: postgres ? (t.database ?? database) : undefined,
          schema: t.schema,
          name: t.table,
          columns: [],
          estimatedRows: null,
        },
        info.engine,
      ),
    ) ?? [];
  if (!tabs.length) tabs.push(createWorkspace());
  return {
    key: saved?.id ?? info.id,
    info,
    saved,
    online,
    tabs,
    activeId:
      tabs.find(
        (t) =>
          t.id === saved?.layout.activeTab ||
          (!saved?.layout.tabs.some((s) => s.database) &&
            tableKey(t.schema, t.selectedName) === saved?.layout.activeTab),
      )?.id ?? tabs[0].id,
    page: 'explorer',
    search: '',
    navSchema: saved?.layout.selectedSchema || info.schema,
    navDatabase: saved?.layout.selectedDatabase || database || '',
    databases: database ? [database] : [],
    databaseSchemas: Object.create(null) as Record<string, string[]>,
    expandedDatabases: database ? [database] : [],
    schemas: info.schema ? [info.schema] : [],
    loadedSchemas:
      online && info.schema && (!postgres || database) ? [schemaKey(info.schema, database)] : [],
    expandedSchemas:
      info.schema && (info.engine === 'mysql' || database)
        ? [schemaKey(info.schema, database)]
        : [],
    overview: null,
    snapshots: [],
    redis: createRedisWorkspace(),
    vpnStatus: null,
  };
}
export function savedWorkspace(saved: SavedConnection): ConnectionWorkspace {
  return connectionWorkspace(
    { ...saved, id: `offline:${saved.id}`, serverVersion: '', tables: [] },
    saved,
    false,
  );
}
export function hasEdits(entry: ConnectionWorkspace) {
  return entry.tabs.some((t) => pendingRows(t) > 0 || t.uncertain);
}
export function layoutOf(entry: ConnectionWorkspace): ProfileLayout {
  return {
    selectedSchema: entry.navSchema,
    selectedDatabase: entry.navDatabase || undefined,
    tabs: entry.tabs
      .filter((t) => t.selectedName)
      .map((t) => ({ database: t.database, schema: t.schema, table: t.selectedName })),
    activeTab: entry.activeId,
  };
}
export function engineLabel(engine: string) {
  return engine === 'mysql' ? 'MySQL' : engine === 'redis' ? 'Redis' : 'PostgreSQL';
}
