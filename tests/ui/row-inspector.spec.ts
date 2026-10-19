import { expect, test, type Page } from '@playwright/test';

test('first click selects any cell; only a second activation in that row edits', async ({
  page,
}) => {
  await page.goto('/');
  const customer = page.getByRole('button', { name: 'Edit row 1 customer', exact: true });
  await customer.click();
  await expect(page.locator('tr.row-selected')).toHaveCount(1);
  await expect(page.getByRole('textbox', { name: /^Cell value/ })).toHaveCount(0);
  await page.keyboard.type('accidental typing');
  await expect(customer).toHaveText('Mali S.');
  await expect(page.getByRole('button', { name: 'Save changes', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Edit row 1 status', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Cell value row 1 status' })).toBeFocused();
  await page.keyboard.press('Escape');

  // Switching rows arms the row, even if an editor was active before.
  await customer.click();
  const second = page.getByRole('button', { name: 'Edit row 2 customer', exact: true });
  await second.click();
  await expect(page.getByRole('textbox', { name: /^Cell value/ })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Select row 2', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await second.press('Enter');
  await expect(page.getByRole('textbox', { name: 'Cell value row 2 customer' })).toBeFocused();
  await page.keyboard.press('Escape');

  // Immutable cells also select; keyboard activation follows the same two-step rule.
  await page.getByRole('button', { name: 'Select row 3 id', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Row 3 values', exact: true })).toBeVisible();
  await expect(page.getByRole('textbox', { name: /^Cell value/ })).toHaveCount(0);
  const fourth = page.getByRole('button', { name: 'Edit row 4 customer', exact: true });
  await fourth.focus();
  await fourth.press('Enter');
  await expect(page.getByRole('textbox', { name: /^Cell value/ })).toHaveCount(0);
  await fourth.press('Enter');
  await expect(page.getByRole('textbox', { name: 'Cell value row 4 customer' })).toBeFocused();
});

test('selection never masks editing or deletion highlights in either theme', async ({
  page,
}, testInfo) => {
  await page.goto('/');
  const cell = page.getByRole('button', { name: 'Edit row 1 customer', exact: true });
  await cell.click();
  for (const [theme, selected, edited, deleted] of [
    ['light', 'rgb(227, 238, 251)', 'rgb(255, 242, 198)', 'rgb(255, 226, 227)'],
    ['dark', 'rgb(38, 61, 86)', 'rgb(74, 60, 32)', 'rgb(81, 45, 52)'],
  ]) {
    await page.getByLabel('Appearance', { exact: true }).selectOption(theme);
    await expect(page.locator('tr.row-selected td').first()).toHaveCSS(
      'background-color',
      selected,
    );
    await cell.click();
    const input = page.getByRole('textbox', { name: 'Cell value row 1 customer' });
    await expect(input.locator('xpath=ancestor::td')).toHaveCSS('background-color', edited);
    await input.fill('Draft customer');
    await input.press('Enter');
    await cell.hover();
    await expect(cell.locator('..')).toHaveCSS('background-color', edited);
    await expect(page.locator('.inspector-edited')).toContainText('Draft customer');
    // Delete from a focused selected cell, without opening an editor.
    await cell.focus();
    await cell.press('Backspace');
    await expect(page.locator('tr.pending-delete')).toHaveCount(1);
    await page.getByRole('button', { name: 'Select row 1 customer', exact: true }).hover();
    for (const td of await page.locator('tr.pending-delete td').all())
      await expect(td).toHaveCSS('background-color', deleted);
    await expect(page.getByRole('complementary', { name: 'Row details' })).toContainText(
      'Pending deletion',
    );
    await page.screenshot({ path: testInfo.outputPath(`row-details-${theme}.png`) });
    await page.getByRole('button', { name: 'Restore deleted rows' }).click();
    await expect(cell.locator('..')).toHaveCSS('background-color', edited);
  }
});

test('row details are compact, resizable, closeable and preferences persist without row data', async ({
  page,
}, testInfo) => {
  await page.goto('/');
  const toggle = page.getByRole('checkbox', { name: 'Row details', exact: true });
  const panel = page.getByRole('complementary', { name: 'Row details' });
  await expect(toggle).toBeChecked();
  await expect(panel).toHaveCount(0);
  await page.getByRole('button', { name: 'Edit row 1 customer', exact: true }).click();
  const areaBox = (await page.locator('.result-area').boundingBox())!;
  const panelBox = (await panel.boundingBox())!;
  expect(panelBox.width / areaBox.width).toBeLessThanOrEqual(0.101);
  expect(panelBox.x + panelBox.width).toBeCloseTo(areaBox.x + areaBox.width, 0);
  await expect(panel).toContainText('Mali S.');
  await expect(panel.getByRole('textbox')).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath('row-details-compact.png') });
  await page.getByRole('button', { name: 'Close row details' }).click();
  await expect(panel).toHaveCount(0);
  await expect(toggle).toBeChecked();
  await expect(page.locator('tr.row-selected')).toHaveCount(1);
  await page.getByRole('button', { name: 'Edit row 2 customer', exact: true }).click();
  await expect(panel).toContainText('Arun K.');
  const edge = page.getByRole('separator', { name: 'Resize row details' });
  const edgeBox = (await edge.boundingBox())!;
  await page.mouse.move(edgeBox.x + edgeBox.width / 2, edgeBox.y + 100);
  await page.mouse.down();
  await page.mouse.move(edgeBox.x - 240, edgeBox.y + 100, { steps: 8 });
  await page.mouse.up();
  expect((await panel.boundingBox())!.width).toBeGreaterThan(panelBox.width + 200);
  await edge.press('ArrowLeft');
  const enlarged = (await panel.boundingBox())!.width;
  await toggle.uncheck();
  await expect(panel).toHaveCount(0);
  await page.reload();
  await expect(toggle).not.toBeChecked();
  await page.getByRole('button', { name: 'Edit row 1 customer', exact: true }).click();
  await expect(panel).toHaveCount(0);
  await expect(page.locator('tr.row-selected')).toHaveCount(1);
  await toggle.check();
  await expect(panel).toBeVisible();
  expect((await panel.boundingBox())!.width).toBeCloseTo(enlarged, 0);
  const stored = await page.evaluate(() => localStorage.getItem('buggy-table.row-inspector.v1'));
  expect(Object.keys(JSON.parse(stored!)).sort()).toEqual(['enabled', 'widthPercent']);
  expect(stored).not.toMatch(/Mali|Arun|customer|rows/);
  await page.getByRole('button', { name: 'Hide table controls' }).click();
  await expect(panel).toBeVisible();
  await edge.press('Home');
  await expect(edge).toHaveAttribute('aria-valuenow', '10');
  // A refreshed result must not retain an inspector on a possibly different record.
  await page.keyboard.press('Control+r');
  await expect(panel).toHaveCount(0);
  await expect(page.locator('tr.row-selected')).toHaveCount(0);
});

test('multi-selection is preserved with modifiers and inspector scrolling never deletes rows', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Select row 1 id', exact: true }).click();
  await page
    .getByRole('button', { name: 'Edit row 3 customer', exact: true })
    .click({ modifiers: ['Shift'] });
  await expect(page.locator('tr.row-selected')).toHaveCount(3);
  await expect(page.getByRole('textbox', { name: /^Cell value/ })).toHaveCount(0);
  const region = page.getByRole('region', { name: 'Row 3 values', exact: true });
  await region.focus();
  await region.press('Backspace');
  await expect(page.locator('tr.pending-delete')).toHaveCount(0);
  await page
    .getByRole('button', { name: 'Edit row 2 customer', exact: true })
    .click({ modifiers: ['ControlOrMeta'] });
  await expect(page.locator('tr.row-selected')).toHaveCount(2);
  await expect(page.getByRole('region', { name: 'Row 3 values', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Delete 2 selected', exact: true }).click();
  await expect(page.locator('tr.pending-delete')).toHaveCount(2);
});

async function typedRows(page: Page) {
  await page.addInitScript(() => {
    const target = window as unknown as Record<string, unknown>;
    target.isTauri = true;
    target.inspectorCalls = [];
    const profile = {
      id: 'typed',
      name: 'Typed fixture',
      engine: 'postgres',
      host: '127.0.0.1',
      port: 5432,
      database: 'fixture',
      username: 'reader',
      schema: 'public',
      tls: 'localOnly',
      rootCaPem: '',
      manualEdits: true,
      group: '',
      pinned: true,
      lastUsed: 1,
      hasPassword: true,
      layout: { selectedSchema: 'public', tabs: [], activeTab: 'empty' },
    };
    const columns = [
      ['id', 'int8'],
      ['payload', 'jsonb'],
      ['amount', 'numeric'],
      ['created_at', 'timestamptz'],
      ['labels', 'text[]'],
      ['note', 'text'],
      ['empty', 'text'],
      ['html', 'text'],
    ].map(([name, dataType]) => ({
      name,
      dataType,
      nullable: true,
      primaryKey: name === 'id',
      editable: name !== 'id',
    }));
    const rows = [
      [
        '9007199254740993',
        '{"big":900719925474099312345,"decimal":1.23000,"nested":{"items":[' +
          Array.from(
            { length: 100 },
            (_, i) => '{"index":' + i + ',"text":"long value ' + i + '"}',
          ).join(',') +
          ']},"last":"FINAL JSON VALUE"}',
        '99999999999999999999.000100',
        '2026-10-19 23:59:01.123456+07:00',
        '{"a,b",NULL,"null"}',
        null,
        '',
        '<script>window.inspectorUnsafe=true</script>',
      ],
      ['9007199254740994', '{"second":true}', '0.00000', 'infinity', '{}', 'null', '', 'safe'],
    ];
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string) => {
        (target.inspectorCalls as string[]).push(command);
        if (command.startsWith('plugin:')) return 1;
        if (command === 'check_update') return { configured: false, currentVersion: '0.1.7' };
        if (command === 'list_connections') return [profile];
        if (command === 'reconnect_profile')
          return {
            profile,
            warning: null,
            connection: {
              ...profile,
              id: 'session-typed',
              serverVersion: '180006',
              tables: [
                {
                  schema: 'public',
                  name: 'typed_rows',
                  estimatedRows: 2,
                  deletable: true,
                  columns,
                },
              ],
            },
          };
        if (command === 'list_schemas') return ['public'];
        if (command === 'browse_table')
          return {
            columns: columns.map((c) => c.name),
            rows,
            sql: 'SELECT …',
            elapsedMs: 1,
            truncated: false,
          };
        return null;
      },
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Reconnect Typed fixture', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Select row 1 id', exact: true })).toBeVisible();
}

test('typed inspector preserves complete JSON and database precision without extra queries', async ({
  page,
}, testInfo) => {
  await typedRows(page);
  const readCalls = () =>
    page.evaluate(() =>
      (window as unknown as { inspectorCalls: string[] }).inspectorCalls.filter((c) =>
        ['browse_table', 'run_query', 'count_table'].includes(c),
      ),
    );
  const before = await readCalls();
  await page.getByRole('button', { name: 'Select row 1 id', exact: true }).click();
  const panel = page.getByRole('complementary', { name: 'Row details' });
  const field = (name: string) =>
    panel
      .locator('.inspector-field')
      .filter({ has: page.locator('.inspector-name', { hasText: new RegExp(`^${name}$`) }) })
      .locator('pre');
  expect(await field('payload').textContent()).toContain(
    '\n  "big": 900719925474099312345,\n  "decimal": 1.23000,',
  );
  expect(await field('payload').textContent()).toContain('FINAL JSON VALUE');
  await expect(field('amount')).toHaveText('99999999999999999999.000100');
  await expect(field('created_at')).toHaveText('2026-10-19 23:59:01.123456+07:00');
  await expect(field('labels')).toHaveText('{"a,b",NULL,"null"}');
  await expect(field('note')).toHaveText('NULL');
  await expect(field('note')).toHaveClass('null-value');
  await expect(field('empty')).toHaveText('""');
  await expect(field('html')).toHaveText('<script>window.inspectorUnsafe=true</script>');
  expect(
    await page.evaluate(() => (window as unknown as { inspectorUnsafe?: boolean }).inspectorUnsafe),
  ).toBeUndefined();
  await page.getByRole('separator', { name: 'Resize row details' }).press('End');
  const scroll = panel.locator('.inspector-scroll');
  await scroll.evaluate((e) => {
    e.scrollTop = e.scrollHeight;
  });
  await expect(field('html')).toBeInViewport();
  await page.screenshot({ path: testInfo.outputPath('typed-row-expanded-bottom.png') });
  await page.getByRole('button', { name: 'Select row 2 id', exact: true }).click();
  await expect.poll(() => scroll.evaluate((e) => e.scrollTop)).toBe(0);
  await expect(field('payload')).toHaveText('{\n  "second": true\n}');
  await expect(field('created_at')).toHaveText('infinity');
  await expect(field('note')).toHaveText('null');
  await expect(field('note')).not.toHaveClass('null-value');
  await page.getByRole('button', { name: 'Select row 1 id', exact: true }).click();
  await page.screenshot({ path: testInfo.outputPath('typed-row-expanded-json.png') });
  // Invalid drafts remain visible as typed; inspector never modifies them.
  await page.getByRole('button', { name: 'Edit row 1 payload', exact: true }).click();
  await page.getByRole('textbox', { name: 'Cell value row 1 payload' }).fill('{"draft":');
  await page.keyboard.press('Enter');
  await expect(field('payload')).toHaveText('{"draft":');
  expect(await readCalls()).toEqual(before);
  expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toMatch(
    /FINAL JSON|99999999|draft|created_at/,
  );
});

test('blocked preference storage keeps the grid usable and reports that settings were not saved', async ({
  page,
}) => {
  await page.addInitScript(() => {
    Storage.prototype.setItem = () => {
      throw new Error('Storage blocked');
    };
  });
  await page.goto('/');
  await page.getByRole('checkbox', { name: 'Row details', exact: true }).uncheck();
  await expect(page.getByRole('alert')).toContainText('local preferences could not be saved');
  await page.getByRole('button', { name: 'Edit row 1 customer', exact: true }).click();
  await expect(page.locator('tr.row-selected')).toHaveCount(1);
  await expect(page.getByRole('complementary', { name: 'Row details' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Edit row 1 customer', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Cell value row 1 customer' })).toBeFocused();
});
