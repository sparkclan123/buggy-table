import { expect, test } from '@playwright/test';
import { editCell } from './grid-helpers';

test('alternating rows and inspector fields remain distinct in both themes and after virtual scrolling', async ({
  page,
}, testInfo) => {
  await page.goto('/');
  for (const [theme, stripe, icon] of [
    ['light', 'rgb(243, 246, 250)', 'rgb(76, 141, 189)'],
    ['dark', 'rgb(36, 40, 47)', 'rgb(130, 188, 230)'],
  ]) {
    await page.getByLabel('Appearance', { exact: true }).selectOption(theme);
    const first = page
      .getByRole('button', { name: 'Edit row 1 customer', exact: true })
      .locator('..');
    const second = page
      .getByRole('button', { name: 'Edit row 2 customer', exact: true })
      .locator('..');
    await expect(second).toHaveCSS('background-color', stripe);
    expect(await first.evaluate((e) => getComputedStyle(e).backgroundColor)).not.toBe(stripe);
    await expect(page.locator('.table-list button svg').first()).toHaveCSS('color', icon);
    await page.getByRole('button', { name: 'Select row 2 id', exact: true }).click();
    await expect(page.locator('.inspector-field').nth(1)).toHaveCSS('background-color', stripe);
    await expect(page.locator('.inspector-field').nth(3)).toHaveCSS('background-color', stripe);
    await page.screenshot({ path: testInfo.outputPath(`alternating-${theme}.png`) });
    await page.getByRole('button', { name: 'Select row 1 id', exact: true }).click();
  }
  await page.locator('.grid-scroll').evaluate((e) => {
    e.scrollTop = 1100;
  });
  await expect
    .poll(() => page.getByRole('button', { name: 'Edit row 1 customer', exact: true }).count())
    .toBe(0);
  const visibleRows = page.locator('.data-table tbody tr[aria-rowindex]');
  expect(await visibleRows.count()).toBeGreaterThan(0);
  for (const row of await visibleRows.all()) {
    const index = Number(await row.getAttribute('aria-rowindex')) - 2;
    await expect(row.locator('td').first()).toHaveCSS(
      'background-color',
      index % 2 === 1 ? 'rgb(36, 40, 47)' : 'rgb(28, 30, 33)',
    );
  }
});

test('held tabs float and reorder in both directions without switching or losing drafts', async ({
  page,
}, testInfo) => {
  await page.goto('/');
  await editCell(page, 1, 'customer');
  await page.getByRole('textbox', { name: 'Cell value row 1 customer' }).fill('Tab draft');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  const strip = page.getByRole('tablist', { name: 'Open tables' });
  const orders = strip.getByRole('tab', { name: /^orders/ });
  const customers = strip.getByRole('tab', { name: /^customers/ });
  let source = (await orders.boundingBox())!;
  let destination = (await customers.boundingBox())!;
  await page.mouse.move(source.x + source.width / 2, source.y + source.height / 2);
  await page.mouse.down();
  await expect(page.locator('.tab-drag-ghost')).toBeVisible();
  await page.mouse.move(
    destination.x + destination.width - 3,
    destination.y + destination.height / 2,
    { steps: 8 },
  );
  await page.screenshot({ path: testInfo.outputPath('tab-drag.png') });
  await page.mouse.up();
  await expect(strip.getByRole('tab').first()).toHaveText('customers');
  await expect(customers).toHaveAttribute('aria-selected', 'true');
  await orders.click();
  await expect(page.getByRole('button', { name: 'Edit row 1 customer', exact: true })).toHaveText(
    'Tab draft',
  );

  source = (await orders.boundingBox())!;
  destination = (await customers.boundingBox())!;
  await page.mouse.move(source.x + 15, source.y + 15);
  await page.mouse.down();
  await page.mouse.move(destination.x + 3, destination.y + 15, { steps: 8 });
  await page.mouse.up();
  await expect(strip.getByRole('tab').first()).toHaveText('orders');
  await expect(orders).toHaveAttribute('aria-selected', 'true');
  await orders.press('Alt+ArrowRight');
  await expect(strip.getByRole('tab').last()).toHaveText('orders');
  await expect(orders).toBeFocused();
});

test('Escape and dropping outside the tab strip cancel reordering', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  const strip = page.getByRole('tablist', { name: 'Open tables' });
  for (const cancel of ['escape', 'outside']) {
    const source = (await strip.getByRole('tab', { name: /^orders/ }).boundingBox())!;
    const target = (await strip.getByRole('tab', { name: /^customers/ }).boundingBox())!;
    await page.mouse.move(source.x + 15, source.y + 15);
    await page.mouse.down();
    await page.mouse.move(target.x + target.width - 3, target.y + 15, { steps: 5 });
    await expect(page.locator('.tab-drag-ghost')).toBeVisible();
    if (cancel === 'escape') await page.keyboard.press('Escape');
    else await page.mouse.move(target.x, target.y + 150);
    await page.mouse.up();
    await expect(strip.getByRole('tab').first()).toHaveText('orders');
    await expect(strip.getByRole('tab', { name: /^customers/ })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await expect(page.locator('.tab-drag-ghost')).toHaveCount(0);
  }
});
