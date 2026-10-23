import { editCell } from './grid-helpers';
import { expect, test } from '@playwright/test';

test('sample browsing, filters and pagination work without external requests', async ({ page }) => {
  const unexpected: string[] = [];
  page.on('request', (request) => {
    if (!new URL(request.url()).hostname.match(/^(127\.0\.0\.1|localhost)$/))
      unexpected.push(request.url());
  });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'orders TABLE' })).toBeVisible();
  await expect(page.getByText('Sample mode · offline')).toBeVisible();
  await page.getByRole('button', { name: 'Next page', exact: true }).click();
  await expect(page.getByText('Page 2', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Filters', exact: true }).click();
  await page.getByLabel('Filter 1 column').selectOption('status');
  await page.getByLabel('Filter 1 value').fill('completed');
  await page.getByRole('button', { name: 'Apply filters' }).click();
  await expect(page.getByText('100 rows shown')).toBeVisible();
  await expect(page.getByText('Page 1', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Clear filter', exact: true }).click();
  await expect(page.getByText('100 rows shown')).toBeVisible();
  const rendered = await page
    .getByRole('table', { name: 'Query results' })
    .locator('tbody tr')
    .count();
  expect(rendered).toBeGreaterThan(15);
  expect(rendered).toBeLessThan(50);
  expect(unexpected).toEqual([]);
});

test('SQL sample and unavailable AI do not pretend to execute', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('tab', { name: 'SQL editor' }).click();
  await page.getByRole('button', { name: 'Run query', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Connect your PostgreSQL database');
  await page.getByRole('button', { name: 'AI assistant NEXT' }).click();
  await expect(page.getByText('Planned · not enabled')).toBeVisible();
});

test('connection dialog traps focus, hides passwords and closes with Escape', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect database', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.getByLabel('Password', { exact: true })).toHaveAttribute('type', 'password');
  await page.getByLabel('Default database (optional)', { exact: true }).fill('test');
  await page.getByLabel('Username', { exact: true }).fill('reader');
  await page.getByLabel('Password', { exact: true }).fill('CANARY_SECRET');
  await page.getByRole('button', { name: 'Connect securely' }).click();
  await expect(page.getByRole('alert')).toContainText('desktop app');
  await expect(page.getByLabel('Password', { exact: true })).toHaveValue('');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  expect(
    await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length })),
  ).toEqual({ local: 0, session: 0 });
});

test('preview looks correct and does not overflow', async ({ page }, testInfo) => {
  await page.goto('/');
  await expect(page.getByRole('table', { name: 'Query results' })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('workspace.png'), fullPage: true });
});

test('table tabs retain edits, filters and pages; Save persists and Discard restores', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Next page', exact: true }).click();
  await editCell(page, 1, 'customer');
  await page.getByRole('textbox', { name: /^Cell value row/ }).fill('Edited locally');
  await page.keyboard.press('Enter');
  await expect(page.getByText('1 row with unsaved changes')).toBeVisible();
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'customers', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await page.getByRole('tab', { name: 'orders' }).click();
  await expect(page.getByText('Page 2', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'Edited locally',
  );
  await page.getByRole('button', { name: 'Refresh table', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Save or discard');
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.getByText('1 row saved in the local sample only.')).toBeVisible();
  await page.getByRole('button', { name: 'Refresh table', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'Edited locally',
  );
  await editCell(page, 1, 'customer');
  await page.getByRole('textbox', { name: /^Cell value row/ }).fill('Discard me');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'Discard changes', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'Edited locally',
  );
  await page.getByRole('button', { name: 'Filters', exact: true }).click();
  await page.getByLabel('Filter 1 column').selectOption('customer');
  await page.getByLabel('Filter 1 value').fill('Edited locally');
  await page.getByRole('button', { name: 'Apply filters' }).click();
  await page.getByRole('tab', { name: 'customers', exact: true }).click();
  await page.getByRole('tab', { name: 'orders', exact: true }).click();
  await expect(page.getByLabel('Filter 1 value')).toHaveValue('Edited locally');
  await expect(page.getByText('1 rows shown')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Edit row 1 id', exact: true })).toHaveCount(0);
});

test('NULL edits and closing dirty tabs require an explicit choice', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await editCell(page, 1, 'email');
  await page.getByRole('button', { name: 'Set row 1 email to NULL' }).click();
  await page.getByRole('tab', { name: 'SQL editor', exact: true }).click();
  await page.getByRole('tab', { name: 'Data', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 email', exact: true })).toHaveText(
    'NULL',
  );
  await page.getByRole('button', { name: 'Close customers tab' }).click();
  await expect(page.getByRole('dialog')).toContainText('Unsaved changes in customers');
  await page.getByRole('button', { name: 'Keep editing' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByRole('button', { name: 'Close customers tab' }).click();
  await page.getByRole('button', { name: 'Save & close' }).click();
  await expect(page.getByRole('tab', { name: 'customers', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 email', exact: true })).toHaveText(
    'NULL',
  );
  await editCell(page, 1, 'name');
  await page.getByRole('textbox', { name: /^Cell value row/ }).fill('Do not save');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'Close customers tab' }).click();
  await page.getByRole('button', { name: 'Discard & close' }).click();
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 name', exact: true })).toHaveText(
    'Mali S.',
  );
});

test('table scroll survives tab switches and sidebar controls fit a small window', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1000, height: 680 });
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Privacy & access' })).toBeInViewport();
  const grid = page.locator('.grid-scroll');
  await grid.evaluate((element) => {
    element.scrollTop = 480;
    element.dispatchEvent(new Event('scroll'));
  });
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await page.getByRole('tab', { name: 'orders', exact: true }).click();
  await expect.poll(() => grid.evaluate((element) => element.scrollTop)).toBe(480);
});

