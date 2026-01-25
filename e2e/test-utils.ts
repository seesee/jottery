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
 * Clicks "Try It Out" if the landing page is shown and ensures password form is ready
 */
export async function handleLandingPage(page: Page): Promise<void> {
  const tryItOutButton = page.locator('button', { hasText: 'Try It Out' }).first();
  const passwordField = page.locator('#password');

  // Wait for page to reach a known state (either landing page or password form)
  const landingOrPassword = tryItOutButton.or(passwordField);
  await expect(landingOrPassword.first()).toBeVisible({ timeout: 15000 });

  // Check if we're on landing page
  const isLandingPage = await tryItOutButton.isVisible().catch(() => false);
  if (isLandingPage) {
    await tryItOutButton.click();
    // Wait for password field after clicking
    await expect(passwordField).toBeVisible({ timeout: 10000 });
  }
}

/**
 * Sets up a fresh test environment with password
 * This is the main entry point for test setup - handles all state transitions robustly
 */
export async function setupFreshEnvironment(page: Page, password: string = 'test-password-123'): Promise<void> {
  // Clear storage then reload to get fresh state
  await page.goto('/');
  await clearAllStorage(page);
  await page.reload();

  const tryItOutButton = page.locator('button', { hasText: 'Try It Out' }).first();
  const passwordField = page.locator('#password');
  const confirmField = page.locator('#confirm');

  // Step 1: Wait for page to reach a known state (either landing page or password form)
  const landingOrPassword = tryItOutButton.or(passwordField);
  await expect(landingOrPassword.first()).toBeVisible({ timeout: 15000 });

  // Step 2: If on landing page, click through to password form
  const isLandingPage = await tryItOutButton.isVisible().catch(() => false);
  if (isLandingPage) {
    await tryItOutButton.click();
    // Wait for password field after clicking
    await expect(passwordField).toBeVisible({ timeout: 10000 });
  }

  // Step 3: Fill in the password form
  await passwordField.fill(password);

  // Confirm field only exists on setup, not unlock
  if (await confirmField.isVisible()) {
    await confirmField.fill(password);
  }

  await page.locator('button', { hasText: /Create Password|Set Password|Unlock/i }).click();

  // Step 4: Wait for app to load
  const appLoaded = page.getByText(/No notes yet|Create your first note/i)
    .or(page.getByRole('list'))
    .or(page.locator('button').filter({ hasText: /New|^\+$/ }));
  await expect(appLoaded.first()).toBeVisible({ timeout: 10000 });
}
