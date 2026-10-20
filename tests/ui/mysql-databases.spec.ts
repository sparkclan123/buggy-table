import { expect, test, type Page } from '@playwright/test';

async function setup(page: Page) {
  await page.addInitScript(() => {
    const w = window as unknown as Record<string, unknown>;
    w.isTauri = true;
    w.mysqlCalls = [];
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, args: Record<string, any> = {}) => {
        if (command.startsWith('plugin:')) return 1;
        (w.mysqlCalls as unknown[]).push({ command, args });
        if (command === 'list_connections') return [];
        if (command === 'connect_profile')
          return {
            profile: null,
            warning: null,
            connection: {
              ...args.request,
              id: 'mysql-server',
              name: 'MySQL fixture',
              serverVersion: '8.4',
              tables: [],
              database: '',
              schema: '',
              manualEdits: false,
            },
          };
        if (command === 'list_schemas') return ['alpha', 'beta', 'empty'];
        if (command === 'schema_tables') {
          if (w.failCatalog) throw { message: 'Catalog access temporarily unavailable.' };
          if (args.schema === 'empty') return [];
          return [
            {
              schema: args.schema,
              name: 'items',
              estimatedRows: 1,
              columns: [
                { name: 'id', dataType: 'int', primaryKey: true, editable: false, nullable: false },
                {
                  name: 'label',
                  dataType: 'varchar',
                  primaryKey: false,
                  editable: false,
                  nullable: true,
                },
              ],
            },
          ];
        }
        if (command === 'browse_table')
          return {
            columns: ['id', 'label'],
            rows: [[args.request.schema === 'alpha' ? '1' : '99', args.request.schema]],
            elapsedMs: 2,
            truncated: false,
            sql: 'SELECT …',
          };
        if (command === 'count_table_rows') return { rows: '1', elapsedMs: 1 };
        if (command === 'inspect_table') return [];
        return null;
      },
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect database', exact: true }).click();
  await page.getByLabel('Database engine').selectOption('mysql');
  await page.getByLabel('Username', { exact: true }).fill('fixture_admin');
  await expect(page.getByLabel('Default database (optional)')).toHaveValue('');
  await page.getByRole('button', { name: 'Connect securely', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
}

async function calls(page: Page, command: string) {
  return page.evaluate(
    (command) =>
      (
        window as unknown as { mysqlCalls: { command: string; args: Record<string, any> }[] }
      ).mysqlCalls
        .filter((call) => call.command === command)
        .map((call) => call.args),
    command,
  );
}

test('MySQL connects without a default database and lazily expands independent database tabs', async ({
  page,
}, info) => {
  await setup(page);
  expect((await calls(page, 'connect_profile'))[0].request.database).toBe('');
  expect(await calls(page, 'schema_tables')).toEqual([]);
  const alpha = page.getByRole('button', { name: 'Expand database alpha', exact: true });
  await alpha.focus();
  await alpha.press('Enter');
  await expect(
    page.getByRole('button', { name: 'Collapse database alpha', exact: true }),
  ).toHaveAttribute('aria-expanded', 'true');
  await page
    .getByLabel('Tables in alpha')
    .getByRole('button', { name: 'items', exact: true })
    .click();
  await expect(page.getByText('1 rows shown')).toBeVisible();
  await page.getByRole('button', { name: 'Expand database beta', exact: true }).click();
  await page
    .getByLabel('Tables in beta')
    .getByRole('button', { name: 'items', exact: true })
    .click();
  expect((await calls(page, 'browse_table')).map((c) => c.request.schema)).toEqual([
    'alpha',
    'beta',
  ]);
  await page.getByRole('button', { name: 'Count exact', exact: true }).click();
  expect((await calls(page, 'count_table_rows'))[0].request.schema).toBe('beta');
  await page.getByRole('button', { name: 'Collapse database alpha', exact: true }).click();
  await page.getByRole('button', { name: 'Expand database alpha', exact: true }).click();
  expect((await calls(page, 'schema_tables')).map((c) => c.schema)).toEqual(['alpha', 'beta']);
  await page
    .getByLabel('Tables in alpha')
    .getByRole('button', { name: 'items', exact: true })
    .click();
  expect(await calls(page, 'browse_table')).toHaveLength(2);
  await page.getByRole('button', { name: 'Refresh database alpha', exact: true }).click();
  expect((await calls(page, 'schema_tables')).map((c) => c.schema)).toEqual([
    'alpha',
    'beta',
    'alpha',
  ]);
  await page.getByRole('button', { name: 'Refresh databases', exact: true }).click();
  expect(await calls(page, 'list_schemas')).toHaveLength(2);
  await page.screenshot({ path: info.outputPath('mysql-database-tree.png') });
  await page.getByLabel('Appearance', { exact: true }).selectOption('dark');
  await page.screenshot({ path: info.outputPath('mysql-database-tree-dark.png') });
});

test('MySQL catalog errors can retry and empty databases remain usable', async ({ page }) => {
  await setup(page);
  await page.evaluate(() => {
    (window as unknown as Record<string, unknown>).failCatalog = true;
  });
  await page.getByRole('button', { name: 'Expand database alpha', exact: true }).click();
  await expect(page.getByText('Catalog access temporarily unavailable.')).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Expand database alpha', exact: true }),
  ).toBeEnabled();
  await page.evaluate(() => {
    (window as unknown as Record<string, unknown>).failCatalog = false;
  });
  await page.getByRole('button', { name: 'Expand database alpha', exact: true }).click();
  await expect(
    page.getByLabel('Tables in alpha').getByRole('button', { name: 'items' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Expand database empty', exact: true }).click();
  await expect(page.getByText('No accessible tables')).toBeVisible();
  await page.getByLabel('Search databases and loaded tables').fill('alpha');
  await expect(
    page.getByRole('button', { name: 'Collapse database alpha', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Collapse database empty', exact: true }),
  ).toHaveCount(0);
});
