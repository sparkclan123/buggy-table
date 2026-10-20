import { createRedisWorkspace, type RedisView } from './redis-workspace';
import type { ConnectionInfo, SavedConnection, Overview, ProfileLayout } from './types';
import { createWorkspace, pendingRows, type TableWorkspace } from './workspace';
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
  const tabs =
    saved?.layout.tabs.map((t) =>
      createWorkspace(
        { schema: t.schema, name: t.table, columns: [], estimatedRows: null },
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
    activeId: tabs.find((t) => t.id === saved?.layout.activeTab)?.id ?? tabs[0].id,
    page: 'explorer',
    search: '',
    navSchema: saved?.layout.selectedSchema || info.schema,
    schemas: info.schema ? [info.schema] : [],
    loadedSchemas: online && info.schema ? [info.schema] : [],
    expandedSchemas: info.engine === 'mysql' && info.schema ? [info.schema] : [],
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
    tabs: entry.tabs
      .filter((t) => t.selectedName)
      .map((t) => ({ schema: t.schema, table: t.selectedName })),
    activeTab: entry.activeId,
  };
}
export function engineLabel(engine: string) {
  return engine === 'mysql' ? 'MySQL' : engine === 'redis' ? 'Redis' : 'PostgreSQL';
}
