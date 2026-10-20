import { test, expect, type Page } from '@playwright/test';
import { editCell } from './grid-helpers';

async function setup(page: Page) {
  await page.addInitScript(() => {
    const target = window as unknown as Record<string, unknown>;
    target.isTauri = true;
    const calls: { command: string; payload: Record<string, unknown> }[] = [];
    target.vpnCalls = calls;
    const owner = '11111111-1111-4111-8111-111111111111';
    const other = '22222222-2222-4222-8222-222222222222';
    const vpnId = '33333333-3333-4333-8333-333333333333';
    const profiles = [owner, other].map((id, i) => ({
      id,
      engine: 'postgres',
      name: i ? 'Beta' : 'Alpha',
      host: 'vpn-db.test',
      port: 5432,
      database: 'fixture',
      username: 'reader',
      schema: 'public',
      tls: 'verifyFull',
      rootCaPem: '',
      manualEdits: true,
      group: '',
      pinned: !i,
      lastUsed: 2 - i,
      hasPassword: true,
      layout: { selectedSchema: 'public', tabs: [], activeTab: 'empty' },
      vpn: { enabled: false, profileId: i ? null : vpnId },
    }));
    const vpns = [
      {
        id: vpnId,
        owner,
        name: 'Office',
        username: 'vpn-user',
        dns: [],
        hasPassword: true,
        hasKeyPassword: false,
      },
    ];
    const table = {
      schema: 'public',
      name: 'items',
      estimatedRows: 1,
      columns: [
        { name: 'id', dataType: 'int4', primaryKey: true, nullable: false, editable: false },
        { name: 'label', dataType: 'text', primaryKey: false, nullable: false, editable: true },
      ],
    };
    let generation = 0;
    const connected = (p: (typeof profiles)[number]) => ({
      connection: {
        ...p,
        id: `session-${p.id}-${++generation}`,
        vpnOwner: p.id,
        serverVersion: '170000',
        tables: [table],
      },
      profile: structuredClone(p),
      warning: null,
    });
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, payload: Record<string, unknown> = {}) => {
        if (command.startsWith('plugin:')) return 1;
        calls.push({ command, payload: JSON.parse(JSON.stringify(payload)) });
        if (command === 'check_update') return { configured: false, currentVersion: '0.1.10' };
        if (command === 'list_connections') return structuredClone(profiles);
        if (command === 'reconnect_profile')
          return connected(profiles.find((p) => p.id === payload.id)!);
        if (command === 'list_schemas') return ['public'];
        if (command === 'schema_tables') return [table];
        if (command === 'browse_table' || command === 'run_query')
          return {
            columns: ['id', 'label'],
            rows: [['1', 'Original']],
            elapsedMs: 3,
            truncated: false,
            sql: 'SELECT …',
          };
        if (command === 'update_connection') {
          const p = profiles.find((p) => p.id === payload.id)!;
          p.layout = payload.layout as typeof p.layout;
          return null;
        }
        if (command === 'vpn_profiles')
          return structuredClone(vpns.filter((p) => p.owner === payload.owner));
        if (command === 'vpn_runtime') return { available: true, prepared: true, message: 'Ready' };
        if (command === 'save_vpn_profile') {
          const edit = payload.edit as Record<string, unknown>;
          const old = vpns.find((p) => p.id === edit.id);
          const p = {
            id: String(edit.id ?? crypto.randomUUID()),
            owner: String(edit.owner),
            name: String(edit.name),
            username: String(edit.username),
            dns: edit.dns as never[],
            hasPassword: Boolean(edit.password || old?.hasPassword),
            hasKeyPassword: false,
          };
          if (old) Object.assign(old, p);
          else vpns.push(p);
          return p;
        }
        if (command === 'remove_vpn_profile') {
          vpns.splice(
            vpns.findIndex((p) => p.id === payload.id),
            1,
          );
          const p = profiles.find((p) => p.id === payload.owner);
          if (p && p.vpn.profileId === payload.id) p.vpn = { enabled: false, profileId: null };
          return p ?? null;
        }
        if (command === 'set_connection_vpn') {
          if (target.vpnFail)
            throw {
              code: 'VPN_AUTH',
              message: 'OpenVPN rejected authentication. Original connection retained.',
            };
          const p = profiles.find((p) => p.id === payload.owner)!;
          p.vpn = payload.selection as typeof p.vpn;
          return payload.connectionId
            ? connected(p)
            : { connection: null, profile: structuredClone(p), warning: null };
        }
        if (command === 'vpn_status') {
          const enabled = profiles.find((p) => String(payload.connectionId).includes(p.id))?.vpn
            .enabled;
          const failed = target.vpnProbeFail && payload.probe;
          return {
            state: enabled ? (failed ? 'unreachable' : 'ready') : 'direct',
            profileId: enabled ? vpnId : null,
            profileName: enabled ? 'Office' : null,
            connectedAt: 1700000000,
            checkedAt: payload.probe ? 1700000001 : null,
            latencyMs: enabled && !failed ? 12 : null,
            message: failed
              ? 'The VPN cannot reach the database TCP port.'
              : enabled
                ? 'VPN and database TCP route are reachable.'
                : 'Using this device’s normal network.',
          };
        }
        if (command === 'connect_profile') {
          target.vpnSubmission = payload;
          throw { message: 'Fixture stop after checking network selection.' };
        }
        return null;
      },
    };
  });
  await page.goto('/');
}
async function start(page: Page) {
  await setup(page);
  await page.getByRole('button', { name: 'Reconnect Alpha', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'items TABLE' })).toBeVisible();
}

