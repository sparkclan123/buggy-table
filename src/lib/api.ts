import { invoke, isTauri } from '@tauri-apps/api/core';
import type {
  AppError,
  SaveRequest,
  SaveResult,
  BrowseRequest,
  ConnectRequest,
  ConnectionInfo,
  IndexInfo,
  Overview,
  QueryResult,
} from './types';

export const desktopAvailable = isTauri();

async function call<T>(command: string, args: Record<string, unknown>): Promise<T> {
  if (!desktopAvailable)
    throw {
      code: 'DESKTOP_REQUIRED',
      message:
        'Open the desktop app to connect to PostgreSQL. This browser preview only contains local sample data.',
    } satisfies AppError;
  return invoke<T>(command, args);
}

export const api = {
  save: (request: SaveRequest) => call<SaveResult>('save_table_changes', { request }),
  connect: (request: ConnectRequest) => call<ConnectionInfo>('connect', { request }),
  disconnect: (connectionId: string) => call<void>('disconnect', { connectionId }),
  query: (connectionId: string, requestId: string, sql: string, explain = false) =>
    call<QueryResult>('run_query', { request: { connectionId, requestId, sql, explain } }),
  cancel: (connectionId: string, requestId: string) =>
    call<void>('cancel_query', { connectionId, requestId }),
  browse: (request: BrowseRequest) => call<QueryResult>('browse_table', { request }),
  indexes: (connectionId: string, table: string) =>
    call<IndexInfo[]>('inspect_table', { connectionId, table }),
  overview: (connectionId: string) => call<Overview>('get_overview', { connectionId }),
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

export function quoteIdentifier(value: string): string {
  return `"${value.replaceAll('"', '""')}"`;
}
