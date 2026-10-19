export function createRedisWorkspace() {
  return {
    pattern: '*',
    cursor: '0',
    scanned: false,
    keys: [] as { id: string; label: string; value: string | null }[],
    infoSection: 'server',
    info: null as Record<string, string> | null,
    infoAt: '',
    infoHistory: {} as Record<string, { at: string; fields: Record<string, string> }[]>,
    selected: '',
    keyType: '',
    ttl: '',
    output: null as unknown,
    hasOutput: false,
    appliedPattern: '*',
    error: '',
    elapsed: 0,
    command: '["PING"]',
  };
}
export type RedisView = ReturnType<typeof createRedisWorkspace>;
