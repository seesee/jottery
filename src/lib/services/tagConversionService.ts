/**
 * Tag conversion service for converting between storage and sync formats
 *
 * Storage format: Single encrypted blob containing JSON array of tags
 *   tags[0] = JSON.stringify(encryptedBlob)
 *
 * Sync format: Array of individually encrypted tags
 *   tags = [encryptedTag1, encryptedTag2, ...]
 */

import { cryptoService, encryptJSON, decryptJSON } from './crypto';

/**
 * Validate a tag value - must be non-empty string
 */
function isValidTag(tag: unknown): tag is string {
  return typeof tag === 'string' && tag.trim().length > 0;
}

/**
 * Read tags in either format. Clients have written both to version history, and
 * one tag in the sync format looks exactly like the storage format, so the only
 * way to tell them apart is by what they decrypt to. A tag that won't decrypt is
 * skipped.
 */
export async function decryptTags(tags: string[] | undefined, key: CryptoKey): Promise<string[]> {
  const decrypted: string[] = [];

  for (const encryptedTagJson of tags ?? []) {
    try {
      const value: unknown = await decryptJSON(JSON.parse(encryptedTagJson), key);
      for (const tag of Array.isArray(value) ? value : [value]) {
        if (isValidTag(tag)) {
          decrypted.push(tag);
        } else {
          console.warn('[TagConversion] Skipping invalid tag');
        }
      }
    } catch (error) {
      console.error('[TagConversion] Failed to decrypt tag:', error);
    }
  }

  return decrypted;
}

/**
 * Convert tags from storage format (single encrypted blob) to sync format (individually encrypted tags)
 *
 * @param storageTags - Tags in storage format (array with single JSON-stringified encrypted blob)
 * @param key - Encryption key
 * @returns Tags in sync format (array of JSON-stringified encrypted strings)
 * @throws if the tags can't be read. An empty list would erase them on the server.
 */
export async function storageToSync(
  storageTags: string[] | undefined,
  key: CryptoKey
): Promise<string[]> {
  // Handle empty/missing tags
  if (!storageTags || storageTags.length === 0) {
    return [];
  }

  // Multiple entries are already in the sync format
  if (storageTags.length > 1) {
    return storageTags;
  }

  const decrypted: unknown = await decryptJSON(JSON.parse(storageTags[0]), key);
  const validTags = (Array.isArray(decrypted) ? decrypted : [decrypted]).filter(isValidTag);

  // Encrypt each valid tag individually with JSON encoding (as required by sync protocol)
  return Promise.all(
    validTags.map(async tag => JSON.stringify(await cryptoService.encryptText(JSON.stringify(tag), key)))
  );
}

/**
 * Convert tags to storage format (a single encrypted blob). Accepts either format.
 *
 * @param syncTags - Tags, normally in sync format (array of JSON-stringified encrypted strings)
 * @param key - Encryption key
 * @returns Tags in storage format (array with single JSON-stringified encrypted blob)
 */
export async function syncToStorage(
  syncTags: string[] | undefined,
  key: CryptoKey
): Promise<string[]> {
  const decryptedTags = await decryptTags(syncTags, key);

  // Re-encrypt as a single blob for storage
  if (decryptedTags.length === 0) {
    return [];
  }

  const encryptedTagsBlob = await encryptJSON(decryptedTags, key);
  return [JSON.stringify(encryptedTagsBlob)];
}

export const tagConversionService = {
  decryptTags,
  storageToSync,
  syncToStorage,
};
