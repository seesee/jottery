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
    // Use terser so we can reserve identifier names that collide with
    // HTML entity names. esbuild (the default) happily mints two-letter
    // variables such as `lt`, `gt`, `amp` during minification, producing
    // substrings like `&&lt&&` inside inlined <script> blocks that look
    // like malformed HTML entity references when the HTML is reviewed
    // (even though the script-data parsing state runs them correctly at
    // runtime).
    minify: 'terser',
    terserOptions: {
      mangle: {
        reserved: ['lt', 'gt', 'amp', 'quot', 'apos'],
      },
    },
    rollupOptions: {
      input: resolve(__dirname, `${bundle}.html`),
      output: {
        inlineDynamicImports: true,
      },
    },
  },
});
