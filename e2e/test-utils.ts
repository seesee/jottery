/**
 * Shared test utilities for e2e tests
 *
 * RADICAL SIMPLIFICATION: Playwright creates a fresh browser context for each test.
 * This means IndexedDB is already empty - we don't need to clear anything!
 * The previous approach of clearing storage was causing race conditions.
 */

import { Page, expect } from '@playwright/test';
import { JotteryPage } from './page-objects';

// Re-export the page object for convenience
export { JotteryPage } from './page-objects';

/**
 * Clears all browser storage (localStorage, sessionStorage, IndexedDB)
 * NOTE: This is typically NOT needed because Playwright uses fresh contexts.
 * Only use this if you need to reset state within a single test.
 */
export async function clearAllStorage(page: Page): Promise<void> {
  await page.evaluate(async () => {
    localStorage.clear();
    sessionStorage.clear();

    const databases = await indexedDB.databases();
    await Promise.all(
      databases.map((db) => {
        return new Promise<void>((resolve) => {
          if (!db.name) {
            resolve();
            return;
          }
          const request = indexedDB.deleteDatabase(db.name);
          request.onsuccess = () => resolve();
          request.onerror = () => resolve();
          request.onblocked = () => resolve();
        });
      })
    );
  });
}

/**
 * Handles the landing page flow for new users
 *
 * NOTE: Since tests use /test baseURL, the landing page is never shown.
 */
export async function handleLandingPage(page: Page): Promise<void> {
  await page.waitForSelector('#app', { state: 'attached', timeout: 30000 });
  const passwordField = page.locator('#password');
  await expect(passwordField).toBeVisible({ timeout: 30000 });
}

/**
 * Sets up a fresh test environment with password.
 * Delegates to JotteryPage.setup() for the actual implementation.
 */
export async function setupFreshEnvironment(page: Page, password: string = 'test-password-123'): Promise<void> {
  const jp = new JotteryPage(page);
  await jp.setup(password);
}
