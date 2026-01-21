/**
 * Saved Search repository for IndexedDB operations
 */

import type { SavedSearch } from '../types';
import { getDB, STORES } from './db';
import { v4 as uuidv4 } from 'uuid';

/**
 * Create a new saved search
 */
export async function create(name: string, query: string): Promise<SavedSearch> {
  const db = getDB();
  const now = new Date().toISOString();

  // Get max order
  const all = await getAll();
  const maxOrder = all.length > 0 ? Math.max(...all.map(s => s.order)) : 0;

  const savedSearch: SavedSearch = {
    id: uuidv4(),
    name,
    query,
    order: maxOrder + 1,
    createdAt: now,
    modifiedAt: now,
    deleted: false,
    version: 1,
    needsSync: true,
  };

  await db.put(STORES.SAVED_SEARCHES, savedSearch);
  return savedSearch;
}

/**
 * Get all non-deleted saved searches, ordered by order field
 */
export async function getAll(): Promise<SavedSearch[]> {
  const db = getDB();
  const searches = await db.getAll(STORES.SAVED_SEARCHES);
  return searches
    .filter(s => !s.deleted)
    .sort((a, b) => a.order - b.order);
}

/**
 * Get a saved search by ID
 */
export async function getById(id: string): Promise<SavedSearch | undefined> {
  const db = getDB();
  return await db.get(STORES.SAVED_SEARCHES, id);
}

/**
 * Update a saved search
 */
export async function update(id: string, updates: Partial<SavedSearch>): Promise<SavedSearch> {
  const db = getDB();
  const existing = await getById(id);
  if (!existing) throw new Error('SavedSearch not found');

  const updated: SavedSearch = {
    ...existing,
    ...updates,
    modifiedAt: new Date().toISOString(),
    version: existing.version + 1,
    needsSync: true,
  };

  await db.put(STORES.SAVED_SEARCHES, updated);
  return updated;
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
  const db = getDB();
  for (let i = 0; i < orderedIds.length; i++) {
    const savedSearch = await getById(orderedIds[i]);
    if (savedSearch) {
      await update(orderedIds[i], { order: i + 1 });
    }
  }
}

/**
 * Get all saved searches that need to be synced
 */
export async function getAllNeedingSync(): Promise<SavedSearch[]> {
  const db = getDB();
  const searches = await db.getAll(STORES.SAVED_SEARCHES);
  return searches.filter(s => s.needsSync === true);
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
