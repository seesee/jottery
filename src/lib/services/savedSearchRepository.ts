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
    // Decrypt name
    let name: string;
    try {
      const encryptedName = JSON.parse(savedSearch.name);
      name = await cryptoService.decryptText(encryptedName, masterKey.key);
    } catch {
      // Handle unencrypted data (legacy)
      name = savedSearch.name;
    }

    // Decrypt query
    let query: string;
    try {
      const encryptedQuery = JSON.parse(savedSearch.query);
      query = await cryptoService.decryptText(encryptedQuery, masterKey.key);
    } catch {
      // Handle unencrypted data (legacy)
      query = savedSearch.query;
    }

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
    const savedSearch = await getById(orderedIds[i]);
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
 * Mark saved search as synced
 */
export async function markSynced(id: string): Promise<void> {
  const db = getDB();
  const savedSearch = await getById(id);
  if (!savedSearch) return;

  const updated: SavedSearch = {
    ...savedSearch,
    syncedAt: new Date().toISOString(),
    needsSync: false,
  };

  await db.put(STORES.SAVED_SEARCHES, updated);
}

export const savedSearchRepository = {
  create,
  getAll,
  getById,
  update,
  softDelete,
  permanentDelete,
  reorder,
  getAllNeedingSync,
  markSynced,
};
