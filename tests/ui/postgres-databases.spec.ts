import { expect, test, type Page } from '@playwright/test';
import { editCell } from './grid-helpers';

async function setup(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as Record<string, any>;
    w.isTauri = true;
    w.pgCalls = [];
    const rows: Record<string, string> = {};
    let saved: Record<string, any> | null = null;
    const connected = () => ({
      profile: saved,
      warning: null,
      connection: { ...saved, id: 'pg-server', serverVersion: '180006', database: '', tables: [] },
    });
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, args: Record<string, any> = {}) => {
        if (command.startsWith('plugin:')) return 1;
        w.pgCalls.push({ command, args });
        if (command === 'list_connections') return saved ? [saved] : [];
        if (command === 'connect_profile') {
          const { password, ...metadata } = args.request;
          saved = {
            ...metadata,
            id: 'saved-pg',
            name: 'PG server',
            hasPassword: true,
            pinned: false,
            group: '',
            lastUsed: 1,
            layout: { selectedSchema: 'public', activeTab: 'empty', tabs: [] },
          };
          return connected();
        }
        if (command === 'reconnect_profile') return connected();
        if (command === 'update_connection') {
          saved!.layout = args.layout;
          w.pgLayout = args.layout;
          return null;
        }
        if (command === 'list_databases')
          return w.pgNames ?? ['alpha', 'beta', 'empty', '__proto__'];
        if (command === 'list_schemas') {
          if (w.failDatabase === args.database)
            throw { message: 'This database is temporarily unavailable.' };
          return args.database === 'empty' ? [] : ['public', 'audit'];
        }
        if (command === 'schema_tables') {
          if (w.failSchema === args.schema) throw { message: 'Schema access unavailable.' };
          return [
            {
              schema: args.schema,
              name: 'items',
              estimatedRows: 1,
              deletable: true,
              columns: [
                {
                  name: 'id',
                  dataType: 'int4',
                  primaryKey: true,
                  editable: false,
                  nullable: false,
                },
                {
                  name: 'label',
                  dataType: 'text',
                  primaryKey: false,
                  editable: true,
                  nullable: false,
                },
              ],
            },
          ];
        }
        const r = args.request;
        if (command === 'browse_table' || command === 'run_query')
          return {
            columns: ['id', 'label'],
            rows: [['1', rows[`${r.database}/${r.schema}`] ?? `${r.database}/${r.schema}`]],
            elapsedMs: 2,
            truncated: false,
            sql: 'SELECT …',
          };
        if (command === 'save_table_changes') {
          rows[`${r.database}/${r.schema}`] = r.rows[0].changes.label;
          return { updatedRows: 1 };
        }
        if (command === 'count_table_rows') return { rows: '1', elapsedMs: 1 };
        if (command === 'inspect_table') return [];
        return null;
      },
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect database', exact: true }).click();
  await expect(page.getByLabel('Default database (optional)')).toHaveValue('');
  await page.getByLabel('Username', { exact: true }).fill('fixture');
  await page.getByRole('button', { name: 'Connect securely', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
}
async function calls(page: Page, command: string) {
  return page.evaluate(
    (command) =>
      (window as unknown as { pgCalls: { command: string; args: any }[] }).pgCalls
        .filter((c) => c.command === command)
        .map((c) => c.args),
    command,
  );
}
const table = (page: Page, database: string, schema = 'public') =>
  page
    .getByLabel(`Tables in ${database}.${schema}`, { exact: true })
    .getByRole('button', { name: 'items', exact: true });

test('PostgreSQL server browser loads lazily, isolates same-named tables and persists database identities', async ({
  page,
}, info) => {
  await setup(page);
  expect((await calls(page, 'connect_profile'))[0].request.database).toBe('');
  expect(await calls(page, 'list_schemas')).toEqual([]);
  expect(await calls(page, 'schema_tables')).toEqual([]);
  await page.getByRole('button', { name: 'Expand database alpha', exact: true }).press('Enter');
  await table(page, 'alpha').click();
  await editCell(page, 1, 'label');
  await page.getByRole('textbox', { name: 'Cell value row 1 label' }).fill('alpha draft');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'Expand database beta', exact: true }).click();
  await table(page, 'beta').click();
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'beta/public',
  );
  await page.getByRole('button', { name: 'Count exact', exact: true }).click();
  expect((await calls(page, 'count_table_rows'))[0].request.database).toBe('beta');
  await page.getByRole('tab', { name: 'Indexes', exact: true }).click();
  expect((await calls(page, 'inspect_table'))[0].database).toBe('beta');
  await page.getByRole('tab', { name: 'SQL editor', exact: true }).click();
  await page.getByRole('button', { name: 'Run query', exact: true }).click();
  expect((await calls(page, 'run_query'))[0].request.database).toBe('beta');
  await page.getByRole('tab', { name: /^items · alpha/ }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'alpha draft',
  );
  expect(await calls(page, 'browse_table')).toHaveLength(2);
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  expect((await calls(page, 'save_table_changes'))[0].request).toMatchObject({
    database: 'alpha',
    schema: 'public',
  });
  await page.getByRole('button', { name: 'Expand schema beta.audit', exact: true }).click();
  await table(page, 'beta', 'audit').click();
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'beta/audit',
  );
  await page.getByRole('button', { name: 'Collapse database alpha', exact: true }).click();
  await page.getByRole('button', { name: 'Expand database alpha', exact: true }).click();
  expect((await calls(page, 'schema_tables')).map((c) => [c.database, c.schema])).toEqual([
    ['alpha', 'public'],
    ['beta', 'public'],
    ['beta', 'audit'],
  ]);
  await page.getByRole('button', { name: 'Refresh database alpha', exact: true }).click();
  expect((await calls(page, 'schema_tables')).at(-1)).toMatchObject({
    database: 'alpha',
    schema: 'public',
  });
  await page.screenshot({ path: info.outputPath('postgres-database-tree.png') });
  await page.getByLabel('Appearance', { exact: true }).selectOption('dark');
  await page.screenshot({ path: info.outputPath('postgres-database-tree-dark.png') });
  await page.getByRole('button', { name: 'Disconnect current connection', exact: true }).click();
  await page.getByRole('button', { name: 'Reconnect', exact: true }).click();
  await expect(
    page.getByRole('tab', { name: 'items · beta · audit', exact: true }),
  ).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('button', { name: 'Edit row 1 label', exact: true })).toHaveText(
    'beta/audit',
  );
  const layout = await page.evaluate(() => (window as unknown as { pgLayout: unknown }).pgLayout);
  expect(JSON.stringify(layout)).not.toContain('alpha draft');
  expect(JSON.stringify(layout)).toContain('"database":"beta"');
  expect((await calls(page, 'browse_table')).at(-1).request).toMatchObject({
    database: 'beta',
    schema: 'audit',
  });
});

