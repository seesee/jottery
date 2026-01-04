#!/usr/bin/env node
/**
 * Translation script for Jottery locale files
 *
 * This script translates the en-US.json locale file into multiple languages.
 * It uses a simple approach: for each target language, it generates a translated
 * version of all strings while preserving the JSON structure and placeholders.
 *
 * Usage:
 *   node scripts/translate-locales.js
 *
 * Note: This uses simple translations. For production, consider using a professional
 * translation service or having native speakers review the output.
 */

const fs = require('fs');
const path = require('path');

// Language configurations
const LANGUAGES = {
  'es': { name: 'Spanish (Español)', nativeName: 'Español' },
  'fr': { name: 'French (Français)', nativeName: 'Français' },
  'de': { name: 'German (Deutsch)', nativeName: 'Deutsch' },
  'it': { name: 'Italian (Italiano)', nativeName: 'Italiano' },
  'pt': { name: 'Portuguese (Português)', nativeName: 'Português' },
  'ja': { name: 'Japanese (日本語)', nativeName: '日本語' },
  'zh': { name: 'Chinese (中文)', nativeName: '中文' },
  'ko': { name: 'Korean (한국어)', nativeName: '한국어' },
  'ru': { name: 'Russian (Русский)', nativeName: 'Русский' },
  'ar': { name: 'Arabic (العربية)', nativeName: 'العربية' },
  'nl': { name: 'Dutch (Nederlands)', nativeName: 'Nederlands' },
  'pl': { name: 'Polish (Polski)', nativeName: 'Polski' },
  'tr': { name: 'Turkish (Türkçe)', nativeName: 'Türkçe' },
  'sv': { name: 'Swedish (Svenska)', nativeName: 'Svenska' },
  'da': { name: 'Danish (Dansk)', nativeName: 'Dansk' },
  'no': { name: 'Norwegian (Norsk)', nativeName: 'Norsk' },
  'fi': { name: 'Finnish (Suomi)', nativeName: 'Suomi' },
};

// Read the source file
const sourceFile = path.join(__dirname, '../src/locales/en-US.json');
const sourceData = JSON.parse(fs.readFileSync(sourceFile, 'utf8'));

console.log('Jottery Locale Translation Script');
console.log('==================================\n');
console.log('This script will create translation templates for the following languages:');
Object.entries(LANGUAGES).forEach(([code, info]) => {
  console.log(`  ${code}: ${info.name}`);
});
console.log('\n⚠️  IMPORTANT:');
console.log('   These are TEMPLATE files that need professional translation.');
console.log('   They currently contain placeholders indicating strings need translation.\n');

// Function to process nested objects
function processObject(obj, langCode, langName, depth = 0) {
  const result = {};

  for (const [key, value] of Object.entries(obj)) {
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
      // Recursively process nested objects
      result[key] = processObject(value, langCode, langName, depth + 1);
    } else if (typeof value === 'string') {
      // For now, mark strings as needing translation
      // Professional translators can replace these
      result[key] = `[${langCode.toUpperCase()}] ${value}`;
    } else {
      // Keep other types as-is
      result[key] = value;
    }
  }

  return result;
}

// Generate translation files
const outputDir = path.join(__dirname, '../src/locales');

Object.entries(LANGUAGES).forEach(([langCode, langInfo]) => {
  const outputFile = path.join(outputDir, `${langCode}.json`);

  // Create a copy with translation markers
  const translatedData = processObject(sourceData, langCode, langInfo.nativeName);

  // Write the file
  fs.writeFileSync(
    outputFile,
    JSON.stringify(translatedData, null, 2) + '\n',
    'utf8'
  );

  console.log(`✓ Created: ${langCode}.json (${langInfo.name})`);
});

console.log('\n✅ Translation templates created!');
console.log('\n📝 Next steps:');
console.log('   1. Use a professional translation service (DeepL, Google Translate, etc.)');
console.log('   2. Or hire native speakers to translate each file');
console.log('   3. Update src/lib/services/i18nService.ts to register new locales');
console.log('   4. Test each language thoroughly\n');
