import { expect, test, type Page } from '@playwright/test';

async function fixture(page: Page, engine: 'redis' | 'postgres' | 'mysql') {
  await page.addInitScript((engine) => {
    const target = window as unknown as Record<string, unknown>;
    target.isTauri = true;
    target.fixtureCalls = [];
    let snapshots = 0;
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, payload: { request?: Record<string, unknown> } = {}) => {
        if (command.startsWith('plugin:')) return 1;
        (target.fixtureCalls as string[]).push(command);
        if (command === 'connect_profile')
          return {
            profile: null,
            warning: null,
            connection: {
              id: 'fixture',
              name: 'Issue fixtures',
              engine,
              host: '127.0.0.1',
              port: 5432,
              database: engine === 'redis' ? '0' : 'fixture',
              username: 'reader',
              schema: 'public',
              tls: 'localOnly',
              manualEdits: false,
              serverVersion: engine === 'postgres' ? '170000' : '8.4',
              tables:
                engine === 'redis'
                  ? []
                  : [
                      {
                        name: 'items',
                        schema: 'public',
                        estimatedRows: 1234,
                        columns: [
                          {
                            name: 'id',
                            dataType: 'int4',
                            nullable: false,
                            primaryKey: true,
                            editable: false,
                          },
                        ],
                      },
                    ],
            },
          };
        if (command === 'browse_table')
          return {
            columns: ['id'],
            rows: [['1']],
            elapsedMs: 2,
            sql: 'SELECT …',
            truncated: false,
          };
        if (command === 'count_table_rows') return { rows: '987', elapsedMs: 3 };
        if (command === 'list_schemas') return ['public'];
        if (command === 'get_overview')
          return {
            database: 'fixture',
            serverVersion: '17',
            databaseSize: '1 MB',
            databaseSizeBytes: '1048576',
            connections: ++snapshots + 1,
            activeConnections: 1,
            transactionsCommitted: String(snapshots * 100),
            transactionsRolledBack: '0',
            cacheHitPercent: engine === 'mysql' ? null : 99.9,
            observedAt: '2026-10-19 12:00:00+00',
            statsReset: '2026-10-18',
          };
        if (command === 'redis_command') {
          const args = payload.request?.args as string[];
          if (args[0] === 'SCAN')
            return {
              elapsedMs: 1,
              value:
                args[3] === 'empty*'
                  ? ['9', []]
                  : args[1] === '0'
                    ? ['7', ['key:a', 'key:b']]
                    : ['0', ['key:b', 'key:c', { encoding: 'hex', data: 'fffe' }]],
            };
          if (args[0] === 'TYPE') return { elapsedMs: 1, value: 'string' };
          if (args[0] === 'TTL') return { elapsedMs: 1, value: '-1' };
          if (args[0] === 'GET')
            return { elapsedMs: 1, value: { preview: 'value for ' + args[1] } };
          if (args[0] === 'INFO') {
            if (target.denyInfo)
              throw { code: 'REDIS_PERMISSION', message: 'INFO is not allowed for this account.' };
            const info: Record<string, string> = {
              server: 'run_id:test\r\nuptime_in_seconds:100\r\nhz:10',
              memory:
                'used_memory:1000\r\nused_memory_rss:2000\r\nused_memory_peak:3000\r\nmem_fragmentation_ratio:2',
              stats:
                'instantaneous_ops_per_sec:3\r\ntotal_commands_processed:45\r\nkeyspace_hits:40\r\nkeyspace_misses:5',
              clients: 'connected_clients:3\r\nblocked_clients:0',
              keyspace: 'db0:keys=12,expires=4,avg_ttl=20',
            };
            return { elapsedMs: 1, value: info[args[1]] };
          }
        }
        return null;
      },
    };
  }, engine);
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect database', exact: true }).click();
  await page.getByLabel('Database engine').selectOption(engine);
  if (engine !== 'redis') await page.getByLabel('Database', { exact: true }).fill('fixture');
  await page.getByLabel('Username', { exact: true }).fill('reader');
  await page.getByRole('button', { name: 'Connect securely', exact: true }).click();
}

