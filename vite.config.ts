import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { viteSingleFile } from 'vite-plugin-singlefile';
import { readFileSync, writeFileSync } from 'fs';

// Read version from package.json
const packageJson = JSON.parse(readFileSync('./package.json', 'utf-8'));
const version = packageJson.version;

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
  define: {
    '__APP_VERSION__': JSON.stringify(version),
  },
  plugins: [
    svelte(),
    viteSingleFile(),
    generateVersionFile()
  ],
  server: {
    port: 3000,
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
