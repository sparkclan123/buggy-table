import { test, expect, type Page } from '@playwright/test';

async function updaterStub(
  page: Page,
  options: { disabled?: boolean; current?: boolean; error?: boolean; progress?: boolean } = {},
) {
  await page.addInitScript((options) => {
    const target = window as unknown as Record<string, unknown>;
    target.isTauri = true;
    target.updateCalls = [];
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (
        command: string,
        payload: { fetch?: boolean; progress?: { onmessage: (value: unknown) => void } },
      ) => {
        if (command.startsWith('plugin:')) return 1;
        if (command === 'list_connections') return [];
        (target.updateCalls as unknown[]).push({ command, fetch: payload.fetch });
        if (command === 'check_update') {
          if (options.error && payload.fetch)
            throw { message: 'Could not check GitHub for updates. Try again later.' };
          return {
            configured: !options.disabled,
            currentVersion: '0.1.0',
            version: payload.fetch && !options.current ? '0.2.0' : null,
            notes: '<script>unsafe()</script> Release notes',
          };
        }
        if (command === 'install_update') {
          if (options.progress) {
            payload.progress?.onmessage({
              stage: 'downloading',
              downloaded: 1048576,
              total: 2097152,
            });
            return new Promise(() => {});
          }
          throw {
            message: 'Download or signature verification failed. Nothing was installed; try again.',
          };
        }
        return null;
      },
    };
  }, options);
}

test('system theme follows OS; explicit choice survives reload and covers editor and inline inputs', async ({
  page,
}, testInfo) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(page.getByLabel('Appearance')).toHaveValue('system');
  await page.getByRole('button', { name: 'Edit row 1 customer', exact: true }).click();
  await expect(page.getByRole('textbox', { name: /^Cell value row/ })).toHaveCSS(
    'background-color',
    'rgb(28, 30, 33)',
  );
  await page.keyboard.press('Escape');
  await page.getByRole('tab', { name: 'SQL editor', exact: true }).click();
  await expect(page.locator('.cm-editor')).toHaveCSS('background-color', 'rgb(28, 30, 33)');
  await page.locator('.cm-content').click();
  const modifier = await page.evaluate(() =>
    navigator.platform.includes('Mac') ? 'Meta' : 'Control',
  );
  await page.keyboard.press(`${modifier}+f`);
  await expect(page.locator('.cm-panels')).toHaveCSS('background-color', 'rgb(51, 51, 56)');
  await page.screenshot({ path: testInfo.outputPath('dark-theme.png') });
  await page.getByLabel('Appearance').selectOption('light');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('.cm-panels')).toHaveCSS('background-color', 'rgb(245, 245, 245)');
  await page.reload();
  await expect(page.getByLabel('Appearance')).toHaveValue('light');
  await page.getByLabel('Appearance').selectOption('system');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('update offer blocks dirty tabs, renders release text safely, and recovers from failed verification', async ({
  page,
}) => {
  await updaterStub(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Update v0.2.0', exact: true }).click();
  await expect(
    page.getByText('<script>unsafe()</script> Release notes', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Edit row 1 customer', exact: true }).click();
  await page.getByRole('textbox', { name: /^Cell value row/ }).fill('Pending work');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'customers', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Update & restart', exact: true })).toBeDisabled();
  await page.getByRole('tab', { name: /^orders/ }).click();
  await page.getByRole('button', { name: 'Discard changes', exact: true }).click();
  await page.getByRole('button', { name: 'Update & restart', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('signature verification failed');
  await expect(page.locator('.app-shell')).not.toHaveAttribute('inert');
  await expect(page.getByRole('button', { name: 'Update & restart', exact: true })).toBeEnabled();
});

test('download progress locks workspace and automatic checks can be disabled persistently', async ({
  page,
}) => {
  await updaterStub(page, { progress: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Update v0.2.0', exact: true }).click();
  await page.getByLabel('Check automatically').uncheck();
  await page.reload();
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await expect(page.getByLabel('Check automatically')).not.toBeChecked();
  expect(
    await page.evaluate(() => (window as unknown as { updateCalls: unknown[] }).updateCalls),
  ).toEqual([{ command: 'check_update', fetch: false }]);
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await page.getByRole('button', { name: 'Update & restart', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Downloading… 1.0 MB');
  await expect(page.locator('.app-shell')).toHaveAttribute('inert', '');
  await expect(page.getByRole('progressbar')).toHaveAttribute('value', '1048576');
});

test('unconfigured build does not fetch; network failures and latest version have clear states', async ({
  page,
}) => {
  await updaterStub(page, { disabled: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await expect(
    page.getByText('Updates are enabled in signed release builds.', { exact: false }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => (window as unknown as { updateCalls: unknown[] }).updateCalls),
  ).toEqual([{ command: 'check_update', fetch: false }]);
});

for (const failure of [false, true]) {
  test(`manual update check reports ${failure ? 'network failure' : 'latest version'}`, async ({
    page,
  }) => {
    await updaterStub(page, { current: true, error: failure });
    await page.goto('/');
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
    await expect(
      page.getByText(
        failure
          ? 'Could not check GitHub for updates. Try again later.'
          : 'You have the latest version.',
        { exact: true },
      ),
    ).toBeVisible();
  });
}