test('PostgreSQL errors retry, empty databases and unusual names work, refresh removes stale names', async ({
  page,
}) => {
  await setup(page);
  await page.evaluate(() => {
    (window as any).failDatabase = 'alpha';
  });
  await page.getByRole('button', { name: 'Expand database alpha', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('temporarily unavailable');
  await page.evaluate(() => {
    (window as any).failDatabase = '';
  });
  await page.getByRole('button', { name: 'Expand database alpha', exact: true }).click();
  await expect(table(page, 'alpha')).toBeVisible();
  await page.evaluate(() => {
    (window as any).failSchema = 'audit';
  });
  await page.getByRole('button', { name: 'Expand schema alpha.audit', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Schema access unavailable');
  await page.evaluate(() => {
    (window as any).failSchema = '';
  });
  await page.getByRole('button', { name: 'Expand schema alpha.audit', exact: true }).click();
  await expect(table(page, 'alpha', 'audit')).toBeVisible();
  await page.getByRole('button', { name: 'Expand database empty', exact: true }).click();
  await expect(page.getByText('No accessible schemas')).toBeVisible();
  await page.getByRole('button', { name: 'Expand database __proto__', exact: true }).click();
  await expect(table(page, '__proto__')).toBeVisible();
  await table(page, '__proto__').click();
  expect((await calls(page, 'browse_table')).at(-1).request.database).toBe('__proto__');
  await page.getByLabel('Search databases and loaded tables').fill('alpha');
  await expect(
    page.getByRole('button', { name: 'Collapse database empty', exact: true }),
  ).toHaveCount(0);
  await page.getByLabel('Search databases and loaded tables').clear();
  await page.evaluate(() => {
    (window as any).pgNames = ['beta', 'empty', '__proto__'];
  });
  await page.getByRole('button', { name: 'Refresh databases', exact: true }).click();
  await expect(page.getByRole('button', { name: /database alpha$/, exact: false })).toHaveCount(0);
  // A database returning to the list must reload its catalog, not reuse stale tables.
  await page.evaluate(() => {
    (window as any).pgNames = ['alpha', 'beta', 'empty', '__proto__'];
  });
  await page.getByRole('button', { name: 'Refresh databases', exact: true }).click();
  const before = (await calls(page, 'schema_tables')).length;
  await page.getByRole('button', { name: 'Expand database alpha', exact: true }).click();
  expect(await calls(page, 'schema_tables')).toHaveLength(before + 1);
});
