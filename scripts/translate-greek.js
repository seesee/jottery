#!/usr/bin/env node
/**
 * Translate Greek locale file only
 * (Welsh not supported by LibreTranslate)
 */

import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const LIBRETRANSLATE_URL = 'https://libretranslate.com/translate';
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function extractPlaceholders(text) {
  const placeholders = [];
  const patterns = [
    /\{[^}]+\}/g,
    /<[^>]+>/g,
    /\$\{[^}]+\}/g,
  ];

  patterns.forEach(pattern => {
    const matches = text.match(pattern);
    if (matches) {
      placeholders.push(...matches);
    }
  });

  return [...new Set(placeholders)];
}

function protectPlaceholders(text) {
  const placeholders = extractPlaceholders(text);
  let protectedText = text;
  const markers = [];

  placeholders.forEach((placeholder, index) => {
    const marker = `__PLACEHOLDER_${index}__`;
    protectedText = protectedText.replace(placeholder, marker);
    markers.push({ marker, original: placeholder });
  });

  return { protectedText, markers };
}

function restorePlaceholders(text, markers) {
  let restored = text;
  markers.forEach(({ marker, original }) => {
    restored = restored.replace(marker, original);
  });
  return restored;
}

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

async function translateObject(obj, targetLang, progress = { current: 0, total: 0 }) {
  const result = {};

  for (const [key, value] of Object.entries(obj)) {
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
      result[key] = await translateObject(value, targetLang, progress);
    } else if (typeof value === 'string' && value.length > 0) {
      const { protectedText, markers } = protectPlaceholders(value);

      progress.current++;
      process.stdout.write(`\r  Progress: ${progress.current}/${progress.total} strings`);

      const translated = await translateWithLibreTranslate(protectedText, targetLang);
      await delay(200);

      result[key] = restorePlaceholders(translated, markers);
    } else {
      result[key] = value;
    }
  }

  return result;
}

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
  console.log('Translating Greek locale file\n');
  console.log('====================================\n');

  const sourceFile = path.join(__dirname, '../src/locales/en-US.json');
  const sourceData = JSON.parse(await fs.readFile(sourceFile, 'utf8'));

  const totalStrings = countStrings(sourceData);
  console.log(`Source file: en-US.json (${totalStrings} strings)\n`);

  console.log('Translating to Greek (el)...');
  const progress = { current: 0, total: totalStrings };
  const translated = await translateObject(sourceData, 'el', progress);

  const outputFile = path.join(__dirname, '../src/locales/el.json');
  await fs.writeFile(
    outputFile,
    JSON.stringify(translated, null, 2) + '\n',
    'utf8'
  );

  console.log('\n  ✓ Saved: el.json');
  console.log('\n✅ Greek translation complete!\n');
}

main().catch(console.error);
