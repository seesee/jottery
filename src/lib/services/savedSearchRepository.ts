/**
 * Saved Search repository for IndexedDB operations
 */

import type { SavedSearch } from '../types';
import { getDB, STORES } from './db';
import { v4 as uuidv4 } from 'uuid';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { ApplicationLockedError, NotFoundError } from '../errors';

/**
 * Create a new saved search
 */
export async function create(name: string, query: string): Promise<SavedSearch> {
  const db = getDB();
  const now = new Date().toISOString();

  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  // Get max order
  const all = await getAll();
  const maxOrder = all.length > 0 ? Math.max(...all.map(s => s.order)) : 0;

  // Encrypt sensitive fields
  const encryptedName = await cryptoService.encryptText(name, masterKey.key);
  const encryptedQuery = await cryptoService.encryptText(query, masterKey.key);

  const savedSearch: SavedSearch = {
    id: uuidv4(),
    name: JSON.stringify(encryptedName),
    query: JSON.stringify(encryptedQuery),
    order: maxOrder + 1,
    createdAt: now,
    modifiedAt: now,
    deleted: false,
    version: 1,
    needsSync: true,
  };

  await db.put(STORES.SAVED_SEARCHES, savedSearch);

  // Return decrypted version for use in app
  return {
    ...savedSearch,
    name,
    query,
  };
}

/**
 * Decrypt a saved search
 */
async function decryptSavedSearch(savedSearch: SavedSearch): Promise<SavedSearch> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  try {
    const name = await cryptoService.decryptText(JSON.parse(savedSearch.name), masterKey.key);
    const query = await cryptoService.decryptText(JSON.parse(savedSearch.query), masterKey.key);

    return {
      ...savedSearch,
      name,
      query,
    };
  } catch (error) {
    console.error('Failed to decrypt saved search:', error);
    // Return with [Encrypted] placeholder if decryption fails
    return {
      ...savedSearch,
      name: '[Encrypted]',
      query: '[Encrypted]',
    };
  }
}

/** Whether a stored field has the shape of ciphertext, as opposed to plaintext */
function isEncryptedField(value: string): boolean {
  try {
    const parsed = JSON.parse(value);
    return typeof parsed?.ciphertext === 'string' && typeof parsed?.iv === 'string';
  } catch {
    return false;
  }
}

/**
 * Whether this vault can read a row. Rows from the sync server must pass this: a
 * row that doesn't decrypt came from something without the key.
 */
export async function isReadable(savedSearch: Pick<SavedSearch, 'name' | 'query'>): Promise<boolean> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  try {
    await cryptoService.decryptText(JSON.parse(savedSearch.name), masterKey.key);
    await cryptoService.decryptText(JSON.parse(savedSearch.query), masterKey.key);
    return true;
  } catch {
    return false;
  }
}

/**
 * Encrypt rows stored as plaintext. markSynced used to write decrypted rows back,
 * so these exist on real devices, and a later reorder or delete could have pushed
 * them to the server.
 *
 * Each row is queued for sync with a new timestamp, which replaces the plaintext
 * copy on the server. Plaintext can't be told apart from a value a server planted
 * before rows from the server were checked, so this runs once only, behind a flag
 * held by the caller. After that, plaintext is an error like any other.
 *
 * @returns the number of rows changed
 */
export async function encryptLegacyRows(): Promise<number> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  const db = getDB();
  const rows = await db.getAll(STORES.SAVED_SEARCHES);
  let changed = 0;

  for (const row of rows) {
    const nameEncrypted = isEncryptedField(row.name);
    const queryEncrypted = isEncryptedField(row.query);
    if (nameEncrypted && queryEncrypted) continue;

    await db.put(STORES.SAVED_SEARCHES, {
      ...row,
      name: nameEncrypted ? row.name : JSON.stringify(await cryptoService.encryptText(row.name, masterKey.key)),
      query: queryEncrypted ? row.query : JSON.stringify(await cryptoService.encryptText(row.query, masterKey.key)),
      modifiedAt: new Date().toISOString(),
      version: row.version + 1,
      needsSync: true,
    });
    changed++;
  }

  return changed;
}

/**
 * Get all non-deleted saved searches, ordered by order field
 */
