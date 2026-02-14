/**
 * Unicode-safe string comparison and normalisation utilities
 *
 * All user-facing text comparisons should go through these helpers
 * to handle accented characters, Turkish İ/i, composed vs decomposed
 * Unicode forms, and locale-aware sorting correctly.
 */

/**
 * Normalise a string for case-insensitive comparison.
 * Applies NFC normalisation (canonical composition) and locale-aware lowercasing.
 *
 * Use this when you need to compare two user-entered strings for equality.
 */
export function normaliseForComparison(s: string): string {
  return s.normalize('NFC').toLocaleLowerCase();
}

/**
 * Check if two strings are equal, ignoring case and Unicode normalisation.
 *
 * Equivalent to `normaliseForComparison(a) === normaliseForComparison(b)`
 * but reads more clearly at call sites.
 */
export function localeEquals(a: string, b: string): boolean {
  return normaliseForComparison(a) === normaliseForComparison(b);
}

/**
 * Check if `haystack` contains `needle`, ignoring case and Unicode normalisation.
 *
 * Use this instead of `haystack.toLowerCase().includes(needle.toLowerCase())`.
 */
export function localeIncludes(haystack: string, needle: string): boolean {
  return normaliseForComparison(haystack).includes(normaliseForComparison(needle));
}

/**
 * Locale-aware string comparison for sorting user-visible text.
 * Returns negative, zero, or positive like `localeCompare`.
 *
 * Uses `sensitivity: 'base'` so accented and unaccented variants
 * sort next to each other (e.g. "café" near "cafe").
 */
export function localeSort(a: string, b: string): number {
  return a.normalize('NFC').localeCompare(b.normalize('NFC'), undefined, {
    sensitivity: 'base',
    numeric: true,
  });
}
