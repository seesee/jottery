/**
 * Timestamps from outside the app. A note's dates are normally ISO strings, but a
 * value from an imported file or an older vault can be anything, and one bad value
 * must never stop the note list loading.
 */

const MAX_DATE_MS = 8.64e15;

/** An ISO string for a date or epoch milliseconds, or null if it isn't a usable date. */
export function toIsoTimestamp(value: unknown): string | null {
  if (typeof value !== 'string' && typeof value !== 'number') return null;

  const time = typeof value === 'number' ? value : Date.parse(value);
  if (!Number.isFinite(time) || Math.abs(time) > MAX_DATE_MS) return null;

  return new Date(time).toISOString();
}

/**
 * A string the UI can safely treat as a date. Strings pass through untouched, because
 * rewriting one could lose precision and make it differ from the stored value.
 */
export function asTimestampString(value: unknown): string {
  return typeof value === 'string' ? value : (toIsoTimestamp(value) ?? new Date(0).toISOString());
}

/** Compare two timestamps, tolerating values that aren't strings. */
export function compareTimestamps(a: unknown, b: unknown): number {
  return String(a ?? '').localeCompare(String(b ?? ''));
}
