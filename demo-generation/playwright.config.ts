import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for screenshot generation
 * This is separate from the main e2e test config
 */
export default defineConfig({
  testDir: './playwright',
  fullyParallel: true, // Run tests in parallel
  forbidOnly: !!process.env.CI,
  retries: 0, // No retries for screenshot generation
  workers: 4, // 4 parallel workers for faster generation
  reporter: 'list',
  timeout: 35000, // 35 seconds per test - kill if hanging
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'off', // Disable trace for faster runs
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
    timeout: 60000, // 60s to start dev server
  },
});
