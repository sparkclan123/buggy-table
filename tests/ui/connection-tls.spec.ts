import { test, expect } from '@playwright/test';

test('PostgreSQL TLS compatibility is explicit and does not leak into other engines', async ({
  page,
}) => {
  await page.addInitScript(() => {
    const target = window as unknown as Record<string, unknown>;
    target.isTauri = true;
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, payload: { request?: Record<string, unknown> }) => {
        if (command.startsWith('plugin:')) return 1;
        if (command === 'check_update') return { configured: false, currentVersion: '0.1.1' };
        if (command === 'connect') {
          target.submittedTls = { tls: payload.request?.tls, ca: payload.request?.rootCaPem };
          throw { message: 'Authentication failed. Check the database role and password.' };
        }
        return null;
      },
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect database', exact: true }).click();
  const security = page.getByLabel('Transport security');
  await expect(security).toHaveValue('verifyFull');
  await security.selectOption('require');
  await expect(page.getByRole('note')).toContainText("server's identity is not verified");
  await page.getByLabel('Database', { exact: true }).fill('fixture');
  await page.getByLabel('Username', { exact: true }).fill('reader');
  await page.getByLabel('Password', { exact: true }).fill('CANARY_SESSION_ONLY');
  await page.getByRole('button', { name: 'Connect securely' }).click();
  await expect(page.getByRole('alert')).toContainText('Authentication failed');
  expect(
    await page.evaluate(() => (window as unknown as Record<string, unknown>).submittedTls),
  ).toEqual({ tls: 'require', ca: '' });
  await expect(page.getByLabel('Password', { exact: true })).toHaveValue('');
  await page.getByLabel('Database engine').selectOption('mysql');
  await expect(security).toHaveValue('verifyFull');
  await expect(security.locator('option[value="require"]')).toHaveCount(0);
  await page.getByLabel('Database engine').selectOption('postgres');
  await expect(security).toHaveValue('verifyFull');
  expect(
    await page.evaluate(() => ({ local: localStorage.length, session: sessionStorage.length })),
  ).toEqual({ local: 0, session: 0 });
});

test('custom CA chooser rejects private keys and oversized files', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Connect database', exact: true }).click();
  await page.getByText('Custom CA certificate (optional)', { exact: true }).click();
  const file = page.getByLabel('CA certificate (.pem / .crt)');
  await file.setInputFiles({
    name: 'private.pem',
    mimeType: 'text/plain',
    buffer: Buffer.from('-----BEGIN PRIVATE KEY-----\nTEST_ONLY\n-----END PRIVATE KEY-----'),
  });
  await expect(page.getByRole('alert')).toContainText('not a private key');
  await expect(file).toHaveValue('');
  await file.setInputFiles({
    name: 'large.pem',
    mimeType: 'text/plain',
    buffer: Buffer.alloc(65537, 'x'),
  });
  await expect(page.getByRole('alert')).toContainText('64 KiB');
  await expect(file).toHaveValue('');
});

for (const major of [15, 16, 17, 18, 19]) {
  test(`PostgreSQL ${major} connects and only future versions show compatibility notice`, async ({
    page,
  }) => {
    await page.addInitScript((major) => {
      const target = window as unknown as Record<string, unknown>;
      target.isTauri = true;
      target.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: 'main' } },
        transformCallback: () => 1,
        invoke: async (command: string, payload: { request?: Record<string, unknown> }) => {
          if (command.startsWith('plugin:')) return 1;
          if (command === 'check_update') return { configured: false, currentVersion: '0.1.2' };
          if (command === 'connect')
            return {
              ...payload.request,
              id: 'version-fixture',
              name: `PostgreSQL ${major} fixture`,
              serverVersion: String(major * 10000),
              tables: [],
            };
          return null;
        },
      };
    }, major);
    await page.goto('/');
    await page.getByRole('button', { name: 'Connect database', exact: true }).click();
    await page.getByLabel('Database', { exact: true }).fill('fixture');
    await page.getByLabel('Username', { exact: true }).fill('reader');
    await page.getByRole('button', { name: 'Connect securely' }).click();
    await expect(page.getByRole('dialog')).toHaveCount(0);
    await expect(
      page.getByText(`PostgreSQL ${major} fixture`, { exact: true }).first(),
    ).toBeVisible();
    const notice = page.getByText(/Compatibility not yet tested/);
    if (major > 18) await expect(notice).toBeVisible();
    else await expect(notice).toHaveCount(0);
  });
}
