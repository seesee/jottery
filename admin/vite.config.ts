import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'fs';

// Read version from package.json (works both from root and admin directory)
const packageJson = JSON.parse(readFileSync('./package.json', 'utf-8'));
const version = packageJson.version;

export default defineConfig(({ command }) => ({
  define: {
    '__APP_VERSION__': JSON.stringify(version),
  },
  plugins: [svelte()],
  // Only use /admin/ base in production build - in dev mode serve from root
  // This allows E2E tests to access both /admin and /user paths
  base: command === 'build' ? '/admin/' : '/',
  server: {
    port: 5174,
    proxy: {
      '/api': {
        target: 'http://localhost:3030',
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  test: {
    globals: true,
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
  },
}));
