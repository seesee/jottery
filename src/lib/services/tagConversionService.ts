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
 * Convert tags from storage format (single encrypted blob) to sync format (individually encrypted tags)
 *
 * @param storageTags - Tags in storage format (array with single JSON-stringified encrypted blob)
 * @param key - Encryption key
 * @returns Tags in sync format (array of JSON-stringified encrypted strings)
 */
export async function storageToSync(
  storageTags: string[] | undefined,
  key: CryptoKey
): Promise<string[]> {
  // Handle empty/missing tags
  if (!storageTags || storageTags.length === 0) {
    return [];
  }

  // Single-blob format: tags[0] is a JSON string containing encrypted array
  if (storageTags.length === 1) {
    try {
      const encryptedTagsBlob = JSON.parse(storageTags[0]);
      const decryptedTagsArray = await decryptJSON<string[]>(encryptedTagsBlob, key);

      // Filter out invalid tags (empty strings, non-strings, etc.)
      const validTags = decryptedTagsArray.filter(tag => {
        if (!isValidTag(tag)) {
          console.warn('[TagConversion] Skipping invalid tag:', tag);
          return false;
        }
        return true;
      });

      // Encrypt each valid tag individually with JSON encoding (as required by sync protocol)
      return Promise.all(
        validTags.map(async tag => {
          const tagJson = JSON.stringify(tag); // JSON-encode the tag
          const encrypted = await cryptoService.encryptText(tagJson, key);
          return JSON.stringify(encrypted); // Stringify the EncryptionResult
        })
      );
    } catch (error) {
      console.error('[TagConversion] Failed to parse storage tags:', error);
      return [];
    }
  }

  // Multiple entries (legacy/unexpected format) - pass through as-is
  console.warn('[TagConversion] Unexpected multiple tag entries, using as-is');
  return storageTags;
}

/**
 * Convert tags from sync format (individually encrypted tags) to storage format (single encrypted blob)
 *
 * @param syncTags - Tags in sync format (array of JSON-stringified encrypted strings)
 * @param key - Encryption key
 * @returns Tags in storage format (array with single JSON-stringified encrypted blob)
 */
export async function syncToStorage(
  syncTags: string[] | undefined,
  key: CryptoKey
): Promise<string[]> {
  // Handle empty/missing tags
  if (!syncTags || syncTags.length === 0) {
    return [];
  }

  // Decrypt each individually encrypted tag
  const decryptedTags: string[] = [];

  for (const encryptedTagJson of syncTags) {
    try {
      const encryptedTag = JSON.parse(encryptedTagJson);
      const tagJson = await cryptoService.decryptText(encryptedTag, key);
      const tag = JSON.parse(tagJson); // Parse the JSON-encoded tag

      // Handle legacy format: if tag is an array, extract individual tags
      if (Array.isArray(tag)) {
        console.warn('[TagConversion] Found legacy array tag format, extracting individual tags');
        for (const individualTag of tag) {
          if (isValidTag(individualTag)) {
            decryptedTags.push(individualTag);
          }
        }
        continue;
      }

      // Validate and add single tag
      if (isValidTag(tag)) {
        decryptedTags.push(tag);
      } else {
        console.warn('[TagConversion] Skipping invalid tag:', tag);
      }
    } catch (error) {
      console.error('[TagConversion] Failed to decrypt tag:', error);
    }
  }

  // Re-encrypt as a single blob for storage
  if (decryptedTags.length === 0) {
    return [];
  }

  const encryptedTagsBlob = await encryptJSON(decryptedTags, key);
  return [JSON.stringify(encryptedTagsBlob)];
}

export const tagConversionService = {
  storageToSync,
  syncToStorage,
};
