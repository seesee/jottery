/**
 * Date formatting utilities using Intl API for proper i18n
 */

import { get } from 'svelte/store';
import { locale } from 'svelte-i18n';

/**
 * Format a date using the current locale
 */
export function formatDate(date: Date | string, options?: Intl.DateTimeFormatOptions): string {
  const dateObj = typeof date === 'string' ? new Date(date) : date;
  const currentLocale = get(locale) || 'en-GB';

  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    ...options,
  };

  return new Intl.DateTimeFormat(currentLocale, defaultOptions).format(dateObj);
}

/**
 * Format a date as short date (no time)
 */
export function formatDateShort(date: Date | string): string {
  return formatDate(date, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  });
}

/**
 * Format a date as time only
 */
export function formatTime(date: Date | string): string {
  return formatDate(date, {
    hour: '2-digit',
    minute: '2-digit',
  });
}

/**
 * Format a date as full date and time
 */
export function formatDateTime(date: Date | string): string {
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
export function formatRelativeTime(date: Date | string): string {
  const dateObj = typeof date === 'string' ? new Date(date) : date;
  const now = new Date();
  const diffInMs = now.getTime() - dateObj.getTime();
  const diffInSeconds = Math.floor(diffInMs / 1000);
  const diffInMinutes = Math.floor(diffInSeconds / 60);
  const diffInHours = Math.floor(diffInMinutes / 60);
  const diffInDays = Math.floor(diffInHours / 24);

  const currentLocale = get(locale) || 'en-GB';
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
    return formatDate(dateObj);
  }
}
