#!/usr/bin/env node
/**
 * Generate locale template files for translation
 *
 * This creates properly structured locale files for each target language,
 * copying the English text as a starting point. Translators can then
 * replace the English text with native translations.
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Target languages with their codes
const TARGET_LANGUAGES = {
  'es': { code: 'es', name: 'Spanish', nativeName: 'Español' },
  'fr': { code: 'fr', name: 'French', nativeName: 'Français' },
  'de': { code: 'de', name: 'German', nativeName: 'Deutsch' },
  'it': { code: 'it', name: 'Italian', nativeName: 'Italiano' },
  'pt': { code: 'pt', name: 'Portuguese', nativeName: 'Português' },
  'ja': { code: 'ja', name: 'Japanese', nativeName: '日本語' },
  'zh': { code: 'zh', name: 'Chinese Simplified', nativeName: '简体中文' },
  'ko': { code: 'ko', name: 'Korean', nativeName: '한국어' },
  'ru': { code: 'ru', name: 'Russian', nativeName: 'Русский' },
  'ar': { code: 'ar', name: 'Arabic', nativeName: 'العربية' },
  'nl': { code: 'nl', name: 'Dutch', nativeName: 'Nederlands' },
  'pl': { code: 'pl', name: 'Polish', nativeName: 'Polski' },
  'tr': { code: 'tr', name: 'Turkish', nativeName: 'Türkçe' },
};

function main() {
  console.log('\nGenerating locale template files\n');
  console.log('=================================\n');

  // Read source file
  const sourceFile = path.join(__dirname, '../src/locales/en-US.json');
  const sourceData = JSON.parse(fs.readFileSync(sourceFile, 'utf8'));

  const outputDir = path.join(__dirname, '../src/locales');

  // Generate each language file
  Object.entries(TARGET_LANGUAGES).forEach(([langCode, langInfo]) => {
    const outputFile = path.join(outputDir, `${langCode}.json`);

    // Copy English as template
    fs.writeFileSync(
      outputFile,
      JSON.stringify(sourceData, null, 2) + '\n',
      'utf8'
    );

    console.log(`✓ Created: ${langCode}.json - ${langInfo.name} (${langInfo.nativeName})`);
  });

  console.log('\n✅ Template files created!\n');
  console.log('📝 Next steps for translation:\n');
  console.log('   Option 1: Use DeepL API (recommended, high quality)');
  console.log('   Option 2: Use Google Cloud Translation API');
  console.log('   Option 3: Upload to a translation platform (Crowdin, Lokalise, etc.)');
  console.log('   Option 4: Hire professional translators');
  console.log('\n   Then update src/lib/services/i18nService.ts to register them.\n');
}

main();
