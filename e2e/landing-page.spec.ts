/**
 * Landing page tests
 *
 * The landing page is only shown at the root path "/" for first-time users.
 * Non-root paths (e.g., /test, /recipes) skip the landing page and go
 * directly to the password setup/unlock form.
 */

import { test, expect } from '@playwright/test';

test.describe('Landing Page', () => {
  test('landing page is shown at root path for first-time users', async ({ page }) => {
    // Navigate to the ROOT path explicitly (not /test which is the baseURL)
    await page.goto('http://localhost:5173/', { waitUntil: 'domcontentloaded' });

    // Wait for app to be ready
    await page.waitForSelector('#app', { state: 'attached', timeout: 30000 });

    // Should see the landing page with "Try It Out" button
    const tryItOutButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    await expect(tryItOutButton).toBeVisible({ timeout: 30000 });

    // Should see landing page content (hero section with title)
    await expect(page.getByRole('heading', { name: /Jottery/i })).toBeVisible();
  });

  test('clicking Try It Out shows password setup form', async ({ page }) => {
    // Navigate to the ROOT path explicitly
    await page.goto('http://localhost:5173/', { waitUntil: 'domcontentloaded' });

    // Wait for app to be ready
    await page.waitForSelector('#app', { state: 'attached', timeout: 30000 });

    // Click Try It Out button
    const tryItOutButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    await expect(tryItOutButton).toBeVisible({ timeout: 30000 });
    await tryItOutButton.click();

    // Should see password setup form
    const passwordField = page.locator('#password');
    const confirmField = page.locator('#confirm');

    await expect(passwordField).toBeVisible({ timeout: 10000 });
    await expect(confirmField).toBeVisible({ timeout: 10000 });
  });

  test('non-root path skips landing page and shows password form directly', async ({ page }) => {
    // Navigate to a non-root path (e.g., /test which is the test baseURL)
    await page.goto('http://localhost:5173/test', { waitUntil: 'domcontentloaded' });

    // Wait for app to be ready
    await page.waitForSelector('#app', { state: 'attached', timeout: 30000 });

    // Should NOT see the landing page
    const tryItOutButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();

    // Password form should be visible directly
    const passwordField = page.locator('#password');
    const confirmField = page.locator('#confirm');

    await expect(passwordField).toBeVisible({ timeout: 30000 });
    await expect(confirmField).toBeVisible({ timeout: 10000 });

    // Try It Out button should NOT be visible
    await expect(tryItOutButton).not.toBeVisible({ timeout: 1000 });
  });
});