test('editing happens inside the cell, Escape reverts, and save/refresh shortcuts keep the app open', async ({
  page,
}) => {
  await page.goto('/');
  const cell = page.getByRole('button', { name: 'Edit row 1 customer', exact: true });
  await editCell(page, 1, 'customer');
  const input = page.getByRole('textbox', { name: 'Cell value row 1 customer', exact: true });
  await expect(page.getByRole('dialog')).toHaveCount(0);
  expect(await input.evaluate((node) => !!node.closest('td'))).toBe(true);
  await input.fill('Revert this');
  await input.press('Escape');
  await expect(cell).toHaveText('Mali S.');
  await expect(page.getByRole('button', { name: 'Save changes', exact: true })).toHaveCount(0);
  await editCell(page, 1, 'customer');
  await input.fill('Inline saved');
  await input.press('Control+s');
  await expect(page.getByText('1 row saved in the local sample only.')).toBeVisible();
  await expect(cell).toHaveText('Inline saved');
  await page.evaluate(() => {
    document.body.dataset.refreshCanary = 'same-document';
  });
  for (const shortcut of ['Control+r', 'Meta+r', 'F5']) {
    await page.keyboard.press(shortcut);
    await expect(cell).toHaveText('Inline saved');
    expect(await page.evaluate(() => document.body.dataset.refreshCanary)).toBe('same-document');
  }
  await editCell(page, 1, 'customer');
  await input.fill('Unsaved inline');
  await input.press('F5');
  await expect(page.getByRole('alert')).toContainText('Save or discard');
  await expect(input).toHaveValue('Unsaved inline');
});

test('column resizing is draggable, keyboard accessible and retained by each table tab', async ({
  page,
}) => {
  await page.goto('/');
  const handle = page.getByRole('slider', { name: 'Resize customer column', exact: true });
  const box = (await handle.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 70, box.y + box.height / 2, { steps: 6 });
  await page.mouse.up();
  await expect(handle).toHaveAttribute('aria-valuenow', '240');
  await expect(page.getByRole('columnheader', { name: /Sort by customer/ })).toHaveAttribute(
    'aria-sort',
    'none',
  );
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await page.getByRole('tab', { name: 'orders', exact: true }).click();
  await expect(handle).toHaveAttribute('aria-valuenow', '240');
  await handle.focus();
  await page.keyboard.press('ArrowLeft');
  await expect(handle).toHaveAttribute('aria-valuenow', '230');
  await handle.dblclick();
  await expect(handle).toHaveAttribute('aria-valuenow', '170');
});

test('header sorting covers the whole dataset before paging and respects pending edits', async ({
  page,
}) => {
  await page.goto('/');
  const header = page.getByRole('button', { name: 'Sort by total', exact: true });
  await header.click();
  await header.click();
  await expect(page.getByRole('columnheader', { name: /Sort by total/ })).toHaveAttribute(
    'aria-sort',
    'descending',
  );
  await expect(page.getByRole('button', { name: 'Edit row 1 total', exact: true })).toHaveText(
    '6187.50',
  );
  await page.getByRole('button', { name: 'Next page', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 total', exact: true })).toHaveText(
    '4812.50',
  );
  await editCell(page, 1, 'customer');
  await page.getByRole('textbox', { name: /^Cell value row/ }).fill('Pending');
  await header.click();
  await expect(page.getByRole('alert')).toContainText('Save or discard');
  await expect(page.getByText('Page 2', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Discard changes', exact: true }).click();
  await header.click();
  await expect(page.getByText('Page 1', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Edit row 1 total', exact: true })).toHaveText(
    '13.75',
  );
});

test('multiple filters support AND, OR, removal and tab persistence', async ({
  page,
}, testInfo) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Filters', exact: true }).click();
  await page.getByLabel('Filter 1 column').selectOption('id');
  await page.getByLabel('Filter 1 operator').selectOption('greaterThan');
  await page.getByLabel('Filter 1 value').fill('440');
  await page.getByRole('button', { name: 'Add filter', exact: true }).click();
  await page.getByLabel('Filter 2 column').selectOption('status');
  await page.getByLabel('Filter 2 value').fill('completed');
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await expect(page.getByText('3 rows shown')).toBeVisible();
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await page.getByRole('tab', { name: 'orders', exact: true }).click();
  await expect(page.getByLabel('Filter 2 value')).toHaveValue('completed');
  await page.getByLabel('Match filters').selectOption('any');
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await expect(page.getByText('100 rows shown')).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('compact-filters.png') });
  await page.getByRole('button', { name: 'Remove filter 2', exact: true }).click();
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await expect(page.getByText('10 rows shown')).toBeVisible();
});

test('engine selection sets defaults and restricts manual editing to PostgreSQL', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect database', exact: true }).click();
  await page.getByLabel('Password', { exact: true }).fill('DO_NOT_RETAIN');
  await page.getByLabel('Database engine').selectOption('mysql');
  await expect(page.getByLabel('Port', { exact: true })).toHaveValue('3306');
  await expect(page.getByLabel('Password', { exact: true })).toHaveValue('');
  await expect(page.getByLabel('Schema', { exact: true })).toHaveCount(0);
  await expect(page.getByLabel('Allow manual cell edits')).toHaveCount(0);
  await expect(page.getByText(/including administrator accounts/)).toBeVisible();
  await expect(page.getByLabel('Default database (optional)')).not.toHaveAttribute('required');
  await page.getByLabel('Database engine').selectOption('redis');
  await expect(page.getByLabel('Port', { exact: true })).toHaveValue('6379');
  await expect(page.getByLabel('Database number', { exact: true })).toHaveValue('0');
  await expect(page.getByLabel('Username', { exact: true })).not.toHaveAttribute('required');
  await page.getByLabel('Database engine').selectOption('postgres');
  await expect(page.getByLabel('Port', { exact: true })).toHaveValue('5432');
  await expect(page.getByLabel('Schema', { exact: true })).toHaveValue('public');
  await expect(page.getByLabel('Allow manual cell edits')).toBeVisible();
});