for (const engine of ['postgres', 'mysql'] as const)
  test(`${engine} approximate count never blocks page loading and snapshot graphs use manual data`, async ({
    page,
  }, testInfo) => {
    await fixture(page, engine);
    await expect(page.getByText('~1,234 total rows')).toBeVisible();
    expect(
      await page.evaluate(() => (window as unknown as { fixtureCalls: string[] }).fixtureCalls),
    ).not.toContain('count_table_rows');
    await page.getByRole('button', { name: 'Count exact' }).click();
    await expect(page.getByText('987 total rows')).toBeVisible();
    await page.getByRole('button', { name: 'Database overview', exact: true }).click();
    await expect(
      page.getByRole('img', { name: /Connections graph:.*1 manual snapshot/ }),
    ).toBeVisible();
    await page.getByRole('button', { name: 'Refresh snapshot', exact: true }).click();
    await expect(
      page.getByRole('img', { name: /Connections graph:.*2 manual snapshots/ }),
    ).toBeVisible();
    if (engine === 'mysql')
      await expect(page.getByText('Statistics unavailable for this metric.')).toBeVisible();
    for (const theme of ['light', 'dark']) {
      await page.getByLabel('Appearance', { exact: true }).selectOption(theme);
      await page.screenshot({
        path: testInfo.outputPath(`${engine}-graphs-${theme}.png`),
        fullPage: true,
      });
    }
  });

test('Redis scan accumulates key records and INFO graphs handle missing metrics and ACL denial', async ({
  page,
}, testInfo) => {
  await fixture(page, 'redis');
  await expect(page.getByRole('table', { name: 'Redis key records' })).toBeVisible();
  await page.getByRole('button', { name: 'Next scan', exact: true }).click();
  await expect(page.getByRole('button', { name: 'key:b', exact: true })).toHaveCount(1);
  await expect(page.getByText('4 keys loaded', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'key:c', exact: true }).click();
  await expect(page.getByLabel('Redis value')).toContainText('value for key:c');
  await page.getByLabel('Redis key pattern').fill('empty*');
  await page.getByLabel('Redis key pattern').press('Enter');
  await expect(page.getByText('No matches yet; continue with Next scan.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Next scan', exact: true })).toBeEnabled();
  await page.getByLabel('Redis command', { exact: true }).fill('["SCAN","0","MATCH","key:*"]');
  await page.getByRole('button', { name: 'Run command', exact: true }).click();
  await expect(page.getByRole('button', { name: 'key:a', exact: true })).toBeVisible();
  await expect(page.getByLabel('Redis value')).toHaveText('Select a key or run a read command.');
  await page.getByRole('button', { name: 'Database overview', exact: true }).click();
  for (const [section, label] of [
    ['server', 'Server uptime'],
    ['memory', 'Memory used'],
    ['stats', 'Operations'],
    ['clients', 'Connected clients'],
    ['keyspace', 'db0 keys'],
  ]) {
    await page.getByRole('button', { name: section, exact: true }).click();
    await expect(page.getByRole('img', { name: new RegExp(`${label} graph`) })).toBeVisible();
  }
  await page.getByRole('button', { name: 'memory', exact: true }).click();
  await page.screenshot({ path: testInfo.outputPath('redis-memory-graphs.png'), fullPage: true });
  await page.evaluate(() => {
    (window as unknown as Record<string, unknown>).denyInfo = true;
  });
  await page.getByRole('button', { name: 'clients', exact: true }).click();
  await expect(page.getByText(/Statistics unavailable. The connected account/)).toBeVisible();
  await expect(page.getByRole('img', { name: /graph/ })).toHaveCount(0);
});
