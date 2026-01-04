#!/usr/bin/env node
/**
 * Copy matching translations from main app to admin app
 *
 * This script copies translations for matching keys from the main app
 * to the admin app, leaving non-matching keys in English as placeholders.
 *
 * Usage:
 *   node scripts/copy-admin-translations.js
 */

import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Languages to process (all except en-GB which is the source)
const LANGUAGES = [
  'zh', 'nl', 'fr', 'de', 'el', 'it',
  'ja', 'ko', 'pl', 'pt', 'ru', 'es', 'tr'
];

// Mapping of admin keys to main app keys
// Many admin keys can use the same translation as main app keys
const KEY_MAPPINGS = {
  // Common words
  'common.refresh': 'common.refresh',
  'common.cancel': 'common.cancel',
  'common.confirm': 'common.confirm',
  'common.loading': 'common.loading',
  'common.email': 'settings.sync.email',
  'common.password': 'login.password',
  'common.actions': null, // "Actions" - generic word
  'common.status': null, // "Status" - generic word
  'common.created': null, // "Created" - generic word

  // Navigation
  'nav.dashboard': null, // "Dashboard" - keep English
  'nav.users': null, // "Users" - keep English
  'nav.settings': 'nav.settings',
  'nav.logout': 'menu.logout',

  // Login
  'login.password': 'login.password',
  'login.errors.invalidCredentials': 'login.errors.invalidCredentials',

  // Settings
  'settings.changePassword.title': 'settings.changePassword.title',
  'settings.changePassword.currentPassword': 'settings.changePassword.currentPassword',
  'settings.changePassword.newPassword': 'settings.changePassword.newPassword',
  'settings.changePassword.confirmPassword': 'settings.changePassword.confirmPassword',
  'settings.changePassword.button': 'settings.changePassword.button',
  'settings.changePassword.buttonChanging': 'settings.changePassword.buttonChanging',
  'settings.changePassword.cancel': 'common.cancel',
  'settings.changePassword.success': 'settings.changePassword.success',
  'settings.changePassword.errors.currentRequired': 'settings.changePassword.errors.currentRequired',
  'settings.changePassword.errors.newRequired': 'settings.changePassword.errors.newRequired',
  'settings.changePassword.errors.minLength': 'settings.changePassword.errors.minLength',
  'settings.changePassword.errors.mismatch': 'settings.changePassword.errors.mismatch',
  'settings.changePassword.errors.failed': 'settings.changePassword.errors.failed',
};

// Get nested value from object using dot notation
function getNestedValue(obj, path) {
  return path.split('.').reduce((current, key) => current?.[key], obj);
}

// Set nested value in object using dot notation
function setNestedValue(obj, path, value) {
  const keys = path.split('.');
  const lastKey = keys.pop();
  const target = keys.reduce((current, key) => {
    if (!current[key]) current[key] = {};
    return current[key];
  }, obj);
  target[lastKey] = value;
}

// Get all keys from object recursively
function getAllKeys(obj, prefix = '') {
  const keys = [];
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
      keys.push(...getAllKeys(value, fullKey));
    } else if (typeof value === 'string') {
      keys.push(fullKey);
    }
  }
  return keys;
}

async function main() {
  console.log('Copying Admin Translations from Main App\n');
  console.log('============================================\n');

  const adminEnGB = JSON.parse(
    await fs.readFile(path.join(__dirname, '../admin/src/locales/en-GB.json'), 'utf8')
  );

  let totalCopied = 0;
  let totalKeys = 0;

  for (const lang of LANGUAGES) {
    console.log(`\nProcessing ${lang}...`);

    // Load main app translation
    const mainTranslation = JSON.parse(
      await fs.readFile(path.join(__dirname, '../src/locales', `${lang}.json`), 'utf8')
    );

    // Load current admin translation (English placeholders)
    const adminTranslation = JSON.parse(
      await fs.readFile(path.join(__dirname, '../admin/src/locales', `${lang}.json`), 'utf8')
    );

    let copied = 0;
    const adminKeys = getAllKeys(adminEnGB);
    totalKeys = adminKeys.length;

    // Copy mapped translations
    for (const [adminKey, mainKey] of Object.entries(KEY_MAPPINGS)) {
      if (mainKey) {
        const translatedValue = getNestedValue(mainTranslation, mainKey);
        if (translatedValue) {
          setNestedValue(adminTranslation, adminKey, translatedValue);
          copied++;
        }
      }
    }

    // Save updated admin translation
    await fs.writeFile(
      path.join(__dirname, '../admin/src/locales', `${lang}.json`),
      JSON.stringify(adminTranslation, null, 2) + '\n',
      'utf8'
    );

    console.log(`  Copied: ${copied}/${totalKeys} strings`);
    console.log(`  Remaining in English: ${totalKeys - copied} strings`);
    totalCopied += copied;
  }

  console.log(`\n\n✅ Translation copy complete!\n`);
  console.log(`Summary:`);
  console.log(`  - Processed ${LANGUAGES.length} languages`);
  console.log(`  - Copied ${totalCopied} total translations`);
  console.log(`  - Average: ${Math.round(totalCopied / LANGUAGES.length)} per language`);
  console.log(`\nNote: Remaining English strings need manual translation.`);
  console.log(`These are admin-specific terms not present in the main app.\n`);
}

main().catch(console.error);
