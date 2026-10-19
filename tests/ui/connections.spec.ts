import { editCell } from './grid-helpers';
import { test, expect, type Page } from '@playwright/test';

// IPC doubles exercise UI ownership. Rust tests use real PostgreSQL schemas and roles.
async function connections(page: Page, restoreTabs = false, manyTabs = false) {
  await page.addInitScript(
    ({ restoreTabs, manyTabs }) => {
      const target = window as unknown as Record<string, unknown>;
      target.isTauri = true;
      const calls: { command: string; id?: string; schema?: string }[] = [];
      target.connectionCalls = calls;
      const profile = (id: string, name: string, engine = 'postgres', hasPassword = true) => ({
        id,
        name,
        engine,
        host: '127.0.0.1',
        port: engine === 'redis' ? 6379 : 5432,
        database: engine === 'redis' ? '0' : 'fixture',
        username: 'reader',
        schema: engine === 'redis' ? '0' : 'public',
        tls: 'localOnly',
        rootCaPem: '',
        manualEdits: engine === 'postgres',
        group: 'Testing',
        pinned: id === 'a',
        lastUsed: id === 'a' ? 100 : 1,
        hasPassword,
        layout: { selectedSchema: 'public', tabs: [], activeTab: 'empty' },
      });
      const profiles = [
        profile('a', 'Alpha'),
        profile('b', 'Beta'),
        profile('redis', 'Cache', 'redis', false),
        profile('bad', 'Unavailable'),
        ...Array.from({ length: 50 }, (_, i) => profile('client' + i, `Client ${i + 1}`, 'mysql')),
      ];
      const rows: Record<string, string> = {};
      if (restoreTabs)
        profiles[0].layout = {
          selectedSchema: 'audit',
          tabs: [
            { schema: 'public', table: 'items' },
            { schema: 'audit', table: 'items' },
          ],
          activeTab: JSON.stringify(['audit', 'items']),
        } as (typeof profiles)[0]['layout'];
      if (manyTabs)
        profiles[0].layout = {
          selectedSchema: 'public',
          tabs: ['public', 'audit'].flatMap((schema) =>
            ['items', 'json_items', 'report_view'].map((table) => ({ schema, table })),
          ),
          activeTab: JSON.stringify(['public', 'items']),
        } as (typeof profiles)[0]['layout'];
      const tables = (schema: string) => [
        {
          schema,
          name: 'items',
          estimatedRows: 1,
          columns: [
            { name: 'id', dataType: 'int8', nullable: false, primaryKey: true, editable: false },
            { name: 'label', dataType: 'text', nullable: false, primaryKey: false, editable: true },
          ],
        },
        {
          schema,
          name: 'json_items',
          estimatedRows: 1,
          columns: [
            {
              name: 'payload',
              dataType: 'jsonb',
              nullable: true,
              primaryKey: false,
              editable: false,
            },
          ],
        },
        {
          schema,
          name: 'report_view',
          estimatedRows: null,
          relationKind: 'view',
          readBlockedReason:
            'View definitions are available for structure inspection. Reading is not enabled.',
          columns: [
            { name: 'label', dataType: 'text', nullable: true, primaryKey: false, editable: false },
          ],
        },
        ...(target.extraTable
          ? [
              {
                schema,
                name: 'new_table',
                estimatedRows: 0,
                columns: [
                  {
                    name: 'id',
                    dataType: 'int8',
                    nullable: false,
                    primaryKey: true,
                    editable: false,
                  },
                ],
              },
            ]
          : []),
      ];
      const connected = (p: (typeof profiles)[number]) => ({
        profile: p,
        warning: null,
        connection: {
          ...p,
          id: 'session-' + p.id,
          serverVersion: '180006',
          tables: p.engine === 'redis' ? [] : tables('public'),
        },
      });
      target.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: 'main' } },
        transformCallback: () => 1,
        invoke: async (command: string, payload: Record<string, unknown> = {}) => {
          if (command.startsWith('plugin:')) return 1;
          if (command === 'check_update') return { configured: false, currentVersion: '0.1.3' };
          const r = (payload.request ?? {}) as Record<string, unknown>;
          calls.push({
            command,
            id: String(payload.id ?? payload.connectionId ?? r.connectionId ?? ''),
            schema: String(payload.schema ?? r.schema ?? ''),
          });
          if (command === 'list_connections') return profiles;
          if (command === 'reconnect_profile') {
            if (payload.id === 'bad')
              throw { code: 'CONNECTION_FAILED', message: 'Fixture server is unavailable.' };
            return connected(profiles.find((p) => p.id === payload.id)!);
          }
          if (command === 'connect_profile') {
            const p = profile(String(payload.savedId ?? 'new'), String(r.name), String(r.engine));
            Object.assign(p, r);
            delete (p as Record<string, unknown>).password;
            const old = profiles.findIndex((x) => x.id === p.id);
            if (old < 0) profiles.push(p);
            else profiles[old] = p;
            return connected(p);
          }
          if (command === 'list_schemas') return ['public', 'audit'];
          if (command === 'schema_tables') return tables(String(payload.schema));
          if (command === 'browse_table' || command === 'run_query')
            return {
              columns: ['id', 'label'],
              rows: [
                [
                  '1',
                  rows[String(r.connectionId) + ':' + String(r.schema)] ?? `${r.schema} original`,
                ],
              ],
              sql: 'SELECT …',
              elapsedMs: 47,
              timings: {
                connectionMs: 0,
                connectionReused: true,
                cleanupMs: 2,
                validationMs: 40,
                executionMs: 5,
              },
              truncated: false,
            };
          if (command === 'save_table_changes') {
            rows[String(r.connectionId) + ':' + String(r.schema)] = (
              r.rows as { changes: { label: string } }[]
            )[0].changes.label;
            return { updatedRows: 1 };
          }
          if (command === 'update_connection') {
            const p = profiles.find((p) => p.id === payload.id)!;
            p.layout = payload.layout as typeof p.layout;
            p.pinned = Boolean(payload.pinned);
            p.group = String(payload.group);
            target.persistedProfiles = profiles;
            return null;
          }
          if (command === 'remove_connection') {
            profiles.splice(
              profiles.findIndex((p) => p.id === payload.id),
              1,
            );
            return null;
          }
          if (command === 'redis_command') {
            const args = r.args as string[];
            if (args[0] === 'SCAN') return { value: ['0', ['fixture:key']], elapsedMs: 1 };
            if (args[0] === 'TYPE') return { value: 'string', elapsedMs: 1 };
            if (args[0] === 'TTL') return { value: 30, elapsedMs: 1 };
            return { value: 'redis fixture', elapsedMs: 1 };
          }
          return null;
        },
      };
    },
    { restoreTabs, manyTabs },
  );
}
async function start(page: Page) {
  await connections(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Reconnect Alpha', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'items TABLE' })).toBeVisible();
}

test('PostgreSQL result time includes connection and validation in both data and SQL views', async ({
  page,
}) => {
  await start(page);
  const timing = page.getByText('47 ms total', { exact: true });
  await expect(timing).toBeVisible();
  await expect(timing).toHaveAttribute(
    'title',
    /Connection: 0 ms \(reused\).*Checks and schema: 40 ms.*Query and transfer: 5 ms.*Cleanup: 2 ms/,
  );
  await page.getByRole('tab', { name: 'SQL editor', exact: true }).click();
  await page.getByRole('button', { name: 'Run query', exact: true }).click();
  await expect(timing).toBeVisible();
  await expect(timing).toHaveAttribute('title', /excludes UI rendering/);
});

test('schema lists JSON tables and restricted views, and refresh discovers new tables without losing drafts', async ({
  page,
}) => {
  await start(page);
  await expect(page.getByRole('button', { name: 'json_items', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'report_view', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'Structure', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(page.getByRole('status')).toContainText('structure inspection');
  await expect(page.getByRole('tab', { name: 'Data', exact: true })).toBeDisabled();
  await page.getByRole('tab', { name: 'items', exact: true }).click();
  await editCell(page, 1, 'label');
  await page.getByRole('textbox', { name: 'Cell value row 1 label' }).fill('keep draft');
  await page.keyboard.press('Enter');
  await page.evaluate(() => {
    (window as unknown as Record<string, unknown>).extraTable = true;
  });
  const refresh = page.getByRole('button', { name: 'Refresh schema', exact: true });
  await expect(refresh).toHaveText('');
  const refreshBox = (await refresh.boundingBox())!;
  const selectBox = (await page.getByLabel('Select PostgreSQL schema').boundingBox())!;
  expect(refreshBox.x + refreshBox.width).toBeLessThan(selectBox.x);
  expect(
    Math.abs(refreshBox.y + refreshBox.height / 2 - selectBox.y - selectBox.height / 2),
  ).toBeLessThan(2);
  await refresh.click();
  await expect(page.getByRole('button', { name: 'new_table', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'keep draft',
  );
});

test('saved profiles load offline; search finds distant connections without connecting', async ({
  page,
}) => {
  await connections(page);
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Alpha', exact: true })).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { connectionCalls: { command: string }[] }).connectionCalls.filter(
        (c) => c.command.includes('connect_profile'),
      ),
    ),
  ).toEqual([]);
  await page.keyboard.press('Control+k');
  await expect(page.getByLabel('Search connections')).toBeFocused();
  await page.getByLabel('Search connections').fill('Client 49');
  await expect(page.getByRole('button', { name: /Client 49.*Disconnected/ })).toBeVisible();
  await page.getByRole('button', { name: /Client 49.*Disconnected/ }).click();
  await expect(page.getByRole('heading', { name: 'Client 49', exact: true })).toBeVisible();
});

test('reopening restores table identities lazily and reconnects the active schema', async ({
  page,
}) => {
  await connections(page, true);
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Alpha', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Reconnect Alpha', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'items · audit', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'audit original',
  );
  await page.getByRole('tab', { name: 'items', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'public original',
  );
});

test('switching connections preserves unsaved cells and guards closing across all connections', async ({
  page,
}) => {
  await start(page);
  await editCell(page, 1, 'label');
  await page.getByRole('textbox', { name: 'Cell value row 1 label' }).fill('Draft in Alpha');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'Reconnect Beta', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'public original',
  );
  const prevented = await page.evaluate(
    () => !window.dispatchEvent(new Event('beforeunload', { cancelable: true })),
  );
  expect(prevented).toBe(true);
  await page.getByRole('button', { name: 'Switch to Alpha', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'Draft in Alpha',
  );
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  const calls = await page.evaluate(
    () =>
      (window as unknown as { connectionCalls: { command: string; id: string; schema: string }[] })
        .connectionCalls,
  );
  expect(calls.filter((c) => c.command === 'disconnect')).toHaveLength(0);
  expect(calls.find((c) => c.command === 'save_table_changes')).toMatchObject({
    id: 'session-a',
    schema: 'public',
  });
});

test('same-named tables in separate schemas retain tabs and save to their owning schema', async ({
  page,
}) => {
  await start(page);
  await page.getByRole('combobox', { name: 'Select PostgreSQL schema' }).selectOption('audit');
  await page.getByRole('button', { name: 'items', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'items · audit', exact: true })).toBeVisible();
  await editCell(page, 1, 'label');
  await page.getByRole('textbox', { name: 'Cell value row 1 label' }).fill('Audit changed');
  await page.keyboard.press('Enter');
  await page.getByRole('tab', { name: 'items', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'public original',
  );
  await page.getByRole('tab', { name: /^items · audit/ }).click();
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  const calls = await page.evaluate(
    () =>
      (window as unknown as { connectionCalls: { command: string; schema: string }[] })
        .connectionCalls,
  );
  expect(calls.find((c) => c.command === 'save_table_changes')?.schema).toBe('audit');
  const persisted = await page.evaluate(() =>
    JSON.stringify((window as unknown as Record<string, unknown>).persistedProfiles),
  );
  expect(persisted).not.toContain('Audit changed');
  expect(persisted).not.toContain('SELECT');
  expect(persisted).toContain('audit');
});

test('password reconnect prefills metadata and Redis state survives switching', async ({
  page,
}) => {
  await start(page);
  await page.getByRole('button', { name: 'Reconnect Cache', exact: true }).click();
  await expect(page.getByLabel('Host', { exact: true })).toHaveValue('127.0.0.1');
  await expect(page.getByLabel('Password', { exact: true })).toHaveValue('');
  await page.getByLabel('Password', { exact: true }).fill('FIXTURE_ONLY');
  await page.getByRole('button', { name: 'Connect securely' }).click();
  await page.getByRole('button', { name: 'fixture:key', exact: true }).click();
  await expect(page.getByLabel('Redis value')).toContainText('redis fixture');
  await page.getByRole('button', { name: 'Switch to Alpha', exact: true }).click();
  await page.getByRole('button', { name: 'Switch to Cache', exact: true }).click();
  await expect(page.getByLabel('Redis value')).toContainText('redis fixture');
  expect(
    await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length })),
  ).toEqual({ local: 0, session: 0 });
});

