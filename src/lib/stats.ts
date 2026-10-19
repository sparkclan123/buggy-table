export interface SnapshotPoint {
  value: number;
  at: string;
}
export function numeric(value: unknown): number | null {
  if (typeof value !== 'string' && typeof value !== 'number') return null;
  if (String(value).trim() === '') return null;
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? number : null;
}
export function parseRedisInfo(value: unknown): Record<string, string> {
  if (typeof value !== 'string') throw new Error('Redis INFO did not return a text snapshot.');
  const fields: Record<string, string> = Object.create(null);
  for (const line of value.split(/\r?\n/)) {
    if (!line || line.startsWith('#')) continue;
    const colon = line.indexOf(':');
    if (colon > 0) fields[line.slice(0, colon)] = line.slice(colon + 1);
  }
  if (!Object.keys(fields).length && !/^# Keyspace\s*$/m.test(value))
    throw new Error('This Redis INFO section is unavailable to the connected account.');
  return fields;
}
export const redisMetrics: Record<string, { key: string; label: string; unit: string }[]> = {
  server: [
    { key: 'uptime_in_seconds', label: 'Server uptime', unit: 'seconds' },
    { key: 'hz', label: 'Server frequency', unit: 'Hz' },
  ],
  memory: [
    { key: 'used_memory', label: 'Memory used', unit: 'bytes' },
    { key: 'used_memory_rss', label: 'Resident memory', unit: 'bytes' },
    { key: 'used_memory_peak', label: 'Peak memory', unit: 'bytes' },
    { key: 'mem_fragmentation_ratio', label: 'Fragmentation ratio', unit: 'ratio' },
  ],
  stats: [
    { key: 'instantaneous_ops_per_sec', label: 'Operations', unit: 'ops/s' },
    { key: 'total_commands_processed', label: 'Commands processed', unit: 'commands' },
    { key: 'keyspace_hits', label: 'Keyspace hits', unit: 'hits' },
    { key: 'keyspace_misses', label: 'Keyspace misses', unit: 'misses' },
  ],
  clients: [
    { key: 'connected_clients', label: 'Connected clients', unit: 'clients' },
    { key: 'blocked_clients', label: 'Blocked clients', unit: 'clients' },
    { key: 'tracking_clients', label: 'Tracking clients', unit: 'clients' },
  ],
};
