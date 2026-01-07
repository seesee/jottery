// Documentation loader with locale fallback support
// Each locale has its own documentation file

import { documentation as enGB } from './en-GB';

// Lazy load other locales to avoid bundling all languages
const documentationModules: Record<string, () => Promise<{ documentation: string }>> = {
  'en-GB': () => Promise.resolve({ documentation: enGB }),
  'en-US': () => import('./en-US'),
  'de': () => import('./de'),
  'el': () => import('./el'),
  'es': () => import('./es'),
  'fr': () => import('./fr'),
  'it': () => import('./it'),
  'ja': () => import('./ja'),
  'ko': () => import('./ko'),
  'nl': () => import('./nl'),
  'pl': () => import('./pl'),
  'pt': () => import('./pt'),
  'ru': () => import('./ru'),
  'tr': () => import('./tr'),
  'zh': () => import('./zh'),
};

// Default fallback documentation (English)
export const defaultDocumentation = enGB;

/**
 * Load documentation for a given locale with fallback to English
 */
export async function loadDocumentation(locale: string): Promise<string> {
  // Try exact locale match first
  if (documentationModules[locale]) {
    try {
      const module = await documentationModules[locale]();
      return module.documentation;
    } catch (e) {
      console.warn(`Failed to load documentation for locale ${locale}, falling back to English`);
    }
  }

  // Try language code only (e.g., 'en' from 'en-GB')
  const langCode = locale.split('-')[0];
  if (langCode !== locale && documentationModules[langCode]) {
    try {
      const module = await documentationModules[langCode]();
      return module.documentation;
    } catch (e) {
      console.warn(`Failed to load documentation for language ${langCode}, falling back to English`);
    }
  }

  // Fallback to English
  return defaultDocumentation;
}

/**
 * Check if documentation exists for a locale (including fallbacks)
 */
export function hasDocumentation(locale: string): boolean {
  if (documentationModules[locale]) return true;
  const langCode = locale.split('-')[0];
  return langCode !== locale && !!documentationModules[langCode];
}
