import { defineConfig, Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'fs';

// Read version from package.json (works both from root and admin directory)
const packageJson = JSON.parse(readFileSync('./package.json', 'utf-8'));
const version = packageJson.version;

// Plugin to serve the SPA at /user path in dev mode (for E2E tests)
function serveUserPortal(): Plugin {
  return {
    name: 'serve-user-portal',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        // Rewrite /user requests to /admin/ so the SPA is served
        if (req.url?.startsWith('/user')) {
          req.url = req.url.replace('/user', '/admin');
        }
        next();
      });
    },
  };
}

export default defineConfig({
  define: {
    '__APP_VERSION__': JSON.stringify(version),
  },
  plugins: [svelte(), serveUserPortal()],
  base: '/admin/',
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
});
