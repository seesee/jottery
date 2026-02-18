/**
 * Internationalization service
 * Configures svelte-i18n for the application
 */

import { register, init, getLocaleFromNavigator } from 'svelte-i18n';

// Register available locales (alphabetical by English name, except English variants first)
register('en-GB', () => import('../../locales/en-GB.json'));
register('en-US', () => import('../../locales/en-US.json'));
register('zh', () => import('../../locales/zh.json'));
register('nl', () => import('../../locales/nl.json'));
register('fr', () => import('../../locales/fr.json'));
register('de', () => import('../../locales/de.json'));
register('el', () => import('../../locales/el.json'));
register('it', () => import('../../locales/it.json'));
register('ja', () => import('../../locales/ja.json'));
register('ko', () => import('../../locales/ko.json'));
register('pl', () => import('../../locales/pl.json'));
register('pt', () => import('../../locales/pt.json'));
register('ru', () => import('../../locales/ru.json'));
register('es', () => import('../../locales/es.json'));
register('tr', () => import('../../locales/tr.json'));

// Available locales (alphabetical by English name, except English variants first)
export const AVAILABLE_LOCALES = [
  { code: 'en-GB', name: '🇬🇧 English (UK)' },
  { code: 'en-US', name: '🇺🇸 English (US)' },
  { code: 'zh', name: '🇨🇳 简体中文' },
  { code: 'nl', name: '🇳🇱 Nederlands' },
  { code: 'fr', name: '🇫🇷 Français' },
  { code: 'de', name: '🇩🇪 Deutsch' },
  { code: 'el', name: '🇬🇷 Ελληνικά' },
  { code: 'it', name: '🇮🇹 Italiano' },
  { code: 'ja', name: '🇯🇵 日本語' },
  { code: 'ko', name: '🇰🇷 한국어' },
  { code: 'pl', name: '🇵🇱 Polski' },
  { code: 'pt', name: '🇵🇹 Português' },
  { code: 'ru', name: '🇷🇺 Русский' },
  { code: 'es', name: '🇪🇸 Español' },
  { code: 'tr', name: '🇹🇷 Türkçe' },
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
 * Priority: 1) Saved setting, 2) Landing page selection (localStorage), 3) Browser detection, 4) Default
 */
export function getInitialLocale(userLocale?: string): string {
  // If user explicitly selected a locale in settings (not 'auto' or empty), use it
  if (userLocale && userLocale !== 'auto' && AVAILABLE_LOCALES.some(l => l.code === userLocale)) {
    return userLocale;
  }

  // Check if user selected a language on the landing page before setup completed
  // (LanguagePicker stores this in localStorage as a bridge to persist through unlock)
  try {
    const landingLocale = localStorage.getItem('jottery-locale');
    if (landingLocale && AVAILABLE_LOCALES.some(l => l.code === landingLocale)) {
      return landingLocale;
    }
  } catch {
    // Ignore localStorage errors (privacy mode, file:// URLs, etc.)
  }

  // Auto-detect from browser
  const browserLocale = getLocaleFromNavigator();
  if (browserLocale && AVAILABLE_LOCALES.some(l => l.code === browserLocale)) {
    return browserLocale;
  }

  // Try to match just the language part (e.g., 'en' from 'en-AU')
  if (browserLocale) {
    const langPart = browserLocale.split('-')[0];
    const matchingLocale = AVAILABLE_LOCALES.find(l => l.code.startsWith(langPart + '-') || l.code === langPart);
    if (matchingLocale) {
      return matchingLocale.code;
    }
  }

  return DEFAULT_LOCALE;
}

/**
 * Get the detected browser locale (for display purposes)
 */
export function getDetectedBrowserLocale(): string {
  const browserLocale = getLocaleFromNavigator();
  if (browserLocale && AVAILABLE_LOCALES.some(l => l.code === browserLocale)) {
    return browserLocale;
  }

  // Try to match just the language part
  if (browserLocale) {
    const langPart = browserLocale.split('-')[0];
    const matchingLocale = AVAILABLE_LOCALES.find(l => l.code.startsWith(langPart + '-') || l.code === langPart);
    if (matchingLocale) {
      return matchingLocale.code;
    }
  }

  return DEFAULT_LOCALE;
}
