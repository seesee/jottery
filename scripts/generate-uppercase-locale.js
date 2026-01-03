#!/usr/bin/env node
/**
 * Generate ALL CAPS test language (EN-GB.json) from en-GB.json
 * This helps identify any hardcoded strings that haven't been translated
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const sourceFile = path.join(__dirname, '../src/locales/en-GB.json');
const targetFile = path.join(__dirname, '../src/locales/EN-GB.json');

// Read the source file
const source = JSON.parse(fs.readFileSync(sourceFile, 'utf8'));

// Recursively transform all string values to uppercase
function toUpperCase(obj) {
  if (typeof obj === 'string') {
    return obj.toUpperCase();
  }

  if (Array.isArray(obj)) {
    return obj.map(toUpperCase);
  }

  if (typeof obj === 'object' && obj !== null) {
    const result = {};
    for (const [key, value] of Object.entries(obj)) {
      result[key] = toUpperCase(value);
    }
    return result;
  }

  return obj;
}

// Generate the uppercase version
const uppercase = toUpperCase(source);

// Write to target file
fs.writeFileSync(targetFile, JSON.stringify(uppercase, null, 2) + '\n', 'utf8');

console.log(`✓ Generated ${targetFile}`);
console.log('  All strings have been converted to UPPERCASE');
console.log('  Use this language in settings to identify any hardcoded strings');
