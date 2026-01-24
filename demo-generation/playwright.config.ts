import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for screenshot generation
 * This is separate from the main e2e test config
 */
export default defineConfig({
  testDir: './playwright',
  fullyParallel: false, // Run serially for screenshot generation
  forbidOnly: !!process.env.CI,
  retries: 0, // No retries for screenshot generation
  workers: 1, // Single worker for consistency
  reporter: 'list',
  timeout: 120000, // 2 minutes per test
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
  },
});
