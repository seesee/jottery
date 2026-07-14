import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  timeout: 30_000,
  use: {
    deviceScaleFactor: 2,
    // viewport is set per-test from screens.json
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