test('VPN default and enabled preference belongs to one database and is restored on reconnect', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'VPN settings for Alpha', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'VPN settings for Alpha' });
  await expect(dialog.getByRole('radio', { name: 'Default VPN Office' })).toBeChecked();
  await dialog.getByLabel('Use VPN for this database').check();
  await dialog.getByRole('button', { name: 'Apply', exact: true }).click();
  await expect(page.getByRole('switch', { name: 'Use VPN for Alpha' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await page.getByRole('button', { name: 'Reconnect Alpha', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'items TABLE' })).toBeVisible();
  await expect(page.getByLabel('VPN ready', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Search or switch connections' }).click();
  await expect(page.getByRole('switch', { name: 'Use VPN for Beta' })).toHaveAttribute(
    'aria-checked',
    'false',
  );
});

test('VPN profiles can be imported, renamed, selected and removed without browser secret storage', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'VPN settings for Alpha', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'VPN settings for Alpha' });
  await dialog.getByRole('button', { name: 'Add VPN', exact: true }).click();
  await dialog.getByLabel('OpenVPN profile', { exact: true }).setInputFiles({
    name: 'Travel.ovpn',
    mimeType: 'text/plain',
    buffer: Buffer.from('client\n<key>\nPRIVATE_CANARY\n</key>'),
  });
  await dialog.getByLabel('VPN username', { exact: true }).fill('vpn-user');
  await dialog.getByLabel('VPN password', { exact: true }).fill('SECRET_CANARY');
  await dialog.getByRole('button', { name: 'Save VPN', exact: true }).click();
  await dialog.getByRole('button', { name: 'Edit VPN Travel', exact: true }).click();
  await expect(dialog.getByLabel('VPN password', { exact: true })).toHaveValue('');
  await dialog.getByLabel('Name', { exact: true }).fill('Travel renamed');
  await dialog.getByRole('button', { name: 'Save VPN', exact: true }).click();
  await dialog.getByRole('radio', { name: 'Default VPN Travel renamed' }).check();
  await dialog.getByRole('button', { name: 'Remove VPN Travel renamed' }).click();
  await dialog.getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(dialog.getByRole('radio', { name: 'Default VPN Travel renamed' })).toHaveCount(0);
  expect(
    await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage })),
  ).not.toMatch(/CANARY/);
  const edits = await page.evaluate(() =>
    (
      window as unknown as {
        vpnCalls: {
          command: string;
          payload: { edit: { name: string; password: string | null; config: string | null } };
        }[];
      }
    ).vpnCalls
      .filter((c) => c.command === 'save_vpn_profile')
      .map((c) => c.payload.edit),
  );
  expect(edits[1].password).toBeNull();
  expect(edits[1].config).toBeNull();
});