test('Redis UI scans, inspects TTL, displays values as text, and refreshes without navigation', async ({
  page,
}, testInfo) => {
  await page.addInitScript(() => {
    // IPC contract stub for browser UI coverage. Rust integration tests use real Redis.
    const target = window as unknown as Record<string, unknown>;
    target.isTauri = true;
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (cmd: string, payload: { request?: Record<string, unknown> }) => {
        if (cmd.startsWith('plugin:')) return 1;
        if (cmd === 'connect_profile')
          return {
            profile: null,
            warning: null,
            connection: {
              id: 'redis-fixture',
              engine: 'redis',
              name: 'Redis UI fixture',
              host: '127.0.0.1',
              port: 16379,
              database: '0',
              schema: '0',
              username: 'reader',
              tls: 'localOnly',
              manualEdits: false,
              serverVersion: 'Redis',
              tables: [],
            },
          };
        if (cmd === 'redis_command') {
          const args = payload.request?.args as string[];
          if (args[0] === 'SCAN') return { value: ['0', ['test:key']], elapsedMs: 1 };
          if (args[0] === 'TYPE') return { value: 'string', elapsedMs: 1 };
          if (args[0] === 'TTL') return { value: '90', elapsedMs: 1 };
          if (args[0] === 'GET')
            return { value: { preview: '<script>window.INJECTED=true</script>' }, elapsedMs: 1 };
          if (args[0] === 'PING') return { value: 'PONG', elapsedMs: 1 };
          if (args[0] === 'INFO')
            return {
              value:
                args[1] === 'server'
                  ? '# Server\r\nuptime_in_seconds:86400\r\nhz:10\r\nrun_id:fixture'
                  : '# Memory\r\nused_memory:1048576\r\nused_memory_rss:2097152\r\nused_memory_peak:3145728\r\nmem_fragmentation_ratio:2',
              elapsedMs: 1,
            };
          throw { code: 'POLICY_DENIED', message: 'Only bounded reads are allowed.' };
        }
        return null;
      },
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect database', exact: true }).click();
  await page.getByLabel('Database engine').selectOption('redis');
  await page.getByRole('button', { name: 'Connect securely' }).click();
  await expect(page.getByRole('region', { name: 'Redis workspace' })).toBeVisible();
  await page.getByRole('button', { name: 'test:key', exact: true }).click();
  await expect(page.getByText('string · TTL: 90 seconds')).toBeVisible();
  await expect(page.getByLabel('Redis value')).toContainText('<script>');
  expect(
    await page.evaluate(() => (window as unknown as Record<string, unknown>).INJECTED),
  ).toBeUndefined();
  await page.getByRole('button', { name: 'Run command', exact: true }).click();
  await expect(page.getByLabel('Redis value')).toContainText('PONG');
  await page.getByLabel('Redis command', { exact: true }).fill('["FLUSHALL"]');
  await page.getByRole('button', { name: 'Run command', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Only bounded reads');
  await page.keyboard.press('Control+r');
  await expect(page.getByRole('button', { name: 'test:key', exact: true })).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(
    await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length })),
  ).toEqual({ local: 0, session: 0 });
  await page.screenshot({ path: testInfo.outputPath('redis-workspace.png'), fullPage: true });
  await page.getByRole('button', { name: 'Database overview', exact: true }).click();
  await expect(page.getByRole('img', { name: /Server uptime graph/ })).toBeVisible();
  await page.getByRole('button', { name: 'memory', exact: true }).click();
  await expect(page.getByRole('img', { name: /Memory used graph/ })).toBeVisible();
});