test('reconnect failure keeps the saved profile and other active connection intact', async ({
  page,
}) => {
  await start(page);
  await page.getByRole('button', { name: 'Reconnect Unavailable', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Close connection dialog' }).click();
  await expect(page.getByRole('alert')).toContainText('unavailable');
  await page.getByRole('button', { name: 'Switch to Alpha', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'items TABLE' })).toBeVisible();
});

test('tab drag autoscrolls an overflowing strip and persists only tab identities in the saved order', async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 700 });
  await connections(page, false, true);
  await page.goto('/');
  await page.getByRole('button', { name: 'Reconnect Alpha', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'items TABLE' })).toBeVisible();
  await expect(page.getByLabel('Rows per page')).toHaveValue('100');
  await editCell(page, 1, 'label');
  await page.getByRole('textbox', { name: 'Cell value row 1 label' }).fill('Reordered draft');
  await page.keyboard.press('Enter');
  const strip = page.getByRole('tablist', { name: 'Open tables' });
  const before = await page.evaluate(
    () =>
      (window as unknown as { connectionCalls: { command: string }[] }).connectionCalls.filter(
        (c) => c.command === 'browse_table',
      ).length,
  );
  const source = (await strip
    .getByRole('tab', { name: 'items Unsaved changes', exact: true })
    .boundingBox())!;
  const bounds = (await strip.boundingBox())!;
  expect(await strip.evaluate((e) => e.scrollWidth > e.clientWidth)).toBe(true);
  expect(await strip.evaluate((e) => getComputedStyle(e, '::-webkit-scrollbar').height)).toBe(
    '3px',
  );
  await page.mouse.move(source.x + 15, source.y + 15);
  await page.mouse.down();
  await page.mouse.move(bounds.x + bounds.width - 2, source.y + 15, { steps: 8 });
  await expect
    .poll(() => strip.evaluate((e) => e.scrollWidth - e.clientWidth - e.scrollLeft))
    .toBeLessThan(3);
  await page.mouse.up();
  await expect(strip.getByRole('tab').last()).toContainText('items');
  await expect(strip.getByRole('tab').last()).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'Reordered draft',
  );
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { connectionCalls: { command: string }[] }).connectionCalls.filter(
          (c) => c.command === 'browse_table',
        ).length,
    ),
  ).toBe(before);
  const saved = await page.evaluate(
    () =>
      (
        window as unknown as {
          persistedProfiles: {
            id: string;
            layout: { tabs: { schema: string; table: string }[]; activeTab: string };
          }[];
        }
      ).persistedProfiles.find((p) => p.id === 'a')!.layout,
  );
  expect(saved.tabs).toEqual([
    { schema: 'public', table: 'json_items' },
    { schema: 'public', table: 'report_view' },
    { schema: 'audit', table: 'items' },
    { schema: 'audit', table: 'json_items' },
    { schema: 'audit', table: 'report_view' },
    { schema: 'public', table: 'items' },
  ]);
  expect(saved.activeTab).toBe(JSON.stringify(['public', 'items']));
  expect(JSON.stringify(saved)).not.toContain('Reordered draft');
});
