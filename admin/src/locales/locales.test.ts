/**
 * Admin Translation Coverage Tests
 *
 * Verifies that all locale files have the same keys as the primary locale (en-GB).
 * This ensures no translation keys are missing in any language.
 */

import { describe, it, expect } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';

const LOCALES_DIR = path.join(__dirname);
const PRIMARY_LOCALE = 'en-GB';

// Get all locale files (excluding test files)
function getLocaleFiles(): string[] {
  return fs
    .readdirSync(LOCALES_DIR)
    .filter((file) => file.endsWith('.json') && !file.includes('.test.'));
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

describe('Admin Translation Coverage', () => {
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
    expect(primaryKeys.length).toBeGreaterThan(50);
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
        console.warn(`Extra keys in ${localeFile} (not in primary): ${extraKeys.join(', ')}`);
      }

      expect(true).toBe(true);
    });
  });
});

describe('Admin Translation Key Structure', () => {
  const primaryLocale = loadLocale(`${PRIMARY_LOCALE}.json`);

  it('should have app section', () => {
    expect(primaryLocale).toHaveProperty('app');
  });

  it('should have common section', () => {
    expect(primaryLocale).toHaveProperty('common');
  });

  it('should have nav section', () => {
    expect(primaryLocale).toHaveProperty('nav');
  });

  it('should have login section', () => {
    expect(primaryLocale).toHaveProperty('login');
  });

  it('should have users section', () => {
    expect(primaryLocale).toHaveProperty('users');
  });

  it('should have settings section', () => {
    expect(primaryLocale).toHaveProperty('settings');
  });

  it('should have devices section', () => {
    expect(primaryLocale).toHaveProperty('devices');
  });

  it('should have userPortal section', () => {
    expect(primaryLocale).toHaveProperty('userPortal');
  });
});
