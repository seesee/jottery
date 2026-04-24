import { defineConfig, type Plugin } from 'vite';
import { viteSingleFile } from 'vite-plugin-singlefile';
import { resolve } from 'path';

// Determine which entry to build via BUNDLE env var.
// Usage: BUNDLE=preview npm run build  (or calc, outliner)
// Default: build all three sequentially via the "build" script.
const bundle = process.env.BUNDLE || 'preview';

/**
 * Patches a long-standing bug in style-mod (a @codemirror/view transitive
 * dep) where a non-global regex only substitutes the first `&` when
 * expanding nested CSS selectors:
 *
 *   part.replace(/&/, sel)   →   part.replace(/&/g, sel)
 *
 * The bug means a rule like `& > & + &` only gets its first `&` replaced.
 * style-mod 4.1.3 (latest at time of writing) still ships the broken
 * form; CodeQL also flags it as "Incomplete string escaping or encoding".
 * Fixed in the emitted bundle because the webview-bundles are shipped
 * as self-contained HTML.
 */
function fixStyleModGlobalReplace(): Plugin {
  return {
    name: 'fix-style-mod-global-replace',
    enforce: 'post',
    generateBundle(_options, bundle) {
      for (const fileName of Object.keys(bundle)) {
        const file = bundle[fileName];
        if (file.type === 'asset' && fileName.endsWith('.html') && typeof file.source === 'string') {
          file.source = file.source.replace(/\.replace\(\/&\/,/g, '.replace(/&/g,');
        }
      }
    },
  };
}

export default defineConfig({
  plugins: [viteSingleFile(), fixStyleModGlobalReplace()],
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
