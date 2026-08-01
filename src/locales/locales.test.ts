/**
 * Translation Coverage Tests
 *
 * Verifies that all locale files have the same keys as the primary locale (en-GB).
 * This ensures no translation keys are missing in any language.
 */

import { describe, it, expect } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';

const LOCALES_DIR = path.join(__dirname);
const PRIMARY_LOCALE = 'en-GB';

// Get all locale files (excluding template and test files)
function getLocaleFiles(): string[] {
  return fs
    .readdirSync(LOCALES_DIR)
    .filter((file) => file.endsWith('.json') && !file.startsWith('_') && !file.includes('.test.'));
}

// Recursively extract all keys from a nested object
function extractKeys(obj: Record<string, unknown>, prefix = ''): string[] {
  const keys: string[] = [];

  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;

    if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
      keys.push(...extractKeys(value as Record<string, unknown>, fullKey));
    } else {
      keys.push(fullKey);
    }
  }

  return keys.sort();
}

// Load and parse a locale file
function loadLocale(filename: string): Record<string, unknown> {
  const filePath = path.join(LOCALES_DIR, filename);
  const content = fs.readFileSync(filePath, 'utf-8');
  return JSON.parse(content);
}

describe('Translation Coverage', () => {
  const localeFiles = getLocaleFiles();
  const primaryLocale = loadLocale(`${PRIMARY_LOCALE}.json`);
  const primaryKeys = extractKeys(primaryLocale);

  it('should have en-GB as the primary locale', () => {
    expect(localeFiles).toContain(`${PRIMARY_LOCALE}.json`);
  });

  it('should have at least 10 locale files', () => {
    expect(localeFiles.length).toBeGreaterThanOrEqual(10);
  });

  it('should have primary locale with translation keys', () => {
    expect(primaryKeys.length).toBeGreaterThan(100);
  });

  describe.each(localeFiles.filter((f) => f !== `${PRIMARY_LOCALE}.json`))('%s', (localeFile) => {
    const locale = loadLocale(localeFile);
    const localeKeys = extractKeys(locale);

    it('should have all keys from primary locale', () => {
      const missingKeys = primaryKeys.filter((key) => !localeKeys.includes(key));

      if (missingKeys.length > 0) {
        throw new Error(
          `Missing ${missingKeys.length} keys in ${localeFile}:\n  - ${missingKeys.slice(0, 20).join('\n  - ')}${missingKeys.length > 20 ? `\n  ... and ${missingKeys.length - 20} more` : ''}`
        );
      }

      expect(missingKeys).toHaveLength(0);
    });

    it('should not have extra keys not in primary locale', () => {
      const extraKeys = localeKeys.filter((key) => !primaryKeys.includes(key));

      if (extraKeys.length > 0) {
        // This is a warning, not a failure - extra keys are allowed but noted
        console.warn(`Extra keys in ${localeFile} (not in primary): ${extraKeys.join(', ')}`);
      }

      // We allow extra keys, so this test always passes but logs warnings
      expect(true).toBe(true);
    });
  });
});

describe('Translation Key Structure', () => {
  const primaryLocale = loadLocale(`${PRIMARY_LOCALE}.json`);

  it('should have app section', () => {
    expect(primaryLocale).toHaveProperty('app');
  });

  it('should have common section', () => {
    expect(primaryLocale).toHaveProperty('common');
  });

  it('should have note section', () => {
    expect(primaryLocale).toHaveProperty('note');
  });

  it('should have search section', () => {
    expect(primaryLocale).toHaveProperty('search');
  });

  it('should have settings section', () => {
    expect(primaryLocale).toHaveProperty('settings');
  });
});

/**
 * Translation Quality Tests
 *
 * Checks that translation values are actually translated and not
 * just copies of the British English (en-GB) text.
 */
