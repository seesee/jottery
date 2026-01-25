/**
 * Shared test utilities for e2e tests
 */

import { Page, expect } from '@playwright/test';

/**
 * Clears all browser storage (localStorage, sessionStorage, IndexedDB)
 * and properly waits for IndexedDB operations to complete
 */
export async function clearAllStorage(page: Page): Promise<void> {
  await page.evaluate(async () => {
    localStorage.clear();
    sessionStorage.clear();

    // Properly await IndexedDB clearing
    const databases = await indexedDB.databases();
    await Promise.all(
      databases.map((db) => {
        return new Promise<void>((resolve, reject) => {
          if (!db.name) {
            resolve();
            return;
          }
          const request = indexedDB.deleteDatabase(db.name);
          request.onsuccess = () => resolve();
          request.onerror = () => reject(request.error);
          request.onblocked = () => {
            // Database is blocked, but we can still proceed
            resolve();
          };
        });
      })
    );
  });
}

/**
 * Handles the landing page flow for new users
 * Clicks "Try It Out" if the landing page is shown
 */
export async function handleLandingPage(page: Page): Promise<void> {
  const getStartedButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
  const passwordInput = page.locator('input[type="password"]').first();

  // Wait for either landing page button or password input (longer timeout for parallel runs)
  // Use a proper race that tells us which one resolved
  const result = await Promise.race([
    getStartedButton.waitFor({ state: 'visible', timeout: 15000 }).then(() => 'landing' as const).catch(() => null),
    passwordInput.waitFor({ state: 'visible', timeout: 15000 }).then(() => 'password' as const).catch(() => null)
  ]);

  // If landing page is showing, click the button
  if (result === 'landing' || await getStartedButton.isVisible()) {
    await getStartedButton.click();
    // Wait for password input to appear after clicking
    await passwordInput.waitFor({ state: 'visible', timeout: 10000 });
  }
}

/**
 * Sets up a fresh test environment with password
 */
export async function setupFreshEnvironment(page: Page, password: string = 'test-password-123'): Promise<void> {
  // Clear storage
  await page.goto('/');
  await clearAllStorage(page);

  // Navigate fresh
  await page.goto('/');

  // Handle landing page
  await handleLandingPage(page);

  // Set up password (with longer timeout for parallel test runs)
  const passwordInputs = page.locator('input[type="password"]');
  await passwordInputs.first().waitFor({ state: 'visible', timeout: 15000 });
  await passwordInputs.first().fill(password);
  await passwordInputs.nth(1).fill(password);
  await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

  // Wait for app to load
  const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
  await expect(appVisible.first()).toBeVisible({ timeout: 15000 });
}
