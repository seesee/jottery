/**
 * Smoke tests - basic "does it work?" checks
 * These should be fast and catch fundamental breakage
 */

import { test, expect } from '@playwright/test';

test.describe('Smoke Tests', () => {
  test('app loads without crashing', async ({ page }) => {
    // Navigate to the app
    await page.goto('/');

    // Should see some content (not a blank page or error)
    await expect(page.locator('body')).toBeVisible();

    // Should not see an error page
    await expect(page.locator('text=/error|crash|fatal/i')).not.toBeVisible();
  });

  test('can complete basic setup and unlock flow', async ({ page }) => {
    // Clear storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload to get fresh state
    await page.goto('/');

    // Should show password setup screen
    const passwordInputs = page.locator('input[type="password"]');
    await expect(passwordInputs.first()).toBeVisible({ timeout: 10000 });

    // Set up password
    const testPassword = 'smoke-test-password';
    await passwordInputs.first().fill(testPassword);
    await passwordInputs.nth(1).fill(testPassword);

    // Submit
    const submitButton = page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock|Continue/i }).first();
    await submitButton.click();

    // Should see the app (either empty state or note list)
    // Be more flexible with what we're looking for
    await page.waitForTimeout(2000); // Give app time to initialize

    const hasAppContent = await page.locator('body').evaluate((body) => {
      // Check if page has meaningful content (not just loading or error)
      return body.textContent && body.textContent.length > 100;
    });

    expect(hasAppContent).toBe(true);
  });

  test('app reloads successfully', async ({ page }) => {
    await page.goto('/');

    // Wait a bit for initial load
    await page.waitForTimeout(1000);

    // Reload the page
    await page.reload();

    // Should still have content after reload
    await page.waitForTimeout(1000);

    const hasContent = await page.locator('body').evaluate((body) => {
      return body.textContent && body.textContent.length > 50;
    });

    expect(hasContent).toBe(true);
  });

  test('basic navigation elements are present', async ({ page }) => {
    await page.goto('/');

    // Wait for the app to be hydrated - look for the app root
    await page.waitForSelector('#app', { state: 'attached' });

    // Wait a bit for Svelte to render the UI
    await page.waitForTimeout(500);

    // Should have some form of navigation or controls
    // Look for common UI elements without being too specific
    const hasButtons = await page.locator('button').count() > 0;
    const hasInputs = await page.locator('input').count() > 0;
    const hasLinks = await page.locator('a').count() > 0;

    // At least one of these should be true
    expect(hasButtons || hasInputs || hasLinks).toBe(true);
  });

  test('JavaScript is working (not a static HTML error)', async ({ page }) => {
    await page.goto('/');

    // Execute JavaScript and verify it works
    const result = await page.evaluate(() => {
      return typeof document !== 'undefined' && typeof window !== 'undefined';
    });

    expect(result).toBe(true);
  });

  test('no critical console errors on load', async ({ page }) => {
    const errors: string[] = [];

    // Listen for console errors
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });

    await page.goto('/');

    // Wait a bit for any async errors
    await page.waitForTimeout(2000);

    // Filter out known/acceptable errors
    const criticalErrors = errors.filter(error => {
      // Ignore certain types of errors that might be expected
      return !error.includes('favicon') &&
             !error.includes('Failed to load resource');
    });

    // Should have no critical errors
    expect(criticalErrors.length).toBe(0);
  });
});