test('VPN route checks do not query tables; reconnect refreshes only the selected database', async ({
  page,
}) => {
  await start(page);
  await page.getByRole('switch', { name: 'Use VPN for Alpha' }).click();
  await expect(page.getByLabel('VPN ready', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'VPN settings for Alpha', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'VPN settings for Alpha' });
  await expect(dialog.getByText('12 ms · TCP route')).toBeVisible();
  const count = await page.evaluate(
    () => (window as unknown as { vpnCalls: unknown[] }).vpnCalls.length,
  );
  await dialog.getByRole('button', { name: 'Test route', exact: true }).click();
  await expect(dialog.getByText(/^Checked /)).toBeVisible();
  const calls = await page.evaluate(
    (from) => (window as unknown as { vpnCalls: { command: string }[] }).vpnCalls.slice(from),
    count,
  );
  expect(calls.some((c) => ['browse_table', 'run_query'].includes(c.command))).toBe(false);
  await page.evaluate(() => ((window as unknown as Record<string, unknown>).vpnProbeFail = true));
  await dialog.getByRole('button', { name: 'Test route', exact: true }).click();
  await expect(dialog.getByText('Database unreachable', { exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: 'Reconnect', exact: true }).click();
  await expect(dialog.getByText('VPN connected', { exact: true })).toBeVisible();
});

test('failed VPN switch keeps the direct session; unsaved edits block route changes', async ({
  page,
}) => {
  await start(page);
  await page.evaluate(() => ((window as unknown as Record<string, unknown>).vpnFail = true));
  await page.getByRole('switch', { name: 'Use VPN for Alpha' }).click();
  await expect(page.getByRole('alert')).toContainText('Original connection retained');
  await expect(page.getByRole('switch', { name: 'Use VPN for Alpha' })).toHaveAttribute(
    'aria-checked',
    'false',
  );
  await editCell(page, 1, 'label');
  await page.getByLabel('Cell value row 1 label', { exact: true }).fill('Draft');
  await page.getByLabel('Cell value row 1 label', { exact: true }).press('Enter');
  const count = await page.evaluate(
    () => (window as unknown as { vpnCalls: unknown[] }).vpnCalls.length,
  );
  await page.getByRole('switch', { name: 'Use VPN for Alpha' }).click();
  await expect(page.getByRole('alert')).toContainText('pending edits');
  const calls = await page.evaluate(
    (from) => (window as unknown as { vpnCalls: { command: string }[] }).vpnCalls.slice(from),
    count,
  );
  expect(calls.some((c) => c.command === 'set_connection_vpn')).toBe(false);
});

test('new database sends its chosen VPN selection with the connection request', async ({
  page,
}) => {
  await setup(page);
  await page.getByRole('button', { name: 'New connection', exact: true }).click();
  await page.getByRole('button', { name: 'VPN settings for Local PostgreSQL' }).click();
  const dialog = page.getByRole('dialog', { name: 'VPN settings for Local PostgreSQL' });
  await dialog.getByRole('button', { name: 'Add VPN', exact: true }).click();
  await dialog.getByLabel('OpenVPN profile', { exact: true }).setInputFiles({
    name: 'New.ovpn',
    mimeType: 'text/plain',
    buffer: Buffer.from('client\nTEST_ONLY'),
  });
  await dialog.getByRole('button', { name: 'Save VPN', exact: true }).click();
  await dialog.getByLabel('Use VPN for this database').check();
  await dialog.getByRole('button', { name: 'Apply', exact: true }).click();
  await page.getByLabel('Host', { exact: true }).fill('vpn-db.test');
  await page.getByLabel('Database', { exact: true }).fill('fixture');
  await page.getByLabel('Username', { exact: true }).fill('reader');
  await page.getByRole('button', { name: 'Connect securely' }).click();
  await expect(page.getByRole('alert')).toContainText('Fixture stop');
  const payload = await page.evaluate(
    () =>
      (
        window as unknown as {
          vpnSubmission: { vpn: { enabled: boolean; profileId: string }; savedId: string };
        }
      ).vpnSubmission,
  );
  expect(payload.vpn.enabled).toBe(true);
  expect(payload.vpn.profileId).toBeTruthy();
  expect(payload.savedId).toMatch(/^[\da-f-]{36}$/);
  await page.getByRole('button', { name: 'Close connection dialog' }).click();
  await expect(page.getByRole('heading', { name: 'Connect your database' })).toHaveCount(0);
  const cleanup = await page.evaluate(
    () =>
      (
        window as unknown as {
          vpnCalls: { command: string; payload: { owner: string } }[];
        }
      ).vpnCalls.find((c) => c.command === 'discard_vpn_draft')?.payload.owner,
  );
  expect(cleanup).toBe(payload.savedId);
});

test('VPN controls fit a small desktop window in both themes', async ({ page }, info) => {
  await page.setViewportSize({ width: 1000, height: 700 });
  await start(page);
  await page.getByRole('switch', { name: 'Use VPN for Alpha' }).click();
  await expect(page.getByLabel('VPN ready', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'VPN settings for Alpha', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'VPN settings for Alpha' });
  await expect(dialog.getByRole('button', { name: 'Apply', exact: true })).toBeEnabled();
  for (const theme of ['light', 'dark']) {
    await page.emulateMedia({ colorScheme: theme as 'light' | 'dark' });
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
    if (info.project.name === 'chromium')
      await page.screenshot({ path: `artifacts/vpn-ui-${theme}.png` });
  }
});
