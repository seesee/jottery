import { test, expect, type Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

/**
 * The account key is shared by every device on an account, and the server sees the
 * account password, so these flows decide whether a server can read notes and whether
 * an account keeps its key. The services are tested directly elsewhere. These prove the
 * setup screens call them, in the right order, before anything is saved.
 */

const NOTES_PASSWORD = 'test-password-123';
const ACCOUNT_PASSWORD = 'account-password-long-enough';
const ENDPOINT = 'https://sync.example.test';
const OTHER = 'https://elsewhere.example.test';
const USER_ID = 'b3c1f5a2-8f0e-4c55-9d52-2f6a1c0e7a11';

interface FakeServer {
  /** Method and path of every request, apart from preflights */
  calls: string[];
  /** What GET wrapped-key returns, or null for a 404 */
  wrappedKey: string | null;
  /** Status and body for register-user */
  registerUser: { status: number; body: string };
}

async function fakeServer(page: Page, overrides: Partial<FakeServer> = {}): Promise<FakeServer> {
  const server: FakeServer = {
    calls: [],
    wrappedKey: null,
    registerUser: { status: 200, body: JSON.stringify({ userId: USER_ID, email: 'a@example.test', status: 'pending_approval', message: 'Waiting for approval' }) },
    ...overrides,
  };

  await page.route(/example\.test/, async route => {
    const request = route.request();
    const headers = { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Headers': '*', 'Access-Control-Allow-Methods': '*' };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers });

    const { origin, pathname } = new URL(request.url());
    server.calls.push(`${request.method()} ${origin === ENDPOINT ? pathname : origin}`);

    if (pathname.endsWith('/auth/register-user')) {
      return route.fulfill({ status: server.registerUser.status, headers, body: server.registerUser.body });
    }
    if (pathname.endsWith('/auth/register-device')) {
      return route.fulfill({ status: 200, headers, json: { clientId: 'client', apiKey: 'plaintext-key', userId: USER_ID, deviceName: 'Test' } });
    }
    if (pathname.endsWith('/auth/wrapped-key')) {
      if (request.method() === 'PUT') return route.fulfill({ status: 200, headers, json: {} });
      return server.wrappedKey
        ? route.fulfill({ status: 200, headers, json: { blob: server.wrappedKey, kdfVersion: 1, kdfIterations: 1000000 } })
        : route.fulfill({ status: 404, headers, body: 'none' });
    }
    if (pathname.endsWith('/sync/push')) return route.fulfill({ status: 200, headers, json: { accepted: [], rejected: [] } });
    if (pathname.endsWith('/sync/pull')) return route.fulfill({ status: 200, headers, json: { notes: [], attachments: [], versions: [] } });
    return route.fulfill({ status: 200, headers, json: {} });
  });

  return server;
}

/** A blob as a server could send it: a key of its choosing, wrapped with this password. */
async function wrappedRandomKey(page: Page, password: string): Promise<string> {
  return page.evaluate(async ({ password, userId }) => {
    const { cryptoService } = await import('/src/lib/services/crypto.ts' as string);
    const wrappingKey = await cryptoService.deriveWrappingKey(password, userId);
    const key = crypto.getRandomValues(new Uint8Array(32));
    return JSON.stringify(await cryptoService.wrapMasterKey(wrappingKey, key));
  }, { password, userId: USER_ID });
}

/** The account's real key: the one this vault already uses, wrapped with this password. */
async function wrappedLocalKey(page: Page, password: string): Promise<string> {
  return page.evaluate(async ({ password, userId }) => {
    const { cryptoService } = await import('/src/lib/services/crypto.ts' as string);
    const { keyManager } = await import('/src/lib/services/keyManager.ts' as string);
    const wrappingKey = await cryptoService.deriveWrappingKey(password, userId);
    return JSON.stringify(await cryptoService.wrapMasterKey(wrappingKey, keyManager.getMasterKey().keyBytes));
  }, { password, userId: USER_ID });
}

test.describe('Sync setup', () => {
  let app: JotteryPage;

  test.beforeEach(async ({ page }) => {
    app = new JotteryPage(page);
    await app.setup(NOTES_PASSWORD);
  });

  async function openSyncTab(page: Page) {
    await app.openSettings();
    await app.dialog.first().locator('button, [role="tab"]').filter({ hasText: /^Sync$/ }).first().click();
  }

  async function openSetup(page: Page) {
    await openSyncTab(page);
    await page.locator('label:has(input.sr-only)').first().click();
    await page.locator('#sync-endpoint').fill(ENDPOINT);
    await page.locator('#sync-device-name').fill('Test device');
  }

  async function fillNewAccount(page: Page, accountPassword: string, notesPassword: string) {
    await page.getByRole('button', { name: /Register new account/ }).click();
    await page.locator('#user-email').fill('a@example.test');
    await page.locator('#user-password').fill(accountPassword);
    await page.locator('#user-password-confirm').fill(accountPassword);
    await page.locator('#notes-password-new').fill(notesPassword);
    await page.getByRole('button', { name: /Register user account/ }).click();
  }

  /** Once an admin has approved the account, the user finishes setting up the device */
  async function finishNewAccount(page: Page) {
    await page.getByRole('button', { name: 'I\'ve been approved - Complete setup' }).click();
  }

  async function fillExistingAccount(page: Page, accountPassword: string, notesPassword: string) {
    await page.getByRole('button', { name: /Connect to existing account/ }).click();
    await page.locator('#existing-email').fill('a@example.test');
    await page.locator('#existing-password').fill(accountPassword);
    await page.locator('#existing-notes-password').fill(notesPassword);
    await page.getByRole('button', { name: /Connect device/ }).click();
  }

  const syncIsConfigured = (page: Page) => page.evaluate(async () => {
    const { syncRepository } = await import('/src/lib/services/syncRepository.ts' as string);
    return !!(await syncRepository.getMetadata())?.apiKey;
  });

  test.describe('matching passwords', () => {
    test('a new account is refused before anything reaches the server', async ({ page }) => {
      const server = await fakeServer(page);
      await openSetup(page);

      await fillNewAccount(page, NOTES_PASSWORD, NOTES_PASSWORD);

      await expect(page.getByText(/must be different/i)).toBeVisible({ timeout: 5000 });
      expect(server.calls.filter(call => call.includes('/auth/'))).toEqual([]);
    });

    test('connecting to an account where they already match warns, and connects', async ({ page }) => {
      // Refusing would stop these users adding a device at all. They didn't choose
      // either password here, and changing one is possible once they're in.
      const server = await fakeServer(page);
      server.wrappedKey = await wrappedLocalKey(page, NOTES_PASSWORD);
      await openSetup(page);

      await fillExistingAccount(page, NOTES_PASSWORD, NOTES_PASSWORD);

      await expect(page.getByText(/same as your account password/i)).toBeVisible({ timeout: 15000 });
      await expect(page.getByText(/must be different/i)).toHaveCount(0);
      expect(await syncIsConfigured(page)).toBe(true);
    });
  });

  test.describe('choosing a new account for an email that has one', () => {
    test('moves to joining the account, and uploads nothing', async ({ page }) => {
      const server = await fakeServer(page, { registerUser: { status: 409, body: 'Email already registered' } });
      await openSetup(page);

      await fillNewAccount(page, ACCOUNT_PASSWORD, NOTES_PASSWORD);

      await expect(page.locator('#existing-email')).toHaveValue('a@example.test', { timeout: 5000 });
      await expect(page.getByText(/already has an account/i)).toBeVisible();
      expect(server.calls).not.toContain('PUT /api/v1/auth/wrapped-key');
    });

    test('never replaces the key an account already has', async ({ page }) => {
      await app.createNote('A note only this device has');
      const server = await fakeServer(page);
      server.wrappedKey = await wrappedRandomKey(page, NOTES_PASSWORD);
      await openSetup(page);

      await fillNewAccount(page, ACCOUNT_PASSWORD, NOTES_PASSWORD);
      await finishNewAccount(page);

      // The account has a different key, so joining would strand this device's notes
      await expect(page.getByText(/use a different key/i)).toBeVisible({ timeout: 15000 });
      expect(server.calls).not.toContain('PUT /api/v1/auth/wrapped-key');
      expect(server.calls).not.toContain('POST /api/v1/sync/push');
      expect(await syncIsConfigured(page)).toBe(false);
    });

    test('uploads the key when the account has none', async ({ page }) => {
      const server = await fakeServer(page);
      await openSetup(page);

      await fillNewAccount(page, ACCOUNT_PASSWORD, NOTES_PASSWORD);
      await finishNewAccount(page);

      await expect.poll(() => server.calls, { timeout: 15000 }).toContain('PUT /api/v1/auth/wrapped-key');
      expect(await syncIsConfigured(page)).toBe(true);
    });
  });

  test.describe('a key wrapped with the account password', () => {
    test('is refused, with a way forward', async ({ page }) => {
      const server = await fakeServer(page);
      server.wrappedKey = await wrappedRandomKey(page, ACCOUNT_PASSWORD);
      await openSetup(page);

      await fillExistingAccount(page, ACCOUNT_PASSWORD, NOTES_PASSWORD);

      await expect(page.getByText(/already connected will fix it/i)).toBeVisible({ timeout: 15000 });
      expect(await syncIsConfigured(page)).toBe(false);
    });
  });

  test.describe('changing server', () => {
    test('disconnecting lifts the limit on where the app can connect', async ({ page }) => {
      await app.createNote('Some note');
      await fakeServer(page);
      await page.evaluate(async endpoint => {
        const { syncRepository } = await import('/src/lib/services/syncRepository.ts' as string);
        const { settingsRepository } = await import('/src/lib/services/settingsRepository.ts' as string);
        const { cryptoService } = await import('/src/lib/services/crypto.ts' as string);
        const { keyManager } = await import('/src/lib/services/keyManager.ts' as string);
        await syncRepository.updateMetadata({
          apiKey: JSON.stringify(await cryptoService.encryptText('key', keyManager.getMasterKey().key)),
          clientId: 'client',
          userId: 'b3c1f5a2-8f0e-4c55-9d52-2f6a1c0e7a11',
          syncEnabled: true,
          syncEndpoint: endpoint,
        });
        await settingsRepository.update({ syncEnabled: true, syncEndpoint: endpoint });
      }, ENDPOINT);

      const unlock = async () => {
        const password = page.locator('#password');
        await expect(password).toBeVisible({ timeout: 30000 });
        await password.fill(NOTES_PASSWORD);
        await password.press('Enter');
        await expect(app.noteListItems.first()).toBeVisible({ timeout: 30000 });
      };
      const canReach = (url: string) => page.evaluate(async target => {
        try {
          await fetch(target);
          return true;
        } catch {
          return false;
        }
      }, url);

      await page.reload({ waitUntil: 'domcontentloaded' });
      await unlock();
      expect(await canReach(`${OTHER}/health`)).toBe(false);

      await openSyncTab(page);
      await page.getByRole('button', { name: /Danger Zone/ }).click();
      await page.getByRole('button', { name: /^Disconnect$/ }).first().click();
      await page.getByRole('dialog').filter({ hasText: /Disconnect from Sync Server\?/ }).getByRole('button', { name: /^Disconnect$/ }).click();

      await unlock();
      expect(await canReach(`${OTHER}/health`)).toBe(true);
    });
  });
});

test.describe('Connecting a new device from the first screen', () => {
  const blobFor = (page: Page, password: string) => wrappedRandomKey(page, password);

  async function openConnectForm(page: Page) {
    await page.getByText('Already using Jottery?').click();
    await page.getByRole('button', { name: /Connect existing account/i }).first().click();
    await page.locator('#connect-endpoint').fill(ENDPOINT);
    await page.locator('#connect-email').fill('a@example.test');
    await page.locator('#connect-server-password').fill(ACCOUNT_PASSWORD);
  }

  const submit = (page: Page, notesPassword: string) => async () => {
    await page.locator('#connect-notes-password').fill(notesPassword);
    await page.getByRole('button', { name: /Connect & Sync/ }).click();
  };

  test('survives a mistyped notes password, which is the likeliest mistake here', async ({ page }) => {
    await page.goto('', { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#app', { state: 'attached' });
    const server = await fakeServer(page);
    server.wrappedKey = await blobFor(page, NOTES_PASSWORD);
    await openConnectForm(page);

    await submit(page, 'mistyped-password')();
    await expect(page.getByText(/could not|couldn't|older version/i).first()).toBeVisible({ timeout: 15000 });

    await submit(page, NOTES_PASSWORD)();

    await expect(page.locator('#connect-notes-password')).toHaveCount(0, { timeout: 15000 });
    expect(server.calls.filter(call => call.endsWith('/auth/register-device'))).toHaveLength(1);
    expect(await page.evaluate(async () => {
      const { syncRepository } = await import('/src/lib/services/syncRepository.ts' as string);
      return (await syncRepository.getMetadata())?.serverKeyVerified;
    })).toBe(true);
  });

  test('writes nothing locally when the key does not open', async ({ page }) => {
    await page.goto('', { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#app', { state: 'attached' });
    const server = await fakeServer(page);
    server.wrappedKey = await blobFor(page, NOTES_PASSWORD);
    await openConnectForm(page);

    await submit(page, 'mistyped-password')();
    await expect(page.getByText(/could not|couldn't|older version/i).first()).toBeVisible({ timeout: 15000 });

    expect(await page.evaluate(async () => {
      const { encryptionRepository } = await import('/src/lib/services/encryptionRepository.ts' as string);
      return !!(await encryptionRepository.getMetadata());
    })).toBe(false);
  });
});