export async function getAll(): Promise<SavedSearch[]> {
  try {
    const db = getDB();
    const searches = await db.getAll(STORES.SAVED_SEARCHES);
    const filtered = searches
      .filter(s => !s.deleted)
      .sort((a, b) => a.order - b.order);

    // Decrypt all searches (skip any that fail)
    const results = await Promise.allSettled(filtered.map(s => decryptSavedSearch(s)));
    const decrypted: SavedSearch[] = [];
    for (const result of results) {
      if (result.status === 'fulfilled') {
        decrypted.push(result.value);
      } else {
        console.error('[savedSearchRepository] Skipping undecryptable saved search:', result.reason);
      }
    }
    return decrypted;
  } catch (error) {
    // If saved_searches store doesn't exist yet (pre-v7 database), return empty array
    console.warn('[savedSearchRepository] Store not available yet:', error);
    return [];
  }
}

/**
 * Get a saved search by ID
 */
export async function getById(id: string): Promise<SavedSearch | undefined> {
  const db = getDB();
  const savedSearch = await db.get(STORES.SAVED_SEARCHES, id);
  if (!savedSearch) return undefined;
  return await decryptSavedSearch(savedSearch);
}

/**
 * Get a row exactly as stored, without decrypting it. For anything that only needs
 * the plaintext metadata, or that writes the row back: a decrypted copy must never
 * reach the database.
 */
export async function getStoredById(id: string): Promise<SavedSearch | undefined> {
  return await getDB().get(STORES.SAVED_SEARCHES, id);
}

/**
 * Update a saved search
 */
export async function update(id: string, updates: Partial<SavedSearch>): Promise<SavedSearch> {
  const db = getDB();
  const existing = await db.get(STORES.SAVED_SEARCHES, id); // Get encrypted version from DB
  if (!existing) throw new NotFoundError('SavedSearch');

  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  // Encrypt updated fields if provided
  const encryptedUpdates: Partial<SavedSearch> = { ...updates };

  if (updates.name !== undefined) {
    const encryptedName = await cryptoService.encryptText(updates.name, masterKey.key);
    encryptedUpdates.name = JSON.stringify(encryptedName);
  }

  if (updates.query !== undefined) {
    const encryptedQuery = await cryptoService.encryptText(updates.query, masterKey.key);
    encryptedUpdates.query = JSON.stringify(encryptedQuery);
  }

  const updated: SavedSearch = {
    ...existing,
    ...encryptedUpdates,
    modifiedAt: new Date().toISOString(),
    version: existing.version + 1,
    needsSync: true,
  };

  await db.put(STORES.SAVED_SEARCHES, updated);

  // Return decrypted version
  return await decryptSavedSearch(updated);
}

/**
 * Soft delete a saved search
 */
export async function softDelete(id: string): Promise<void> {
  await update(id, {
    deleted: true,
    deletedAt: new Date().toISOString(),
  });
}

/**
 * Permanently delete a saved search
 */
export async function permanentDelete(id: string): Promise<void> {
  const db = getDB();
  await db.delete(STORES.SAVED_SEARCHES, id);
}

/**
 * Reorder saved searches
 */
export async function reorder(orderedIds: string[]): Promise<void> {
  for (let i = 0; i < orderedIds.length; i++) {
    const savedSearch = await getStoredById(orderedIds[i]);
    if (savedSearch) {
      await update(orderedIds[i], { order: i + 1 });
    }
  }
}

/**
 * Get all saved searches that need to be synced
 * Returns encrypted data for transmission to server
 */
export async function getAllNeedingSync(): Promise<SavedSearch[]> {
  try {
    const db = getDB();
    const searches = await db.getAll(STORES.SAVED_SEARCHES);
    // Return encrypted data as-is for sync (server stores encrypted)
    return searches.filter(s => s.needsSync === true);
  } catch (error) {
    // If saved_searches store doesn't exist yet (pre-v7 database), return empty array
    console.warn('[savedSearchRepository] Store not available yet:', error);
    return [];
  }
}

/**
 * Mark saved search as synced. This works on the stored row, so the name and query
 * stay encrypted.
 *
 * @param pushedVersion - The version that was pushed. Every update raises it, so a
 *                        row with a different one changed during the push and still
 *                        needs syncing. A timestamp can't show this: two writes can
 *                        share a millisecond.
 */
export async function markSynced(id: string, pushedVersion?: number): Promise<void> {
  const db = getDB();
  const savedSearch = await getStoredById(id);
  if (!savedSearch) return;

  const changedSincePush = pushedVersion !== undefined && savedSearch.version !== pushedVersion;

  await db.put(STORES.SAVED_SEARCHES, {
    ...savedSearch,
    syncedAt: new Date().toISOString(),
    needsSync: changedSincePush,
  });
}

export const savedSearchRepository = {
  create,
  encryptLegacyRows,
  getAll,
  getStoredById,
  isReadable,
  getById,
  update,
  softDelete,
  permanentDelete,
  reorder,
  getAllNeedingSync,
  markSynced,
};
