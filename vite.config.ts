import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { viteSingleFile } from 'vite-plugin-singlefile';
import { visualizer } from 'rollup-plugin-visualizer';
import basicSsl from '@vitejs/plugin-basic-ssl';
import { readFileSync, writeFileSync } from 'fs';

// Read version from package.json
const packageJson = JSON.parse(readFileSync('./package.json', 'utf-8'));
const version = packageJson.version;

// Mobile testing configuration (via environment variables)
// Usage: VITE_HTTPS=true VITE_HOST=true VITE_ALLOWED_HOSTS=entrapta,other npm run dev
const enableHttps = process.env.VITE_HTTPS === 'true';
const enableHost = process.env.VITE_HOST === 'true';
const allowedHosts = process.env.VITE_ALLOWED_HOSTS?.split(',').filter(Boolean) || [];

// Plugin to generate version.json during build
function generateVersionFile() {
  return {
    name: 'generate-version-file',
    writeBundle() {
      const versionData = {
        version,
        buildTime: new Date().toISOString(),
        buildHash: Date.now().toString(36), // Simple hash based on build time
      };
      writeFileSync('dist/version.json', JSON.stringify(versionData, null, 2));
      console.log('Generated version.json:', versionData);
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  // Use relative paths for compatibility with both web server and Electron (file:// protocol)
  base: './',
  define: {
    '__APP_VERSION__': JSON.stringify(version),
  },
  plugins: [
    svelte(),
    viteSingleFile(),
    generateVersionFile(),
    // Only enable HTTPS when VITE_HTTPS=true (needed for crypto.subtle on mobile)
    ...(enableHttps ? [basicSsl()] : []),
    visualizer({
      filename: 'dist/stats.html',
      open: false,
      gzipSize: true,
      brotliSize: true,
    })
  ],
  server: {
    port: 5173,
    // Enable network access when VITE_HOST=true
    ...(enableHost && { host: true }),
    // Allow specific hostnames when VITE_ALLOWED_HOSTS is set
    ...(allowedHosts.length > 0 && { allowedHosts }),
  },
  build: {
    target: 'esnext',
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false,
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        inlineDynamicImports: true,
      }
    }
  },
});
