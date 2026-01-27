/**
 * E2E tests for user self-service portal
 *
 * These tests use Playwright route mocking to simulate server responses,
 * allowing tests to run without a live server.
 */

import { test, expect, Page, Route } from '@playwright/test';

// Mock user data
const mockUser = {
  id: 'user-123',
  email: 'test@example.com',
  isAdmin: false,
};

const mockAccountInfo = {
  email: 'test@example.com',
  noteCount: 42,
  attachmentCount: 7,
  storageUsedBytes: 1048576, // 1 MB
  storageQuotaMb: 100,
  createdAt: '2024-01-15T10:30:00Z',
  lastSyncAt: '2025-01-09T15:00:00Z',
};

// Helper to set up API mocks
async function setupApiMocks(page: Page, options: {
  loginSuccess?: boolean;
  accountInfo?: typeof mockAccountInfo | null;
  changePasswordSuccess?: boolean;
  deactivateSuccess?: boolean;
  deleteSuccess?: boolean;
} = {}) {
  const {
    loginSuccess = true,
    accountInfo = mockAccountInfo,
    changePasswordSuccess = true,
    deactivateSuccess = true,
    deleteSuccess = true,
  } = options;

  // Mock login endpoint
  await page.route('**/api/v1/user/login', async (route: Route) => {
    if (loginSuccess) {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          sessionId: 'mock-session-token',
          expiresAt: '2025-01-16T00:00:00Z',
          user: mockUser,
        }),
      });
    } else {
      await route.fulfill({
        status: 401,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'Invalid email or password' }),
      });
    }
  });

  // Mock account info endpoint
  await page.route('**/api/v1/user/account', async (route: Route) => {
    const method = route.request().method();

    if (method === 'GET') {
      if (accountInfo) {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(accountInfo),
        });
      } else {
        await route.fulfill({
          status: 401,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'Unauthorized' }),
        });
      }
    } else if (method === 'DELETE') {
      const url = route.request().url();
      if (url.includes('mode=deactivate')) {
        if (deactivateSuccess) {
          await route.fulfill({ status: 204 });
        } else {
          await route.fulfill({
            status: 500,
            contentType: 'application/json',
            body: JSON.stringify({ error: 'Failed to deactivate' }),
          });
        }
      } else if (url.includes('mode=delete')) {
        if (deleteSuccess) {
          await route.fulfill({ status: 204 });
        } else {
          await route.fulfill({
            status: 500,
            contentType: 'application/json',
            body: JSON.stringify({ error: 'Failed to delete' }),
          });
        }
      }
    }
  });

  // Mock change password endpoint
  await page.route('**/api/v1/user/change-password', async (route: Route) => {
    if (changePasswordSuccess) {
      await route.fulfill({ status: 204 });
    } else {
      await route.fulfill({
        status: 401,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'Current password is incorrect' }),
      });
    }
  });

  // Mock logout endpoint
  await page.route('**/api/v1/user/logout', async (route: Route) => {
    await route.fulfill({ status: 204 });
  });
}

// Use admin dev server port for user portal tests (5174)
// Can be overridden with ADMIN_BASE_URL env var
test.use({ baseURL: process.env.ADMIN_BASE_URL || 'http://localhost:5174' });

