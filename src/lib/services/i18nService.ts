/**
 * Internationalization service
 * Configures svelte-i18n for the application
 */

import { register, init, getLocaleFromNavigator } from 'svelte-i18n';

// Register available locales
register('en-GB', () => import('../../locales/en-GB.json'));
register('en-US', () => import('../../locales/en-US.json'));

// Available locales
export const AVAILABLE_LOCALES = [
  { code: 'en-GB', name: 'English (UK)' },
  { code: 'en-US', name: 'English (US)' },
];

// Default locale
export const DEFAULT_LOCALE = 'en-GB';

/**
 * Initialize i18n with a specific locale or auto-detect
 */
export function initI18n(locale?: string) {
  init({
    fallbackLocale: DEFAULT_LOCALE,
    initialLocale: locale || getLocaleFromNavigator() || DEFAULT_LOCALE,
  });
}

/**
 * Get the locale code from settings or default
 */
export function getInitialLocale(userLocale?: string): string {
  if (userLocale && AVAILABLE_LOCALES.some(l => l.code === userLocale)) {
    return userLocale;
  }

  const browserLocale = getLocaleFromNavigator();
  if (browserLocale && AVAILABLE_LOCALES.some(l => l.code === browserLocale)) {
    return browserLocale;
  }

  return DEFAULT_LOCALE;
}
