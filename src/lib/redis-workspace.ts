export function createRedisWorkspace() {
  return {
    pattern: '*',
    cursor: '0',
    scanned: false,
    keys: [] as string[],
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
