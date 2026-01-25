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
  // Playwright's click() automatically waits for visibility
  // If button doesn't exist (already past landing page), catch and continue
  try {
    await page.locator('button', { hasText: 'Try It Out' }).first().click({ timeout: 5000 });
  } catch {
    // Button not present - we're already past the landing page
  }
}

/**
 * Sets up a fresh test environment with password
 */
export async function setupFreshEnvironment(page: Page, password: string = 'test-password-123'): Promise<void> {
  // Clear storage then reload to get fresh state
  await page.goto('/');
  await clearAllStorage(page);
  await page.reload();

  // Handle landing page if shown
  await handleLandingPage(page);

  // Set up password using id-based locators for reliability
  await page.locator('#password').fill(password);
  await page.locator('#confirm').fill(password);
  await page.locator('button', { hasText: /Create Password|Set Password|Unlock/i }).click();

  // Wait for app to load
  await expect(page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list')).first()).toBeVisible();
}
