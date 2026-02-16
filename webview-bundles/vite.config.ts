import { defineConfig } from 'vite';
import { viteSingleFile } from 'vite-plugin-singlefile';
import { resolve } from 'path';

// Determine which entry to build via BUNDLE env var.
// Usage: BUNDLE=preview npm run build  (or calc, outliner)
// Default: build all three sequentially via the "build" script.
const bundle = process.env.BUNDLE || 'preview';

export default defineConfig({
  plugins: [viteSingleFile()],
  build: {
    target: 'esnext',
    outDir: 'dist',
    emptyOutDir: false,
    sourcemap: false,
    cssCodeSplit: false,
    rollupOptions: {
      input: resolve(__dirname, `${bundle}.html`),
      output: {
        inlineDynamicImports: true,
      },
    },
  },
});
