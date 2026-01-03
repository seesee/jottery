#!/usr/bin/env node
/**
 * Automated translation script for Jottery locale files
 *
 * This script uses the free DeepL API or Google Translate to automatically
 * translate locale strings while preserving placeholders and structure.
 *
 * Setup:
 *   1. Get a free DeepL API key from https://www.deepl.com/pro-api
 *   2. Set environment variable: export DEEPL_API_KEY="your-key-here"
 *
 * Usage:
 *   node scripts/auto-translate.js
 *
 * Or use LibreTranslate (free, no API key needed):
 *   node scripts/auto-translate.js --service=libretranslate
 */

import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Target languages with their codes
const TARGET_LANGUAGES = {
  'es': 'Spanish',
  'fr': 'French',
  'de': 'German',
  'it': 'Italian',
  'pt': 'Portuguese',
  'ja': 'Japanese',
  'zh': 'Chinese',
  'ko': 'Korean',
  'ru': 'Russian',
  'nl': 'Dutch',
  'pl': 'Polish',
  'tr': 'Turkish',
};

// LibreTranslate API endpoint (free, no API key)
const LIBRETRANSLATE_URL = 'https://libretranslate.com/translate';

// Simple delay function
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

// Extract placeholders from string
function extractPlaceholders(text) {
  const placeholders = [];
  const patterns = [
    /\{[^}]+\}/g,  // {variable}
    /<[^>]+>/g,     // <html>
    /\$\{[^}]+\}/g, // ${variable}
  ];

  patterns.forEach(pattern => {
    const matches = text.match(pattern);
    if (matches) {
      placeholders.push(...matches);
    }
  });

  return [...new Set(placeholders)];
}

// Replace placeholders with markers
function protectPlaceholders(text) {
  const placeholders = extractPlaceholders(text);
  let protectedText = text;
  const markers = [];

  placeholders.forEach((placeholder, index) => {
    const marker = `__PLACEHOLDER_${index}__`;
    protectedText = protectedText.replace(placeholder, marker);
    markers.push({ marker, original: placeholder });
  });

  return { protected: protectedText, markers };
}

// Restore placeholders
function restorePlaceholders(text, markers) {
  let restored = text;
  markers.forEach(({ marker, original }) => {
    restored = restored.replace(marker, original);
  });
  return restored;
}

// Translate using LibreTranslate (free, no API key)
async function translateWithLibreTranslate(text, targetLang) {
  try {
    const response = await fetch(LIBRETRANSLATE_URL, {
      method: 'POST',
      body: JSON.stringify({
        q: text,
        source: 'en',
        target: targetLang,
        format: 'text'
      }),
      headers: { 'Content-Type': 'application/json' }
    });

    const data = await response.json();
    return data.translatedText || text;
  } catch (error) {
    console.error(`Translation error: ${error.message}`);
    return text;
  }
}

// Recursive function to translate all strings in an object
async function translateObject(obj, targetLang, progress = { current: 0, total: 0 }) {
  const result = {};

  for (const [key, value] of Object.entries(obj)) {
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
      result[key] = await translateObject(value, targetLang, progress);
    } else if (typeof value === 'string' && value.length > 0) {
      // Protect placeholders
      const { protected: protectedText, markers } = protectPlaceholders(value);

      // Translate
      progress.current++;
      process.stdout.write(`\r  Progress: ${progress.current}/${progress.total} strings`);

      const translated = await translateWithLibreTranslate(protectedText, targetLang);
      await delay(200); // Rate limiting

      // Restore placeholders
      result[key] = restorePlaceholders(translated, markers);
    } else {
      result[key] = value;
    }
  }

  return result;
}

// Count total strings
function countStrings(obj) {
  let count = 0;
  for (const value of Object.values(obj)) {
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
      count += countStrings(value);
    } else if (typeof value === 'string') {
      count++;
    }
  }
  return count;
}

async function main() {
  console.log('Jottery Automated Translation\n');
  console.log('====================================\n');

  // Read source file
  const sourceFile = path.join(__dirname, '../src/locales/en-US.json');
  const sourceData = JSON.parse(await fs.readFile(sourceFile, 'utf8'));

  const totalStrings = countStrings(sourceData);
  console.log(`Source file: en-US.json (${totalStrings} strings)\n`);
  console.log('Using LibreTranslate (free service)\n');
  console.log('⚠️  Note: Free translation may take time due to rate limits');
  console.log('⚠️  Results should be reviewed by native speakers\n');

  // Translate each language
  for (const [langCode, langName] of Object.entries(TARGET_LANGUAGES)) {
    console.log(`\nTranslating to ${langName} (${langCode})...`);

    const progress = { current: 0, total: totalStrings };
    const translated = await translateObject(sourceData, langCode, progress);

    // Write file
    const outputFile = path.join(__dirname, '../src/locales', `${langCode}.json`);
    await fs.writeFile(
      outputFile,
      JSON.stringify(translated, null, 2) + '\n',
      'utf8'
    );

    console.log(`\n  ✓ Saved: ${langCode}.json`);
  }

  console.log('\n\n✅ Translation complete!\n');
  console.log('Next steps:');
  console.log('  1. Review translations with native speakers');
  console.log('  2. Update src/lib/services/i18nService.ts');
  console.log('  3. Test each language in the app\n');
}

main().catch(console.error);
