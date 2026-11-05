/**
 * E2E tests for the auto-lock timeout.
 * The browser clock is fast-forwarded, so idle periods don't really take that long.
 */

import { test, expect, type Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

const PASSWORD = 'test-password-123';

/** The unlock form has a password field and, unlike first-run setup, no confirmation. */
function unlockField(page: Page) {
  return page.locator('#password');
}

async function unlock(page: Page, jp: JotteryPage) {
  await expect(unlockField(page)).toBeVisible({ timeout: 15000 });
  await unlockField(page).fill(PASSWORD);
  await unlockField(page).press('Enter');
  await expect(jp.appLoaded.first()).toBeVisible({ timeout: 30000 });
}

async function lockAndUnlock(page: Page, jp: JotteryPage) {
  await jp.lockButton.first().click();
  await unlock(page, jp);
}

async function setTimeoutMinutes(page: Page, jp: JotteryPage, minutes: number) {
  await jp.openSettings();
  await page.locator('#setting-autolock').fill(String(minutes));
  await page.waitForTimeout(1000); // settings auto-save after 500ms
  await page.keyboard.press('Escape');
  await expect(page.locator('#setting-autolock')).toHaveCount(0);
}

/** Sets the flag directly: this is about auto-lock, not the steps to turn it on. */
async function enableRememberPassword(page: Page) {
  await page.evaluate(async () => {
    const databases = await indexedDB.databases();
    const name = databases.find((database) => database.name?.startsWith('jottery'))?.name;
    await new Promise<void>((resolve, reject) => {
      const open = indexedDB.open(name!);
      open.onerror = () => reject(open.error);
      open.onsuccess = () => {
        const db = open.result;
        const transaction = db.transaction('settings', 'readwrite');
        const store = transaction.objectStore('settings');
        const read = store.get('user-settings');
        read.onsuccess = () => store.put({ ...read.result, rememberPassword: true }, 'user-settings');
        transaction.oncomplete = () => {
          db.close();
          resolve();
        };
        transaction.onerror = () => reject(transaction.error);
      };
    });
  });
}

/**
 * Remember Password unlocks again by itself, so a lock leaves no trace by the
 * time a test looks. This records the lock screen appearing at all.
 */
async function watchForLockScreen(page: Page) {
  await page.evaluate(() => {
    const observer = new MutationObserver(() => {
      if (document.querySelector('#password')) document.body.dataset.sawLockScreen = 'true';
    });
    observer.observe(document.body, { childList: true, subtree: true });
  });
}

async function sawLockScreen(page: Page) {
  return page.evaluate(() => document.body.dataset.sawLockScreen === 'true');
}

async function idleFor(page: Page, minutes: number) {
  await page.clock.runFor(minutes * 60 * 1000);
}

async function expectLocked(page: Page) {
  await expect(unlockField(page)).toBeVisible({ timeout: 10000 });
}

async function expectUnlocked(page: Page, jp: JotteryPage) {
  await page.waitForTimeout(1000); // the lock path is async: let it finish if it started
  await expect(unlockField(page)).toHaveCount(0);
  await expect(jp.appLoaded.first()).toBeVisible();
}

test.describe('Auto-lock timeout', () => {
  let jp: JotteryPage;

  test.beforeEach(async ({ page }) => {
    await page.clock.install();
    jp = new JotteryPage(page);
    await jp.setup(PASSWORD);
    await lockAndUnlock(page, jp); // starts the idle timers on the default timeout
  });

  test('a longer timeout applies straight away', async ({ page }) => {
    await setTimeoutMinutes(page, jp, 60);
    await idleFor(page, 16);
    await expectUnlocked(page, jp);
  });

  test('a longer timeout applies after locking and unlocking', async ({ page }) => {
    await setTimeoutMinutes(page, jp, 60);
    await lockAndUnlock(page, jp);
    await idleFor(page, 16);
    await expectUnlocked(page, jp);
  });

  test('a longer timeout applies after a reload', async ({ page }) => {
    await setTimeoutMinutes(page, jp, 60);
    await page.reload();
    await unlock(page, jp);
    await idleFor(page, 16);
    await expectUnlocked(page, jp);
  });

  test('a shorter timeout applies straight away', async ({ page }) => {
    await setTimeoutMinutes(page, jp, 5);
    await idleFor(page, 6);
    await expectLocked(page);
  });

  test('Remember Password keeps auto-lock off', async ({ page }) => {
    await enableRememberPassword(page);
    await page.reload();
    await unlock(page, jp);
    await watchForLockScreen(page);
    await idleFor(page, 16);
    await expectUnlocked(page, jp);
    expect(await sawLockScreen(page)).toBe(false);
  });

  test('the default timeout locks after 15 minutes', async ({ page }) => {
    await idleFor(page, 16);
    await expectLocked(page);
  });

  test('a longer timeout still locks once it has passed', async ({ page }) => {
    await setTimeoutMinutes(page, jp, 60);
    await lockAndUnlock(page, jp);
    await idleFor(page, 61);
    await expectLocked(page);
  });
});

test.describe('Locking with two tabs open', () => {
  test('locks both', async ({ page, context }) => {
    const jp = new JotteryPage(page);
    await jp.setup();
    await jp.createNote('Seen in two tabs');

    const other = await context.newPage();
    await other.goto('', { waitUntil: 'domcontentloaded' });
    await unlock(other, new JotteryPage(other));
    await expect(other.getByText('Seen in two tabs').first()).toBeVisible();

    await jp.lockButton.click();

    await expect(unlockField(other)).toBeVisible({ timeout: 15000 });
    await expect(other.getByText('Seen in two tabs')).toHaveCount(0);
  });
});

test.describe('However the app locks', () => {
  test('nothing decrypted stays in memory, even when the key is cleared on its own', async ({ page }) => {
    const jp = new JotteryPage(page);
    await jp.setup();

    const left = await page.evaluate(async () => {
      const { inboxItems, notes } = await import('/src/lib/stores/appStore.ts' as string);
      const { keyManager } = await import('/src/lib/services/keyManager.ts' as string);
      const read = (store: { subscribe: (run: (value: unknown[]) => void) => () => void }) => {
        let value: unknown[] = [];
        store.subscribe(current => { value = current; })();
        return value;
      };

      inboxItems.set([{ id: 'a', content: 'secret', tags: [], createdAt: '2026-01-01T00:00:00Z', sizeBytes: 6 }]);
      keyManager.clearMasterKey();
      await new Promise(resolve => setTimeout(resolve, 100));

      return { inbox: read(inboxItems).length, notes: read(notes).length };
    });

    expect(left).toEqual({ inbox: 0, notes: 0 });
  });
});
