import { invoke, isTauri } from '@tauri-apps/api/core';
import type {
  AppError,
  RowCount,
  SavedConnection,
  ConnectedProfile,
  ProfileLayout,
  RedisResult,
  SaveRequest,
  SaveResult,
  BrowseRequest,
  ConnectRequest,
  TableInfo,
  ConnectionInfo,
  IndexInfo,
  Overview,
  QueryResult,
  VpnSelection,
  VpnProfile,
  VpnEdit,
  VpnStatus,
  VpnRuntimeInfo,
  ChangedNetwork,
} from './types';

export const desktopAvailable = isTauri();

async function call<T>(command: string, args: Record<string, unknown>): Promise<T> {
  if (!desktopAvailable)
    throw {
      code: 'DESKTOP_REQUIRED',
      message:
        'Open the desktop app to connect to a database. This browser preview only contains local sample data.',
    } satisfies AppError;
  return invoke<T>(command, args);
}

export const api = {
  connections: () => call<SavedConnection[]>('list_connections', {}),
  connectProfile: (
    request: ConnectRequest,
    savedId: string | null,
    group: string,
    remember: boolean,
    savePassword: boolean,
    vpn?: VpnSelection,
  ) =>
    call<ConnectedProfile>('connect_profile', {
      request,
      savedId,
      group,
      remember,
      savePassword,
      vpn,
    }),
  vpnProfiles: (owner: string) => call<VpnProfile[]>('vpn_profiles', { owner }),
  discardVpnDraft: (owner: string) => call<void>('discard_vpn_draft', { owner }),
  saveVpnProfile: (edit: VpnEdit) => call<VpnProfile>('save_vpn_profile', { edit }),
  removeVpnProfile: (owner: string, id: string) =>
    call<SavedConnection | null>('remove_vpn_profile', { owner, id }),
  vpnRuntime: () => call<VpnRuntimeInfo>('vpn_runtime', {}),
  prepareVpn: () => call<void>('prepare_vpn', {}),
  vpnStatus: (connectionId: string, probe = false) =>
    call<VpnStatus>('vpn_status', { connectionId, probe }),
  setConnectionVpn: (owner: string, selection: VpnSelection, connectionId: string | null) =>
    call<ChangedNetwork>('set_connection_vpn', { owner, selection, connectionId }),
  reconnect: (id: string) => call<ConnectedProfile>('reconnect_profile', { id }),
  updateConnection: (id: string, layout: ProfileLayout, pinned: boolean, group: string) =>
    call<void>('update_connection', { id, layout, pinned, group }),
  removeConnection: (id: string) => call<void>('remove_connection', { id }),
  databases: (connectionId: string) => call<string[]>('list_databases', { connectionId }),
  schemas: (connectionId: string, database?: string) =>
    call<string[]>('list_schemas', { connectionId, database }),
  schemaTables: (connectionId: string, schema: string, database?: string) =>
    call<TableInfo[]>('schema_tables', { connectionId, schema, database }),
  redis: (connectionId: string, requestId: string, args: string[]) =>
    call<RedisResult>('redis_command', { request: { connectionId, requestId, args } }),
  save: (request: SaveRequest) => call<SaveResult>('save_table_changes', { request }),
  connect: (request: ConnectRequest) => call<ConnectionInfo>('connect', { request }),
  disconnect: (connectionId: string) => call<void>('disconnect', { connectionId }),
  query: (
    connectionId: string,
    requestId: string,
    sql: string,
    explain = false,
    schema?: string,
    database?: string,
  ) =>
    call<QueryResult>('run_query', {
      request: { connectionId, requestId, sql, explain, schema, database },
    }),
  cancel: (connectionId: string, requestId: string) =>
    call<void>('cancel_query', { connectionId, requestId }),
  count: (request: BrowseRequest) => call<RowCount>('count_table_rows', { request }),
  browse: (request: BrowseRequest) => call<QueryResult>('browse_table', { request }),
  indexes: (connectionId: string, table: string, schema?: string, database?: string) =>
    call<IndexInfo[]>('inspect_table', { connectionId, table, schema, database }),
  overview: (connectionId: string, database?: string) =>
    call<Overview>('get_overview', { connectionId, database }),
};

export function errorMessage(error: unknown): string {
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
  )
    return error.message;
  return 'The operation could not be completed. Please reconnect and try again.';
}

export function quoteIdentifier(value: string, engine: string = 'postgres'): string {
  if (engine === 'mysql') return `\`${value.replaceAll('`', '``')}\``;
  return `"${value.replaceAll('"', '""')}"`;
}
