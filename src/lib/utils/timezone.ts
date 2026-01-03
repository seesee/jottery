/**
 * Timezone utilities for formatting timestamps
 */

import { derived, type Readable } from 'svelte/store';
import { locale } from 'svelte-i18n';
import { settings } from '../stores/appStore';

/**
 * Internal function to format timestamps
 */
function _formatTimestamp(
  isoString: string,
  format: 'full' | 'date' | 'time' | 'relative',
  currentLocale: string,
  timezone: string
): string {
  const date = new Date(isoString);

  // Get the timezone to use
  const tz = timezone === 'local' ? undefined : timezone;

  switch (format) {
    case 'full':
      return new Intl.DateTimeFormat(currentLocale, {
        timeZone: tz,
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      }).format(date);

    case 'date':
      return new Intl.DateTimeFormat(currentLocale, {
        timeZone: tz,
        year: 'numeric',
        month: 'short',
        day: 'numeric',
      }).format(date);

    case 'time':
      return new Intl.DateTimeFormat(currentLocale, {
        timeZone: tz,
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      }).format(date);

    case 'relative':
      return _formatRelativeTime(date, currentLocale);

    default:
      return isoString;
  }
}

/**
 * Format a timestamp in the user's selected timezone (reactive)
 * @param isoString ISO 8601 timestamp string (UTC)
 * @param format 'full' | 'date' | 'time' | 'relative'
 * @returns Reactive formatted timestamp store
 */
export function formatTimestamp(
  isoString: string,
  format: 'full' | 'date' | 'time' | 'relative' = 'full'
): Readable<string> {
  return derived([locale, settings], ([$locale, $settings]) => {
    const currentLocale = $locale || 'en-GB';
    const timezone = $settings.timezone || 'local';
    return _formatTimestamp(isoString, format, currentLocale, timezone);
  });
}

/**
 * Internal function to format relative time
 */
function _formatRelativeTime(date: Date, currentLocale: string): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  const rtf = new Intl.RelativeTimeFormat(currentLocale, { numeric: 'auto' });

  if (diffSec < 60) {
    return rtf.format(0, 'second'); // "now" or locale equivalent
  } else if (diffMin < 60) {
    return rtf.format(-diffMin, 'minute');
  } else if (diffHour < 24) {
    return rtf.format(-diffHour, 'hour');
  } else if (diffDay < 7) {
    return rtf.format(-diffDay, 'day');
  } else {
    // For older dates, show the actual date - but we need timezone too
    // Since we're inside the derived function, we can't call formatTimestamp
    // Just format as a simple date
    return new Intl.DateTimeFormat(currentLocale, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    }).format(date);
  }
}

/**
 * Get the current timezone name for display
 */
export function getTimezoneName(): string {
  const $settings = get(settings);
  const timezone = $settings.timezone || 'local';

  if (timezone === 'local') {
    // Get the browser's timezone
    return Intl.DateTimeFormat().resolvedOptions().timeZone;
  }

  return timezone;
}

/**
 * Get timezone offset string (e.g., "+00:00", "-05:00")
 */
export function getTimezoneOffset(isoString?: string): string {
  const $settings = get(settings);
  const timezone = $settings.timezone || 'local';
  const tz = timezone === 'local' ? undefined : timezone;

  const date = isoString ? new Date(isoString) : new Date();

  // Create a formatter to extract timezone offset
  const formatter = new Intl.DateTimeFormat('en-GB', {
    timeZone: tz,
    timeZoneName: 'longOffset',
  });

  const parts = formatter.formatToParts(date);
  const offsetPart = parts.find(p => p.type === 'timeZoneName');

  if (offsetPart && offsetPart.value.startsWith('GMT')) {
    const offset = offsetPart.value.replace('GMT', '');
    if (offset === '') return '+00:00';

    // Convert GMT+X or GMT-X to +XX:00 or -XX:00 format
    const match = offset.match(/([+-])(\d+)(?::(\d+))?/);
    if (match) {
      const sign = match[1];
      const hours = match[2].padStart(2, '0');
      const minutes = (match[3] || '00').padStart(2, '0');
      return `${sign}${hours}:${minutes}`;
    }
  }

  return '+00:00';
}
