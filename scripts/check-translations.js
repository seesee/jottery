#!/usr/bin/env node

/**
 * Translation Checker
 *
 * Compares non-English locale files against en-GB.json to find
 * potentially untranslated strings (identical values).
 *
 * Usage:
 *   node scripts/check-translations.js [--json] [--fail-on-issues]
 *
 * Options:
 *   --json           Output results as JSON
 *   --fail-on-issues Exit with code 1 if issues found (for CI)
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const LOCALES_DIR = path.join(__dirname, '..', 'src', 'locales');

// Non-English locales to check
const LOCALES_TO_CHECK = [
  'de', 'fr', 'es', 'it', 'pt', 'nl', 'pl', 'tr', 'el', 'ja', 'ko', 'ru', 'zh'
];

// Keys/patterns to exclude from comparison (legitimate English or technical terms)
const EXCLUSIONS = {
  // Exact key matches (these keys are expected to have English/technical values)
  keys: new Set([
    'app.name',                    // Product name "Jottery"
    'landing.tuiClient',           // Technical term
    'landing.standaloneWebApp',    // Technical term
    'inbox.token.title',           // "Inbox API" - API name
    'settings.sync.existingAccount.emailPlaceholder', // Email format placeholder
    'settings.sync.newAccount.emailPlaceholder',
    'onboarding.sync.existingAccount.emailPlaceholder',
    'onboarding.sync.newAccount.emailPlaceholder',
    // Password placeholders (dots)
    'settings.syncSetup.registration.passwordPlaceholder',
    'settings.syncSetup.registration.confirmPasswordPlaceholder',
    // CLI code examples (kept in English as they're actual commands)
    'landing.tuiFeatures.cli.code',
    'inbox.token.curlExample',
  ]),

  // Key patterns to exclude (regex matching against key names)
  keyPatterns: [
    /\.code$/,                     // Code examples
    /\.example$/,                  // Examples
    /Placeholder$/,                // Placeholders
  ],

  // Value patterns to exclude (regex)
  valuePatterns: [
    /^you@example\.com$/,          // Email placeholder
    /^https?:\/\//,                // URLs
    /^\{[a-z_]+\}$/,               // Pure template variables like {count}
    /^[A-Z]{2,}$/,                 // Acronyms like "API", "TUI", "PDF"
    /^v?\d+\.\d+/,                 // Version numbers
    /^[a-z]+:\/\//,                // Protocol prefixes
    /^#[a-zA-Z]/,                  // Tag references like #tag
    /^\d+$/,                       // Pure numbers
    /^[A-Z][a-z]+\s+(API|SDK|CLI|TUI|GUI)$/, // Technical terms
    /^•+$/,                        // Password dots
    /^jottery\s/,                  // CLI commands
  ],

  // Short values to ignore (single words that may legitimately be the same)
  maxLengthToIgnore: 3,

  // Values that are intentionally kept in English across all locales
  // These are technical terms, product names, or universal words
  exactValues: new Set([
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
    // Technical UI terms often kept in English
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
    // Section names that may match across languages
    'Quick Start',
    // Words that are identical in multiple languages
    'Syntax:',       // Same in German
    'Version:',      // Same in German
    'Description',   // Same in French
    'note',          // Same in French
    'notes',         // Same in French
    'Error',         // Same in Spanish
    'Tags:',         // Same in Portuguese/Dutch
    '1 item',        // Same in Portuguese/Dutch (borrowed English)
  ]),
};

/**
 * Recursively flatten a nested object into dot-notation keys
 */
function flattenObject(obj, prefix = '') {
  const result = {};

  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;

    if (value && typeof value === 'object' && !Array.isArray(value)) {
      Object.assign(result, flattenObject(value, fullKey));
    } else {
      result[fullKey] = value;
    }
  }

  return result;
}

/**
 * Check if a key/value pair should be excluded from comparison
 */
function shouldExclude(key, value) {
  // Exclude by exact key
  if (EXCLUSIONS.keys.has(key)) {
    return true;
  }

  // Exclude by key pattern
  for (const pattern of EXCLUSIONS.keyPatterns) {
    if (pattern.test(key)) {
      return true;
    }
  }

  // Exclude arrays (like feature lists - these need manual review)
  if (Array.isArray(value)) {
    return true;
  }

  // Only check string values
  if (typeof value !== 'string') {
    return true;
  }

  // Exclude very short values
  if (value.length <= EXCLUSIONS.maxLengthToIgnore) {
    return true;
  }

  // Exclude exact value matches
  if (EXCLUSIONS.exactValues.has(value)) {
    return true;
  }

  // Exclude values matching patterns
  for (const pattern of EXCLUSIONS.valuePatterns) {
    if (pattern.test(value)) {
      return true;
    }
  }

  return false;
}

/**
 * Compare two locale files and find identical values
 */
function compareLocales(englishData, localeData, localeName) {
  const englishFlat = flattenObject(englishData);
  const localeFlat = flattenObject(localeData);

  const issues = [];

  for (const [key, englishValue] of Object.entries(englishFlat)) {
    const localeValue = localeFlat[key];

    // Skip if key doesn't exist in locale (missing key is a different issue)
    if (localeValue === undefined) {
      continue;
    }

    // Skip excluded keys/values
    if (shouldExclude(key, englishValue)) {
      continue;
    }

    // Check if values are identical
    if (englishValue === localeValue) {
      issues.push({
        key,
        value: englishValue,
        locale: localeName,
      });
    }
  }

  return issues;
}

/**
 * Main function
 */
function main() {
  const args = process.argv.slice(2);
  const jsonOutput = args.includes('--json');
  const failOnIssues = args.includes('--fail-on-issues');

  // Load English reference
  const englishPath = path.join(LOCALES_DIR, 'en-GB.json');
  const englishData = JSON.parse(fs.readFileSync(englishPath, 'utf8'));

  const allIssues = {};
  let totalIssues = 0;

  // Check each locale
  for (const locale of LOCALES_TO_CHECK) {
    const localePath = path.join(LOCALES_DIR, `${locale}.json`);

    if (!fs.existsSync(localePath)) {
      console.error(`Warning: ${locale}.json not found`);
      continue;
    }

    const localeData = JSON.parse(fs.readFileSync(localePath, 'utf8'));
    const issues = compareLocales(englishData, localeData, locale);

    if (issues.length > 0) {
      allIssues[locale] = issues;
      totalIssues += issues.length;
    }
  }

  // Output results
  if (jsonOutput) {
    console.log(JSON.stringify({
      totalIssues,
      localesChecked: LOCALES_TO_CHECK.length,
      issues: allIssues,
    }, null, 2));
  } else {
    if (totalIssues === 0) {
      console.log('✓ No untranslated strings found!\n');
      console.log(`Checked ${LOCALES_TO_CHECK.length} locales against en-GB.json`);
    } else {
      console.log(`Found ${totalIssues} potentially untranslated strings:\n`);

      for (const [locale, issues] of Object.entries(allIssues)) {
        console.log(`\n=== ${locale}.json (${issues.length} issues) ===`);
        for (const issue of issues) {
          console.log(`  ${issue.key}`);
          console.log(`    "${issue.value.substring(0, 60)}${issue.value.length > 60 ? '...' : ''}"`);
        }
      }

      console.log('\n---');
      console.log('To exclude false positives, add keys to EXCLUSIONS in this script.');
      console.log('For technical terms, add to EXCLUSIONS.exactValues.');
    }
  }

  // Exit with error code if issues found and --fail-on-issues is set
  if (failOnIssues && totalIssues > 0) {
    process.exit(1);
  }
}

main();
