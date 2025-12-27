/**
 * Dynamic syntax highlighter using highlight.js with on-demand language loading
 */

import hljs from 'highlight.js/lib/core';

// Import core languages at build time
import javascript from 'highlight.js/lib/languages/javascript';
import typescript from 'highlight.js/lib/languages/typescript';
import python from 'highlight.js/lib/languages/python';
import markdown from 'highlight.js/lib/languages/markdown';
import json from 'highlight.js/lib/languages/json';
import xml from 'highlight.js/lib/languages/xml'; // includes HTML
import css from 'highlight.js/lib/languages/css';
import bash from 'highlight.js/lib/languages/bash';
import sql from 'highlight.js/lib/languages/sql';

// Register core languages immediately
hljs.registerLanguage('javascript', javascript);
hljs.registerLanguage('typescript', typescript);
hljs.registerLanguage('python', python);
hljs.registerLanguage('markdown', markdown);
hljs.registerLanguage('json', json);
hljs.registerLanguage('xml', xml);
hljs.registerLanguage('html', xml); // HTML uses XML parser
hljs.registerLanguage('css', css);
hljs.registerLanguage('bash', bash);
hljs.registerLanguage('sh', bash); // sh is an alias for bash
hljs.registerLanguage('shell', bash); // shell is an alias for bash
hljs.registerLanguage('sql', sql);

// Cache for dynamically loaded languages
const loadedLanguages = new Set<string>([
  'javascript',
  'typescript',
  'python',
  'markdown',
  'json',
  'xml',
  'html',
  'css',
  'bash',
  'sh',
  'shell',
  'sql',
]);

// In-flight loading promises to avoid duplicate loads
const loadingPromises = new Map<string, Promise<void>>();

/**
 * Dynamically import a language definition
 */
async function importLanguage(languageId: string): Promise<any> {
  // Map language IDs to their import paths
  const languageImports: Record<string, () => Promise<any>> = {
    // Popular languages
    java: () => import('highlight.js/lib/languages/java'),
    csharp: () => import('highlight.js/lib/languages/csharp'),
    php: () => import('highlight.js/lib/languages/php'),
    ruby: () => import('highlight.js/lib/languages/ruby'),
    go: () => import('highlight.js/lib/languages/go'),
    rust: () => import('highlight.js/lib/languages/rust'),
    swift: () => import('highlight.js/lib/languages/swift'),
    kotlin: () => import('highlight.js/lib/languages/kotlin'),
    scala: () => import('highlight.js/lib/languages/scala'),

    // Web languages
    scss: () => import('highlight.js/lib/languages/scss'),
    sass: () => import('highlight.js/lib/languages/scss'), // sass uses scss
    less: () => import('highlight.js/lib/languages/less'),
    stylus: () => import('highlight.js/lib/languages/stylus'),
    handlebars: () => import('highlight.js/lib/languages/handlebars'),

    // Systems languages
    c: () => import('highlight.js/lib/languages/c'),
    cpp: () => import('highlight.js/lib/languages/cpp'),
    objectivec: () => import('highlight.js/lib/languages/objectivec'),

    // Data languages
    yaml: () => import('highlight.js/lib/languages/yaml'),
    toml: () => import('highlight.js/lib/languages/ini'), // toml uses ini
    ini: () => import('highlight.js/lib/languages/ini'),
    properties: () => import('highlight.js/lib/languages/properties'),
    diff: () => import('highlight.js/lib/languages/diff'),
    dockerfile: () => import('highlight.js/lib/languages/dockerfile'),

    // Other languages
    perl: () => import('highlight.js/lib/languages/perl'),
    r: () => import('highlight.js/lib/languages/r'),
    matlab: () => import('highlight.js/lib/languages/matlab'),
    lua: () => import('highlight.js/lib/languages/lua'),
    vim: () => import('highlight.js/lib/languages/vim'),
    powershell: () => import('highlight.js/lib/languages/powershell'),
    graphql: () => import('highlight.js/lib/languages/graphql'),
    elixir: () => import('highlight.js/lib/languages/elixir'),
    erlang: () => import('highlight.js/lib/languages/erlang'),
    haskell: () => import('highlight.js/lib/languages/haskell'),
    clojure: () => import('highlight.js/lib/languages/clojure'),
  };

  const importFn = languageImports[languageId];
  if (!importFn) {
    throw new Error(`Unknown language: ${languageId}`);
  }

  const module = await importFn();
  return module.default;
}

/**
 * Ensure a language is loaded and registered
 */
export async function ensureLanguageLoaded(languageId: string): Promise<boolean> {
  const normalized = languageId.toLowerCase();

  // Already loaded
  if (loadedLanguages.has(normalized)) {
    return true;
  }

  // Currently loading - wait for it
  if (loadingPromises.has(normalized)) {
    await loadingPromises.get(normalized);
    return loadedLanguages.has(normalized);
  }

  // Start loading
  const loadingPromise = (async () => {
    try {
      const langDef = await importLanguage(normalized);
      hljs.registerLanguage(normalized, langDef);
      loadedLanguages.add(normalized);
      console.log(`[SyntaxHighlighter] Loaded language: ${normalized}`);
    } catch (error) {
      console.warn(`[SyntaxHighlighter] Failed to load language ${normalized}:`, error);
    } finally {
      loadingPromises.delete(normalized);
    }
  })();

  loadingPromises.set(normalized, loadingPromise);
  await loadingPromise;

  return loadedLanguages.has(normalized);
}

/**
 * Highlight code with syntax highlighting
 * Returns HTML string with highlighting or plain text if language not available
 */
export async function highlightCode(code: string, language: string): Promise<string> {
  const normalized = language.toLowerCase();

  // Try to load the language if not already loaded
  await ensureLanguageLoaded(normalized);

  // Check if language is available
  if (hljs.getLanguage(normalized)) {
    try {
      const result = hljs.highlight(code, { language: normalized });
      return result.value;
    } catch (error) {
      console.warn(`[SyntaxHighlighter] Error highlighting ${normalized}:`, error);
      return hljs.highlight(code, { language: 'plaintext' }).value;
    }
  }

  // Fallback to auto-detect or plaintext
  try {
    const result = hljs.highlightAuto(code);
    return result.value;
  } catch (error) {
    console.warn(`[SyntaxHighlighter] Auto-highlight failed:`, error);
    return code; // Return plain text
  }
}

/**
 * Get the underlying highlight.js instance
 */
export function getHljsInstance() {
  return hljs;
}

/**
 * Check if a language is currently loaded
 */
export function isLanguageLoaded(languageId: string): boolean {
  return loadedLanguages.has(languageId.toLowerCase());
}

/**
 * Preload multiple languages at once
 */
export async function preloadLanguages(languageIds: string[]): Promise<void> {
  await Promise.all(languageIds.map(id => ensureLanguageLoaded(id)));
}
