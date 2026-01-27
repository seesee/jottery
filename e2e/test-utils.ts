/**
 * Shared test utilities for e2e tests
 *
 * RADICAL SIMPLIFICATION: Playwright creates a fresh browser context for each test.
 * This means IndexedDB is already empty - we don't need to clear anything!
 * The previous approach of clearing storage was causing race conditions.
 */

import { Page, expect } from '@playwright/test';

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
          request.onerror = () => resolve(); // Don't fail, just continue
          request.onblocked = () => resolve(); // Don't fail, just continue
        });
      })
    );
  });
}

/**
 * Handles the landing page flow for new users
 *
 * NOTE: Since tests use /test baseURL, the landing page is never shown.
 * The landing page only appears at the root path ("/").
 * This function is kept for backward compatibility but is now a no-op.
 */
export async function handleLandingPage(page: Page): Promise<void> {
  // Wait for app to be ready
  await page.waitForSelector('#app', { state: 'attached', timeout: 30000 });

  // Tests use /test path which skips the landing page
  // Just wait for password field to be visible
  const passwordField = page.locator('#password');
  await expect(passwordField).toBeVisible({ timeout: 30000 });
}

/**
 * Sets up a fresh test environment with password
 *
 * SIMPLE APPROACH:
 * - Playwright gives us a fresh browser context (empty IndexedDB)
 * - Tests use /test baseURL which skips the landing page
 * - Fresh context goes directly to password setup form
 * - Use click + selectAll + type instead of fill() for more reliable input
 * - Verify field values after filling to catch any issues
 */
export async function setupFreshEnvironment(page: Page, password: string = 'test-password-123'): Promise<void> {
  // Navigate to the app (baseURL is /test, so this goes to /test)
  await page.goto('', { waitUntil: 'domcontentloaded' });

  // Wait for app to be ready
  await page.waitForSelector('#app', { state: 'attached', timeout: 30000 });

  const passwordField = page.locator('#password');
  const confirmField = page.locator('#confirm');

  // /test path skips landing page, goes directly to password setup form
  // Wait for password setup form (confirm field indicates setup mode, not unlock)
  await expect(confirmField).toBeVisible({ timeout: 30000 });
  await expect(passwordField).toBeVisible({ timeout: 5000 });

  // Ensure fields are ready for input
  await expect(passwordField).toBeEnabled({ timeout: 5000 });
  await expect(confirmField).toBeEnabled({ timeout: 5000 });

  // Focus, select all, then type password
  // This is more reliable than fill() which can sometimes append instead of replace
  await passwordField.click();
  await passwordField.press('Control+a');
  await passwordField.pressSequentially(password, { delay: 10 });

  // Move to confirm field
  await confirmField.click();
  await confirmField.press('Control+a');
  await confirmField.pressSequentially(password, { delay: 10 });

  // Verify fields have correct values before submitting
  await expect(passwordField).toHaveValue(password, { timeout: 5000 });
  await expect(confirmField).toHaveValue(password, { timeout: 5000 });

  // Find and click submit button
  const submitButton = page.locator('button[type="submit"]').or(
    page.locator('button').filter({ hasText: /Create Password|Set Password|Unlock/i })
  ).first();

  await expect(submitButton).toBeEnabled({ timeout: 5000 });
  await submitButton.click();

  // Wait for main app to appear (indicates successful setup)
  const appLoaded = page.locator('button').filter({ hasText: /New|^\+$/ })
    .or(page.getByRole('list'))
    .or(page.getByText(/No notes yet|Create your first note/i))
    .or(page.locator('input[type="search"], input[placeholder*="search" i]'));

  await expect(appLoaded.first()).toBeVisible({ timeout: 30000 });
}
