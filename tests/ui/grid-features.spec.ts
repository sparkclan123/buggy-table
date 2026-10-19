import { expect, test } from '@playwright/test';

test('page sizes, optional counts, collapsed controls and virtual rows', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByLabel('Rows per page')).toHaveValue('50');
  await expect(page.getByText('450 total rows')).toBeVisible();
  for (const size of [100, 250, 500]) {
    await page.getByLabel('Rows per page').selectOption(String(size));
    await expect(page.getByText(`${Math.min(size, 450)} rows shown`)).toBeVisible();
    expect(await page.locator('.data-table tbody tr').count()).toBeLessThan(50);
  }
  await page.getByRole('button', { name: 'Hide table controls' }).click();
  await expect(page.getByRole('tab', { name: 'Structure', exact: true })).toBeHidden();
  await expect(page.getByRole('table', { name: 'Query results' })).toBeVisible();
  await page.getByRole('button', { name: 'Show table controls' }).click();
  await page.getByRole('button', { name: 'Filters', exact: true }).click();
  await page.getByLabel('Filter 1 column').selectOption('status');
  await page.getByLabel('Filter 1 value').fill('completed');
  await expect(page.getByLabel('Filter 1 operator').locator('option:checked')).toHaveText('=');
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await page.getByRole('button', { name: 'Count exact', exact: true }).click();
  await expect(page.getByText('150 matching rows')).toBeVisible();
});

test('edited cells and staged deletions survive until explicit save, including Cmd+W guard', async ({
  page,
}, testInfo) => {
  await page.goto('/');
  await page.getByLabel('Appearance', { exact: true }).selectOption('light');
  const cell = page.getByRole('button', { name: 'Edit row 1 customer', exact: true });
  await cell.click();
  const input = page.getByRole('textbox', { name: 'Cell value row 1 customer', exact: true });
  await input.fill('Draft');
  await input.press('Backspace');
  await expect(page.locator('.pending-delete')).toHaveCount(0);
  await input.press('Enter');
  await expect(cell.locator('..')).toHaveClass(/edited/);
  await expect(cell.locator('..')).toHaveCSS('background-color', 'rgb(255, 242, 198)');
  await page.getByRole('button', { name: 'Select row 1', exact: true }).click();
  await page.keyboard.press('Backspace');
  await expect(page.locator('tr.pending-delete')).toHaveCount(1);
  for (const [theme, color] of [
    ['light', 'rgb(255, 226, 227)'],
    ['dark', 'rgb(81, 45, 52)'],
  ]) {
    await page.getByLabel('Appearance', { exact: true }).selectOption(theme);
    const cells = page.locator('tr.pending-delete td');
    for (let i = 0; i < (await cells.count()); i++)
      await expect(cells.nth(i)).toHaveCSS('background-color', color);
    await page.screenshot({ path: testInfo.outputPath(`pending-delete-${theme}.png`) });
  }
  await expect(page.getByText('450 total rows')).toBeVisible();
  await page.keyboard.press('Meta+w');
  await expect(page.getByRole('dialog')).toContainText('Unsaved changes');
  await page.getByRole('button', { name: 'Keep editing' }).click();
  await page.getByRole('button', { name: 'Restore deleted rows' }).click();
  await expect(page.locator('tr.pending-delete')).toHaveCount(0);
  await expect(cell).toHaveText('Draf');
  await page.getByRole('button', { name: 'Select row 1', exact: true }).click();
  await page.keyboard.press('Delete');
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.getByText('449 total rows')).toBeVisible();
  await expect(cell).toHaveText('Arun K.');
  await page.keyboard.press('Meta+w');
  await expect(page.getByRole('tab', { name: 'orders', exact: true })).toHaveCount(0);
});

test('column reorder preserves correct cells and persists only the layout', async ({ page }) => {
  await page.goto('/');
  const customer = page.getByRole('button', { name: 'Sort by customer', exact: true });
  await customer.focus();
  await customer.press('Alt+ArrowLeft');
  const headings = page.locator('.column-heading');
  await expect(headings.first()).toHaveAttribute('aria-label', 'Sort by customer');
  await expect(page.locator('.data-table tbody tr').first().locator('td').nth(1)).toHaveText(
    'Mali S.',
  );
  await page.reload();
  await expect(headings.first()).toHaveAttribute('aria-label', 'Sort by customer');
  const stored = await page.evaluate(() => localStorage.getItem('buggy-table.column-layouts.v1'));
  expect(stored).toContain('customer');
  expect(stored).not.toContain('Mali S.');
  await page.getByRole('button', { name: 'Sort by total', exact: true }).dragTo(customer);
  await expect(headings.first()).toHaveAttribute('aria-label', 'Sort by total');
  await page.getByRole('button', { name: 'Edit row 1 total', exact: true }).click();
  await page.getByRole('textbox', { name: 'Cell value row 1 total' }).fill('99.50');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1 total', exact: true })).toHaveText(
    '99.50',
  );
});

test('timestamp editor and filter accept typing and calendar without timezone conversion', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Edit row 1 created_at', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Cell value row 1 created_at', exact: true });
  await input.fill('2026-10-19 23:01:02.123456+07:00');
  await page
    .getByRole('button', { name: 'Choose date/time for Cell value row 1 created_at' })
    .click();
  await expect(page.getByLabel('UTC offset')).toHaveValue('+07:00');
  await expect(page.getByLabel('Fractional seconds')).toHaveValue('123456');
  await page.getByLabel('Calendar date').fill('2026-10-20');
  await page.getByLabel('Clock time').fill('22:02:03');
  await page.getByRole('button', { name: 'Apply date and time' }).click();
  await expect(input).toHaveValue('2026-10-20 22:02:03.123456+07:00');
  await input.press('Enter');
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await page.getByRole('button', { name: 'Filters', exact: true }).click();
  await page.getByLabel('Filter 1 column').selectOption('created_at');
  await page.getByLabel('Filter 1 value', { exact: true }).fill('2026-10-20 22:02:03.123456+07:00');
  await page.getByRole('button', { name: 'Choose date/time for Filter 1 value' }).click();
  await expect(page.getByLabel('Calendar date')).toHaveValue('2026-10-20');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await expect(page.getByText('1 rows shown')).toBeVisible();
});

test('full values are optional, wrap long cells and retain sticky row selectors', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 1000, height: 720 });
  await page.goto('/');
  const toggle = page.getByRole('button', { name: 'Full values' });
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await page.getByRole('button', { name: 'Edit row 1 customer', exact: true }).click();
  await page
    .getByRole('textbox', { name: 'Cell value row 1 customer' })
    .fill('Long value '.repeat(120));
  await page.keyboard.press('Enter');
  await toggle.click();
  await expect(page.locator('.grid-scroll')).toHaveClass(/full-values/);
  await expect
    .poll(() =>
      page
        .locator('.data-table tbody tr')
        .first()
        .evaluate((e) => e.getBoundingClientRect().height),
    )
    .toBeGreaterThan(100);
  const before = await page.locator('thead .row-number').boundingBox();
  await page.locator('.grid-scroll').evaluate((e) => {
    e.scrollLeft = 300;
  });
  const after = await page.locator('thead .row-number').boundingBox();
  expect(Math.abs(after!.x - before!.x)).toBeLessThan(2);
  await page.locator('.grid-scroll').evaluate((e) => {
    e.scrollTop = e.scrollHeight;
  });
  await expect(page.getByRole('button', { name: 'Select row 50', exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('full-values.png') });
});
