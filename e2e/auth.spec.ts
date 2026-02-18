/**
 * E2E tests for authentication flow
 */

import { test, expect } from '@playwright/test';
import { clearAllStorage, handleLandingPage, JotteryPage } from './test-utils';

test.describe('Authentication', () => {
  test.beforeEach(async ({ page }) => {
    // Playwright provides a fresh browser context - just navigate
    await page.goto('', { waitUntil: 'domcontentloaded' });
  });

  test('should show password setup on first visit', async ({ page }) => {
    await handleLandingPage(page);

    // Should see the setup screen
    await expect(page.locator('h1, h2').filter({ hasText: /Set.*Password|Create.*Password|Welcome/i })).toBeVisible();

    // Should have password input
    await expect(page.locator('input[type="password"]').first()).toBeVisible();
  });

  test('should create new password and unlock app', async ({ page }) => {
    const password = 'test-password-123';

    await handleLandingPage(page);

    // Wait for password field to be enabled before interacting
    const passwordField = page.locator('#password');
    const confirmField = page.locator('#confirm');

    await expect(passwordField).toBeEnabled({ timeout: 5000 });
    await expect(confirmField).toBeEnabled({ timeout: 5000 });

    // Enter password
    await passwordField.fill(password);
    await confirmField.fill(password);

    // Submit
    const submitButton = page.locator('button[type="submit"]').or(
      page.locator('button').filter({ hasText: /Create|Set|Unlock|Continue/i })
    ).first();
    await expect(submitButton).toBeEnabled({ timeout: 5000 });
    await submitButton.click();

    // Should see the main app (look for empty state message or note list)
    const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
    await expect(appVisible.first()).toBeVisible({ timeout: 10000 });
  });

  test('should show error on password mismatch during setup', async ({ page }) => {
    await handleLandingPage(page);

    // Wait for password field to be enabled before interacting
    const passwordField = page.locator('#password');
    const confirmField = page.locator('#confirm');

    await expect(passwordField).toBeEnabled({ timeout: 5000 });
    await expect(confirmField).toBeEnabled({ timeout: 5000 });

    // Enter mismatched passwords
    await passwordField.fill('password123');
    await confirmField.fill('different456');

    // Submit
    const submitButton = page.locator('button[type="submit"]').or(
      page.locator('button').filter({ hasText: /Create|Set|Unlock/i })
    ).first();
    await expect(submitButton).toBeEnabled({ timeout: 5000 });
    await submitButton.click();

    // Should see error message
    await expect(page.locator('text=/password.*match|mismatch/i')).toBeVisible({ timeout: 5000 });
  });

  test('should lock and unlock app', async ({ page }) => {
    const password = 'test-password-123';

    await handleLandingPage(page);

    // Wait for password fields to be enabled
    const passwordField = page.locator('#password');
    const confirmField = page.locator('#confirm');
    await expect(passwordField).toBeEnabled({ timeout: 5000 });

    // Setup password
    await passwordField.fill(password);
    if (await confirmField.isVisible()) {
      await confirmField.fill(password);
    }

    const submitButton = page.locator('button[type="submit"]').or(
      page.locator('button').filter({ hasText: /Create|Set|Unlock/i })
    ).first();
    await submitButton.click();

    // Wait for app to load
    const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
    await expect(appVisible.first()).toBeVisible({ timeout: 10000 });

    // Lock the app (look for lock button/menu item)
    const lockButton = page.locator('button, [role="button"]').filter({ hasText: /Lock|Logout/i }).first();
    if (await lockButton.isVisible({ timeout: 2000 }).catch(() => false)) {
      await lockButton.click();

      // Should show unlock screen
      const unlockPasswordField = page.locator('input[type="password"]');
      await expect(unlockPasswordField).toBeVisible({ timeout: 5000 });
      await expect(unlockPasswordField).toBeEnabled({ timeout: 5000 });

      // Unlock with correct password
      await unlockPasswordField.fill(password);
      const unlockButton = page.locator('button[type="submit"]').or(
        page.locator('button').filter({ hasText: /Unlock/i })
      ).first();
      await unlockButton.click();

      // Should see app again
      await expect(appVisible.first()).toBeVisible({ timeout: 10000 });
    }
  });

  test('should remember encryption after page reload', async ({ page }) => {
    const password = 'test-password-123';

    await handleLandingPage(page);

    // Wait for password fields to be enabled
    const passwordField = page.locator('#password');
    const confirmField = page.locator('#confirm');
    await expect(passwordField).toBeEnabled({ timeout: 5000 });

    // Setup password
    await passwordField.fill(password);
    if (await confirmField.isVisible()) {
      await confirmField.fill(password);
    }

    const submitButton = page.locator('button[type="submit"]').or(
      page.locator('button').filter({ hasText: /Create|Set|Unlock/i })
    ).first();
    await submitButton.click();

    // Wait for app to fully load before reloading
    const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
    await expect(appVisible.first()).toBeVisible({ timeout: 10000 });

    // Reload page
    await page.reload();

    // Wait for unlock screen - should show password field (not setup, so no confirm field)
    const unlockPasswordField = page.locator('input[type="password"]');
    await expect(unlockPasswordField).toBeVisible({ timeout: 10000 });
    await expect(unlockPasswordField).toBeEnabled({ timeout: 5000 });

    // Should have only one password field (unlock, not setup)
    const passwordFields = await page.locator('input[type="password"]').count();
    expect(passwordFields).toBeLessThanOrEqual(1);

    // Unlock
    await unlockPasswordField.fill(password);
    const unlockButton = page.locator('button').filter({ hasText: /Unlock/i }).first();
    await unlockButton.click();

    // Should see app again after unlock
    await expect(appVisible.first()).toBeVisible({ timeout: 10000 });
  });

  test('should show error on wrong password', async ({ page }) => {
    const jp = new JotteryPage(page);
    const correctPassword = 'test-password-123';
    const wrongPassword = 'wrong-password';

    await handleLandingPage(page);

    // Wait for password fields to be enabled
    const passwordField = page.locator('#password');
    const confirmField = page.locator('#confirm');
    await expect(passwordField).toBeEnabled({ timeout: 5000 });

    // Setup password
    await passwordField.fill(correctPassword);
    if (await confirmField.isVisible()) {
      await confirmField.fill(correctPassword);
    }

    const submitButton = page.locator('button[type="submit"]').or(
      page.locator('button').filter({ hasText: /Create|Set|Unlock/i })
    ).first();
    await submitButton.click();

    // Wait for app to load
    const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
    await expect(appVisible.first()).toBeVisible({ timeout: 10000 });

    // Create a note (needed for password verification to work)
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();
    await editor.pressSequentially('Test note for password verification');
    await page.waitForTimeout(3000);

    // Now reload to lock
    await page.reload();

    // Wait for unlock screen
    const unlockPasswordField = page.locator('input[type="password"]');
    await expect(unlockPasswordField).toBeVisible({ timeout: 10000 });
    await expect(unlockPasswordField).toBeEnabled({ timeout: 5000 });

    // Try wrong password
    await unlockPasswordField.fill(wrongPassword);
    const unlockButton = page.locator('button').filter({ hasText: /Unlock/i }).first();
    await unlockButton.click();

    // Should see error message in red error box
    await expect(page.locator('.bg-red-50, .dark\\:bg-red-900\\/20').getByText(/incorrect password/i)).toBeVisible({ timeout: 5000 });
  });
});
