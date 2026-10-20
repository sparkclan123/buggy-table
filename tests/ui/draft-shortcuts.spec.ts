import { test, expect, type Page } from '@playwright/test';
import { editCell } from './grid-helpers';
const mod = process.platform === 'darwin' ? 'Meta' : 'Control';
// Exercise browser clipboard events including native keyboard dispatch, without
// relying on headless OS clipboard ownership shared between parallel workers.
async function clipboard(page: Page) {
  await page.evaluate(() => {
    let saved: Record<string, string> = {};
    document.addEventListener('copy', (e) => {
      if (e.defaultPrevented && e.clipboardData)
        saved = Object.fromEntries(
          [...e.clipboardData.types].map((t) => [t, e.clipboardData!.getData(t)]),
        );
    });
    (window as unknown as { pasteFixture: () => void }).pasteFixture = () => {
      const data = new DataTransfer();
      Object.entries(saved).forEach(([k, v]) => data.setData(k, v));
      document.activeElement?.dispatchEvent(
        new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: data }),
      );
    };
  });
}
async function paste(page: Page) {
  await page.evaluate(() => (window as unknown as { pasteFixture: () => void }).pasteFixture());
}

test('undo groups unsaved cell changes, deletion and restoration without touching SQL history', async ({
  page,
}) => {
  await page.goto('/');
  await editCell(page, 1, 'customer');
  const input = page.getByLabel('Cell value row 1 customer', { exact: true });
  await input.fill('First edit');
  await input.press('Enter');
  await page.keyboard.press(`${mod}+z`);
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'Mali S.',
  );
  await page.keyboard.press(`${mod}+Shift+z`);
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'First edit',
  );
  await page.keyboard.press('Delete');
  await expect(page.locator('.pending-delete')).toHaveCount(1);
  await page.keyboard.press(`${mod}+z`);
  await expect(page.locator('.pending-delete')).toHaveCount(0);
  await editCell(page, 1, 'customer');
  await input.fill('Current edit');
  await input.press(`${mod}+z`);
  await expect(input).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'First edit',
  );
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await page.getByRole('button', { name: 'Select row 1', exact: true }).click();
  await page.keyboard.press(`${mod}+z`);
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'First edit',
  );
});

test('row copy appends drafts with defaults, supports undo and saves only on request', async ({
  page,
}) => {
  await page.goto('/');
  await clipboard(page);
  await page.getByRole('button', { name: 'Select row 1', exact: true }).click();
  await page.keyboard.press(`${mod}+c`);
  await paste(page);
  await expect(page.getByText('100 rows shown + 1 new')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Select row 101 id', exact: true })).toHaveText(
    'DEFAULT',
  );
  await expect(page.getByRole('button', { name: 'Edit row 101 customer', exact: true })).toHaveText(
    'Mali S.',
  );
  await expect(page.getByText('450 total rows')).toBeVisible();
  await page.keyboard.press(`${mod}+z`);
  await expect(page.getByText('100 rows shown + 1 new')).toHaveCount(0);
  await page.keyboard.press(`${mod}+Shift+z`);
  await expect(page.getByText('100 rows shown + 1 new')).toBeVisible();
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.getByText('451 total rows')).toBeVisible();
  await page.getByLabel('Rows per page').selectOption('500');
  await page.locator('.grid-scroll').evaluate((e) => (e.scrollTop = e.scrollHeight));
  await expect(page.getByRole('button', { name: 'Select row 451 id', exact: true })).toHaveText(
    '451',
  );
  await expect(page.getByRole('button', { name: 'Edit row 451 customer', exact: true })).toHaveText(
    'Mali S.',
  );
});

test('long cell editor expands inside viewport, wraps content and restores on Escape', async ({
  page,
}, info) => {
  await page.goto('/');
  await editCell(page, 1, 'customer');
  const input = page.getByLabel('Cell value row 1 customer', { exact: true });
  await input.fill('Long content '.repeat(300));
  const box = await input.boundingBox();
  expect(box!.width).toBeGreaterThan(300);
  expect(box!.height).toBeGreaterThan(100);
  expect(box!.x + box!.width).toBeLessThanOrEqual(1380);
  await expect(input).toHaveCSS('white-space', 'pre-wrap');
  expect(await input.evaluate((e) => getComputedStyle(e, '::-webkit-scrollbar').width)).toBe('3px');
  await page.screenshot({ path: info.outputPath('expanded-editor.png') });
  await input.press('Escape');
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'Mali S.',
  );
  await expect(page.locator('td.edited')).toHaveCount(0);
});

test('Cmd/Ctrl Enter runs SQL without inserting a line', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('tab', { name: 'SQL editor', exact: true }).click();
  const editor = page.getByLabel('SQL query editor');
  await editor.fill('SELECT 42;');
  await editor.press(`${mod}+Enter`);
  await expect(page.getByRole('alert')).toContainText('Connect');
  await expect(editor).toHaveText('SELECT 42;');
});

test('multi-row drafts stay in their own table and deleting unsaved copies does not delete database rows', async ({
  page,
}) => {
  await page.goto('/');
  await clipboard(page);
  await page.getByRole('button', { name: 'Select row 1', exact: true }).click();
  await page
    .getByRole('button', { name: 'Select row 2', exact: true })
    .click({ modifiers: ['Shift'] });
  await page.keyboard.press(`${mod}+c`);
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await page.getByRole('button', { name: 'Select row 1', exact: true }).click();
  await paste(page);
  await expect(page.getByText('3 rows shown')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Save changes', exact: true })).toHaveCount(0);
  await page.getByRole('tab', { name: 'orders', exact: true }).click();
  await page.getByRole('button', { name: 'Select row 1', exact: true }).focus();
  await paste(page);
  await expect(page.getByText('100 rows shown + 2 new')).toBeVisible();
  await page.keyboard.press('Delete');
  await expect(page.locator('.pending-delete')).toHaveCount(2);
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.getByText('450 total rows')).toBeVisible();
  await expect(page.getByText('100 rows shown + 2 new')).toHaveCount(0);
});