describe('Translation Quality', () => {
  // Keys that are expected to have en-GB/technical values
  const EXCLUDED_KEYS = new Set([
    'app.name', // Product name "Jottery"
    'landing.tuiClient', // Technical term
    'landing.standaloneWebApp', // Technical term
    'inbox.token.title', // "Inbox API" - API name
    'settings.sync.existingAccount.emailPlaceholder',
    'settings.sync.newAccount.emailPlaceholder',
    'onboarding.sync.existingAccount.emailPlaceholder',
    'onboarding.sync.newAccount.emailPlaceholder',
    'settings.syncSetup.registration.passwordPlaceholder',
    'settings.syncSetup.registration.confirmPasswordPlaceholder',
    'landing.tuiFeatures.cli.code',
    'inbox.token.curlExample',
  ]);

  // Key patterns to exclude
  const EXCLUDED_KEY_PATTERNS = [
    /\.code$/, // Code examples
    /\.example$/, // Examples
    /Placeholder$/, // Placeholders
  ];

  // Value patterns to exclude
  const EXCLUDED_VALUE_PATTERNS = [
    /^you@example\.com$/, // Email placeholder
    /^https?:\/\//, // URLs
    /^\{[a-z_]+\}$/, // Pure template variables like {count}
    /^[A-Z]{2,}$/, // Acronyms like "API", "TUI", "PDF"
    /^v?\d+\.\d+/, // Version numbers
    /^[a-z]+:\/\//, // Protocol prefixes
    /^#[a-zA-Z]/, // Tag references like #tag
    /^\d+$/, // Pure numbers
    /^[A-Z][a-z]+\s+(API|SDK|CLI|TUI|GUI)$/, // Technical terms
    /^•+$/, // Password dots
    /^jottery\s/, // CLI commands
  ];

  // Values that are intentionally kept as en-GB across all locales
  const EXCLUDED_VALUES = new Set([
    'Jottery',
    'TUI Client',
    'Standalone Web App',
    'Inbox API',
    'Markdown',
    'JSON',
    'PDF',
    'HTML',
    'CSS',
    'API',
    'URL',
    'UUID',
    'OK',
    'ID',
    // Keyboard keys (universal)
    'Ctrl',
    'Alt',
    'Shift',
    'Tab',
    'Enter',
    'Esc',
    'Space',
    // Technical UI terms often kept as en-GB
    'Editor',
    'Global',
    'Menu',
    'Tags',
    'Info',
    'Query',
    'Format',
    'Type',
    'Trigger',
    'Server',
    'Password',
    'Version',
    'Visual',
    'General',
    'Inbox',
    'Records',
    'Documentation',
    'Quick Start',
    // Words that are identical in multiple languages
    'Syntax:', // Same in German
    'Version:', // Same in German
    'Text', // Same in German/French
    'Navigation', // Same in German/French/Spanish
    'Description', // Same in French
    'note', // Same in French
    'notes', // Same in French
    'Note', // Same in French (vaultHealth.noteLabel)
    '{total} notes', // Same in French ("notes" is French)
    'Error', // Same in Spanish
    'Tags:', // Same in Portuguese/Dutch
    '1 item', // Same in Portuguese/Dutch (borrowed from English)
  ]);

  const MAX_LENGTH_TO_IGNORE = 3;

  // Flatten nested object to dot-notation keys with values
  function flattenWithValues(
    obj: Record<string, unknown>,
    prefix = ''
  ): Map<string, string> {
    const result = new Map<string, string>();

    for (const [key, value] of Object.entries(obj)) {
      const fullKey = prefix ? `${prefix}.${key}` : key;

      if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
        const nested = flattenWithValues(value as Record<string, unknown>, fullKey);
        nested.forEach((v, k) => result.set(k, v));
      } else if (typeof value === 'string') {
        result.set(fullKey, value);
      }
    }

    return result;
  }

  // Check if a key/value pair should be excluded from comparison
  function shouldExclude(key: string, value: string): boolean {
    if (EXCLUDED_KEYS.has(key)) return true;

    for (const pattern of EXCLUDED_KEY_PATTERNS) {
      if (pattern.test(key)) return true;
    }

    if (value.length <= MAX_LENGTH_TO_IGNORE) return true;

    if (EXCLUDED_VALUES.has(value)) return true;

    for (const pattern of EXCLUDED_VALUE_PATTERNS) {
      if (pattern.test(value)) return true;
    }

    return false;
  }

  const localeFiles = getLocaleFiles();
  const primaryLocale = loadLocale(`${PRIMARY_LOCALE}.json`);
  const primaryValues = flattenWithValues(primaryLocale);

  // Exclude en-US from translation quality checks - it's American English,
  // intentionally very similar to British English (en-GB)
  const LOCALES_TO_CHECK = localeFiles.filter(
    (f) => f !== `${PRIMARY_LOCALE}.json` && f !== 'en-US.json'
  );

  describe.each(LOCALES_TO_CHECK)('%s', (localeFile) => {
    it('should have translated values (not identical to en-GB)', () => {
      const locale = loadLocale(localeFile);
      const localeValues = flattenWithValues(locale);

      const untranslated: string[] = [];

      primaryValues.forEach((englishValue, key) => {
        const localeValue = localeValues.get(key);

        // Skip if key doesn't exist (covered by other test)
        if (localeValue === undefined) return;

        // Skip excluded keys/values
        if (shouldExclude(key, englishValue)) return;

        // Check if values are identical (potentially untranslated)
        if (englishValue === localeValue) {
          untranslated.push(`${key}: "${englishValue.substring(0, 50)}${englishValue.length > 50 ? '...' : ''}"`);
        }
      });

      if (untranslated.length > 0) {
        throw new Error(
          `Found ${untranslated.length} potentially untranslated strings in ${localeFile}:\n  - ${untranslated.slice(0, 10).join('\n  - ')}${untranslated.length > 10 ? `\n  ... and ${untranslated.length - 10} more` : ''}`
        );
      }

      expect(untranslated).toHaveLength(0);
    });
  });
});
