/**
 * Encoding for a password kept in browser storage. This is obfuscation, not
 * encryption, and exists only so the value isn't readable at a glance.
 *
 * btoa only accepts characters up to U+00FF, so it can't encode a Russian, Greek or
 * Japanese password. Values are stored as base64 of the UTF-8 bytes instead, under a
 * prefix that can't occur in base64. Anything without it was written by an older
 * version, and is decoded the old way.
 */

const UTF8_PREFIX = 'u8:';

export function encodeStoredPassword(password: string): string {
  const bytes = new TextEncoder().encode(password);
  return UTF8_PREFIX + btoa(String.fromCharCode(...bytes));
}

/** @throws if the value is corrupt */
export function decodeStoredPassword(value: string): string {
  if (!value.startsWith(UTF8_PREFIX)) {
    return atob(value);
  }

  const bytes = Uint8Array.from(atob(value.slice(UTF8_PREFIX.length)), char => char.charCodeAt(0));
  return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
}
