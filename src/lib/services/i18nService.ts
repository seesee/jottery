/**
 * Internationalization service
 * Configures svelte-i18n for the application
 */

import { register, init, getLocaleFromNavigator } from 'svelte-i18n';

// Register available locales
register('en-GB', () => import('../../locales/en-GB.json'));
register('en-US', () => import('../../locales/en-US.json'));
register('es', () => import('../../locales/es.json'));
register('fr', () => import('../../locales/fr.json'));
register('de', () => import('../../locales/de.json'));
register('it', () => import('../../locales/it.json'));
register('pt', () => import('../../locales/pt.json'));
register('ja', () => import('../../locales/ja.json'));
register('zh', () => import('../../locales/zh.json'));
register('ko', () => import('../../locales/ko.json'));
register('ru', () => import('../../locales/ru.json'));
register('nl', () => import('../../locales/nl.json'));
register('pl', () => import('../../locales/pl.json'));
register('tr', () => import('../../locales/tr.json'));

// Available locales
export const AVAILABLE_LOCALES = [
  { code: 'en-GB', name: 'English (UK)' },
  { code: 'en-US', name: 'English (US)' },
  { code: 'es', name: 'Español' },
  { code: 'fr', name: 'Français' },
  { code: 'de', name: 'Deutsch' },
  { code: 'it', name: 'Italiano' },
  { code: 'pt', name: 'Português' },
  { code: 'ja', name: '日本語' },
  { code: 'zh', name: '简体中文' },
  { code: 'ko', name: '한국어' },
  { code: 'ru', name: 'Русский' },
  { code: 'nl', name: 'Nederlands' },
  { code: 'pl', name: 'Polski' },
  { code: 'tr', name: 'Türkçe' },
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
