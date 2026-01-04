/**
 * Date formatting utilities using Intl API for proper i18n and timezone support
 */

import { derived, type Readable } from 'svelte/store';
import { locale } from 'svelte-i18n';
import { settings } from '../stores/appStore';

/**
 * Internal function to format dates with locale and timezone
 */
function _formatDate(
  date: Date | string,
  currentLocale: string,
  timezone: string,
  options?: Intl.DateTimeFormatOptions
): string {
  const dateObj = typeof date === 'string' ? new Date(date) : date;

  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    timeZone: timezone === 'local' ? undefined : timezone,
    ...options,
  };

  return new Intl.DateTimeFormat(currentLocale, defaultOptions).format(dateObj);
}

/**
 * Create a reactive formatted date store
 */
export function formatDate(date: Date | string, options?: Intl.DateTimeFormatOptions): Readable<string> {
  return derived([locale, settings], ([$locale, $settings]) => {
    const currentLocale = $locale || 'en-GB';
    const timezone = $settings.timezone || 'local';
    return _formatDate(date, currentLocale, timezone, options);
  });
}

/**
 * Format a date as short date (no time)
 */
export function formatDateShort(date: Date | string): Readable<string> {
  return formatDate(date, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  });
}

/**
 * Format a date as time only
 */
export function formatTime(date: Date | string): Readable<string> {
  return formatDate(date, {
    hour: '2-digit',
    minute: '2-digit',
  });
}

/**
 * Format a date as full date and time
 */
export function formatDateTime(date: Date | string): Readable<string> {
  return formatDate(date, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

/**
 * Format a relative time (e.g., "2 hours ago")
 */
export function formatRelativeTime(date: Date | string): Readable<string> {
  return derived([locale, settings], ([$locale, $settings]) => {
    const dateObj = typeof date === 'string' ? new Date(date) : date;
    const now = new Date();
    const diffInMs = now.getTime() - dateObj.getTime();
    const diffInSeconds = Math.floor(diffInMs / 1000);
    const diffInMinutes = Math.floor(diffInSeconds / 60);
    const diffInHours = Math.floor(diffInMinutes / 60);
    const diffInDays = Math.floor(diffInHours / 24);

    const currentLocale = $locale || 'en-GB';
    const rtf = new Intl.RelativeTimeFormat(currentLocale, { numeric: 'auto' });

    if (diffInSeconds < 60) {
      return rtf.format(-diffInSeconds, 'second');
    } else if (diffInMinutes < 60) {
      return rtf.format(-diffInMinutes, 'minute');
    } else if (diffInHours < 24) {
      return rtf.format(-diffInHours, 'hour');
    } else if (diffInDays < 30) {
      return rtf.format(-diffInDays, 'day');
    } else {
      // For dates older than 30 days, show absolute date
      const timezone = $settings.timezone || 'local';
      return _formatDate(dateObj, currentLocale, timezone);
    }
  });
}
