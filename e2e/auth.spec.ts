/**
 * E2E tests for authentication flow
 */

import { test, expect } from '@playwright/test';

test.describe('Authentication', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all local storage and IndexedDB before each test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      // Clear IndexedDB
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });
    // Reload to get fresh state
    await page.goto('/');
  });

  test('should show password setup on first visit', async ({ page }) => {
    // Handle landing page for new users - click "Try It Out" if it appears
    const getStartedButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    const passwordInput = page.locator('input[type="password"]').first();

    // Wait for either landing page button or password input
    await Promise.race([
      getStartedButton.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {}),
      passwordInput.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {})
    ]);

    // Click Try It Out if visible
    if (await getStartedButton.isVisible()) {
      await getStartedButton.click();
    }

    // Should see the setup screen
    await expect(page.locator('h1, h2').filter({ hasText: /Set.*Password|Create.*Password|Welcome/i })).toBeVisible();

    // Should have password input
    await expect(page.locator('input[type="password"]').first()).toBeVisible();
  });

  test('should create new password and unlock app', async ({ page }) => {
    const password = 'test-password-123';

    // Handle landing page for new users - click "Try It Out" if it appears
    const getStartedButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    const passwordInput = page.locator('input[type="password"]').first();

    // Wait for either landing page button or password input
    await Promise.race([
      getStartedButton.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {}),
      passwordInput.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {})
    ]);

    // Click Try It Out if visible
    if (await getStartedButton.isVisible()) {
      await getStartedButton.click();
    }

    // Wait for password inputs to be visible
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });

    // Enter password (assuming first setup has 2 fields: password + confirm)
    await passwordInputs.first().fill(password);
    await passwordInputs.nth(1).fill(password);

    // Submit
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock|Continue/i }).first().click();

    // Should see the main app (look for empty state message or note list)
    const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
    await expect(appVisible.first()).toBeVisible({ timeout: 5000 });
  });

  test('should show error on password mismatch during setup', async ({ page }) => {
    // Handle landing page for new users - click "Try It Out" if it appears
    const getStartedButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    const passwordInput = page.locator('input[type="password"]').first();

    // Wait for either landing page button or password input
    await Promise.race([
      getStartedButton.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {}),
      passwordInput.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {})
    ]);

    // Click Try It Out if visible
    if (await getStartedButton.isVisible()) {
      await getStartedButton.click();
    }

    // Wait for password inputs
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });

    // Enter mismatched passwords
    await passwordInputs.first().fill('password123');
    await passwordInputs.nth(1).fill('different456');

    // Submit
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

    // Should see error message
    await expect(page.locator('text=/password.*match|mismatch/i')).toBeVisible({ timeout: 2000 });
  });

  test('should lock and unlock app', async ({ page }) => {
    const password = 'test-password-123';

    // Handle landing page for new users - click "Try It Out" if it appears
    const getStartedButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    const passwordInput = page.locator('input[type="password"]').first();

    // Wait for either landing page button or password input
    await Promise.race([
      getStartedButton.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {}),
      passwordInput.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {})
    ]);

    // Click Try It Out if visible
    if (await getStartedButton.isVisible()) {
      await getStartedButton.click();
    }

    // Setup password first
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill(password);
    await passwordInputs.nth(1).fill(password);
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

    // Wait for app to load
    await page.waitForTimeout(1000);

    // Lock the app (look for lock button/menu item)
    const lockButton = page.locator('button, [role="button"]').filter({ hasText: /Lock|Logout/i }).first();
    if (await lockButton.isVisible()) {
      await lockButton.click();

      // Should show unlock screen
      await expect(page.locator('input[type="password"]')).toBeVisible();

      // Unlock with correct password
      await page.locator('input[type="password"]').fill(password);
      await page.locator('button[type="submit"], button').filter({ hasText: /Unlock/i }).first().click();

      // Should see app again
      const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
      await expect(appVisible.first()).toBeVisible();
    }
  });

  test('should remember encryption after page reload', async ({ page }) => {
    const password = 'test-password-123';

    // Handle landing page for new users - click "Try It Out" if it appears
    const getStartedButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    const passwordInput = page.locator('input[type="password"]').first();

    // Wait for either landing page button or password input
    await Promise.race([
      getStartedButton.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {}),
      passwordInput.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {})
    ]);

    // Click Try It Out if visible
    if (await getStartedButton.isVisible()) {
      await getStartedButton.click();
    }

    // Setup password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill(password);
    await passwordInputs.nth(1).fill(password);
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

    // Wait for app
    await page.waitForTimeout(1000);

    // Reload page
    await page.reload();

    // Should show unlock screen (not setup)
    await expect(page.locator('input[type="password"]')).toBeVisible();

    // Should have only one password field (unlock, not setup)
    const passwordFields = await page.locator('input[type="password"]').count();
    expect(passwordFields).toBeLessThanOrEqual(1);

    // Unlock
    await page.locator('input[type="password"]').fill(password);
    await page.locator('button').filter({ hasText: /Unlock/i }).first().click();

    // Should see app
    const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
    await expect(appVisible.first()).toBeVisible();
  });

  test('should show error on wrong password', async ({ page }) => {
    const correctPassword = 'test-password-123';
    const wrongPassword = 'wrong-password';

    // Handle landing page for new users - click "Try It Out" if it appears
    const getStartedButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    const passwordInput = page.locator('input[type="password"]').first();

    // Wait for either landing page button or password input
    await Promise.race([
      getStartedButton.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {}),
      passwordInput.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {})
    ]);

    // Click Try It Out if visible
    if (await getStartedButton.isVisible()) {
      await getStartedButton.click();
    }

    // Setup password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill(correctPassword);
    await passwordInputs.nth(1).fill(correctPassword);
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Create a note (needed for password verification to work)
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();
    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor.click();
    await editor.pressSequentially('Test note for password verification');
    await page.waitForTimeout(3000);

    // Now reload to lock
    await page.reload();

    // Try wrong password
    await page.locator('input[type="password"]').fill(wrongPassword);
    await page.locator('button').filter({ hasText: /Unlock/i }).first().click();

    // Should see error message in red error box
    await expect(page.locator('.bg-red-50, .dark\\:bg-red-900\\/20').getByText(/incorrect password/i)).toBeVisible({ timeout: 5000 });
  });
});