test.describe('User Portal', () => {
  test.beforeEach(async ({ page }) => {
    // Clear localStorage before each test
    await page.goto('/user');
    await page.evaluate(() => {
      localStorage.clear();
    });
  });

  test.describe('Login', () => {
    test('should display login page at /user', async ({ page }) => {
      await setupApiMocks(page);
      await page.goto('/user');

      // Should see login form
      await expect(page.locator('h1, h2').filter({ hasText: /Jottery|Sign in/i })).toBeVisible();
      await expect(page.locator('input[type="email"], input[id*="email"]')).toBeVisible();
      await expect(page.locator('input[type="password"]')).toBeVisible();
      await expect(page.locator('button').filter({ hasText: /Sign in/i })).toBeVisible();
    });

    test('should login successfully with valid credentials', async ({ page }) => {
      await setupApiMocks(page, { loginSuccess: true });
      await page.goto('/user');

      // Fill login form
      await page.locator('input[type="email"], input[id*="email"]').fill('test@example.com');
      await page.locator('input[type="password"]').fill('password123456');

      // Submit
      await page.locator('button').filter({ hasText: /Sign in/i }).click();

      // Should see account page after login
      await expect(page.locator('text=/Account|My Account/i').first()).toBeVisible({ timeout: 5000 });
    });

    test('should show error message on invalid credentials', async ({ page }) => {
      await setupApiMocks(page, { loginSuccess: false });
      await page.goto('/user');

      // Fill login form with wrong credentials
      await page.locator('input[type="email"], input[id*="email"]').fill('test@example.com');
      await page.locator('input[type="password"]').fill('wrongpassword');

      // Submit
      await page.locator('button').filter({ hasText: /Sign in/i }).click();

      // Should see error message
      await expect(page.locator('text=/Invalid email or password/i')).toBeVisible({ timeout: 5000 });
    });
  });

  test.describe('Account Information', () => {
    test.beforeEach(async ({ page }) => {
      await setupApiMocks(page);
      // Simulate logged in state
      await page.goto('/user');
      await page.evaluate(() => {
        localStorage.setItem('user_session_token', 'mock-session-token');
      });
    });

    test('should display account information', async ({ page }) => {
      await page.goto('/user');

      // Should see account details
      await expect(page.locator('text=test@example.com')).toBeVisible({ timeout: 5000 });
      await expect(page.locator('text=/42|Notes/i').first()).toBeVisible();
      await expect(page.locator('text=/Storage|1/i').first()).toBeVisible();
    });
  });

  test.describe('Change Password', () => {
    test.beforeEach(async ({ page }) => {
      await setupApiMocks(page);
      await page.goto('/user');
      await page.evaluate(() => {
        localStorage.setItem('user_session_token', 'mock-session-token');
      });
    });

    test('should change password successfully', async ({ page }) => {
      await page.goto('/user');

      // Navigate to settings tab if present
      const settingsTab = page.locator('button, a').filter({ hasText: /Settings/i }).first();
      if (await settingsTab.isVisible()) {
        await settingsTab.click();
      }

      // Fill password change form
      const currentPasswordInput = page.locator('input[id*="current"], input[placeholder*="current"]').first();
      const newPasswordInput = page.locator('input[id*="new"], input[placeholder*="new"]').first();
      const confirmPasswordInput = page.locator('input[id*="confirm"], input[placeholder*="confirm"]').first();

      await currentPasswordInput.fill('currentpassword');
      await newPasswordInput.fill('newpassword123');
      await confirmPasswordInput.fill('newpassword123');

      // Submit
      await page.locator('button').filter({ hasText: /Update Password|Change Password/i }).click();

      // Should see success message
      await expect(page.locator('text=/successfully|changed/i').first()).toBeVisible({ timeout: 5000 });
    });

    test('should show error when passwords do not match', async ({ page }) => {
      await page.goto('/user');

      // Navigate to settings tab if present
      const settingsTab = page.locator('button, a').filter({ hasText: /Settings/i }).first();
      if (await settingsTab.isVisible()) {
        await settingsTab.click();
      }

      // Fill password change form with mismatched passwords
      const currentPasswordInput = page.locator('input[id*="current"], input[placeholder*="current"]').first();
      const newPasswordInput = page.locator('input[id*="new"], input[placeholder*="new"]').first();
      const confirmPasswordInput = page.locator('input[id*="confirm"], input[placeholder*="confirm"]').first();

      await currentPasswordInput.fill('currentpassword');
      await newPasswordInput.fill('newpassword123');
      await confirmPasswordInput.fill('differentpassword');

      // Submit
      await page.locator('button').filter({ hasText: /Update Password|Change Password/i }).click();

      // Should see error message
      await expect(page.locator('text=/match|mismatch/i').first()).toBeVisible({ timeout: 5000 });
    });

    test('should show error when new password is too short', async ({ page }) => {
      await page.goto('/user');

      // Navigate to settings tab if present
      const settingsTab = page.locator('button, a').filter({ hasText: /Settings/i }).first();
      if (await settingsTab.isVisible()) {
        await settingsTab.click();
      }

      // Fill with short password
      const currentPasswordInput = page.locator('input[id*="current"], input[placeholder*="current"]').first();
      const newPasswordInput = page.locator('input[id*="new"], input[placeholder*="new"]').first();
      const confirmPasswordInput = page.locator('input[id*="confirm"], input[placeholder*="confirm"]').first();

      await currentPasswordInput.fill('currentpassword');
      await newPasswordInput.fill('short');
      await confirmPasswordInput.fill('short');

      // Submit
      await page.locator('button').filter({ hasText: /Update Password|Change Password/i }).click();

      // Should see error about minimum length
      await expect(page.locator('text=/12 characters|too short/i').first()).toBeVisible({ timeout: 5000 });
    });
  });

  test.describe('Delete Notes', () => {
    test.beforeEach(async ({ page }) => {
      await setupApiMocks(page);
      await page.goto('/user');
      await page.evaluate(() => {
        localStorage.setItem('user_session_token', 'mock-session-token');
      });
    });

    test('should show delete notes confirmation modal', async ({ page }) => {
      await page.goto('/user');

      // Navigate to settings tab if present
      const settingsTab = page.locator('button, a').filter({ hasText: /Settings/i }).first();
      if (await settingsTab.isVisible()) {
        await settingsTab.click();
      }

      // Click delete notes button in the danger zone
      await page.locator('button').filter({ hasText: /Delete Notes/i }).first().click();

      // Should see confirmation modal
      await expect(page.locator('.fixed, [role="dialog"]').locator('text=/delete.*notes|Are you sure/i').first()).toBeVisible({ timeout: 5000 });

      // Should have confirm and cancel buttons
      await expect(page.locator('.fixed button, [role="dialog"] button').filter({ hasText: /Delete|Confirm/i })).toBeVisible();
      await expect(page.locator('.fixed button, [role="dialog"] button').filter({ hasText: /Cancel/i })).toBeVisible();
    });
  });

  test.describe('Account Deletion', () => {
    test.beforeEach(async ({ page }) => {
      await setupApiMocks(page);
      await page.goto('/user');
      await page.evaluate(() => {
        localStorage.setItem('user_session_token', 'mock-session-token');
      });
    });

    test('should show delete confirmation modal with DELETE input', async ({ page }) => {
      await page.goto('/user');

      // Wait for page to load
      await page.waitForLoadState('networkidle');

      // Navigate to settings tab if present
      const settingsTab = page.locator('button, a').filter({ hasText: /Settings/i }).first();
      if (await settingsTab.isVisible({ timeout: 3000 }).catch(() => false)) {
        await settingsTab.click();
        await page.waitForTimeout(300);
      }

      // Click delete button in the danger zone - wait for it to be visible first
      const deleteButton = page.locator('button').filter({ hasText: /^Delete$/i }).first();
      await expect(deleteButton).toBeVisible({ timeout: 10000 });
      await deleteButton.click();

      // Should see confirmation modal with input
      await expect(page.locator('.fixed, [role="dialog"]').locator('text=/DELETE|permanently/i').first()).toBeVisible({ timeout: 5000 });
      await expect(page.locator('.fixed input, [role="dialog"] input').first()).toBeVisible();
    });

    test('should not allow deletion without typing DELETE', async ({ page }) => {
      await page.goto('/user');

      // Wait for page to load
      await page.waitForLoadState('networkidle');

      // Navigate to settings tab if present
      const settingsTab = page.locator('button, a').filter({ hasText: /Settings/i }).first();
      if (await settingsTab.isVisible({ timeout: 3000 }).catch(() => false)) {
        await settingsTab.click();
        await page.waitForTimeout(300);
      }

      // Click delete button - wait for it to be visible first
      const deleteButton = page.locator('button').filter({ hasText: /^Delete$/i }).first();
      await expect(deleteButton).toBeVisible({ timeout: 10000 });
      await deleteButton.click();

      // Wait for modal
      await page.waitForTimeout(500);

      // Find the delete forever button in the modal
      const confirmDeleteButton = page.locator('.fixed button, [role="dialog"] button').filter({ hasText: /Delete Forever/i });

      // Button should be disabled when DELETE is not typed
      await expect(confirmDeleteButton).toBeDisabled();
    });
  });

  test.describe('Logout', () => {
    test.beforeEach(async ({ page }) => {
      await setupApiMocks(page);
      await page.goto('/user');
      await page.evaluate(() => {
        localStorage.setItem('user_session_token', 'mock-session-token');
      });
    });

    test('should logout successfully', async ({ page }) => {
      await page.goto('/user');

      // Find and click logout button (in nav or header)
      const logoutButton = page.locator('nav button, header button, aside button').filter({ hasText: /Log ?out/i }).first();
      await logoutButton.click();

      // Wait for confirmation modal to appear
      await page.waitForTimeout(500);

      // Click confirm button in the modal (the modal has higher z-index)
      const confirmButton = page.locator('.fixed button, [role="dialog"] button').filter({ hasText: /Log ?out/i });
      if (await confirmButton.isVisible({ timeout: 2000 }).catch(() => false)) {
        await confirmButton.click();
      }

      // Should see login page
      await expect(page.locator('input[type="email"], input[id*="email"]')).toBeVisible({ timeout: 10000 });
    });
  });
});
