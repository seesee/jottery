import { defineConfig, devices } from '@playwright/test';

// Cross-browser matrix triples wall time and flake surface. Default local
// runs to Chromium only; CI runs the full matrix so merge coverage is
// unchanged. Set FULL_MATRIX=1 locally to reproduce the CI matrix.
const RUN_FULL_MATRIX = !!process.env.CI || !!process.env.FULL_MATRIX;

const allProjects = [
  {
    name: 'chromium',
    use: { ...devices['Desktop Chrome'] },
  },
  {
    name: 'firefox',
    use: { ...devices['Desktop Firefox'] },
  },
  {
    name: 'webkit',
    use: { ...devices['Desktop Safari'] },
  },
];

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 1,
  // 4 workers × N projects = up to 12 browser instances on the full matrix
  workers: process.env.CI ? 1 : 4,
  reporter: 'html',
  use: {
    // Use /test path to skip landing page - each notebook path gets isolated storage
    baseURL: 'http://localhost:5173/test',
    trace: 'on-first-retry',
  },

  projects: RUN_FULL_MATRIX ? allProjects : allProjects.slice(0, 1),

  webServer: [
    {
      command: 'npm run dev',
      url: 'http://localhost:5173',
      reuseExistingServer: !process.env.CI,
      timeout: 120000, // 2 minutes
    },
    {
      command: 'npm run dev',
      cwd: './admin',
      url: 'http://localhost:5174',
      reuseExistingServer: !process.env.CI,
      timeout: 120000, // 2 minutes
    },
  ],
});
