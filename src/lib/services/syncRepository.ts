/**
 * Sync metadata repository for IndexedDB
 */

import type { SyncMetadata, NoteSyncMetadata } from '../types';
import { getDB, STORES } from './db';

const METADATA_KEY = 'metadata';

class IndexedDBSyncRepository {
  /**
   * Get global sync metadata
   */
  async getMetadata(): Promise<SyncMetadata | null> {
    console.log('[SyncRepo] Getting sync metadata...');
    const db = getDB();
    console.log(`[SyncRepo] DB version: ${db.version}, stores: ${[...db.objectStoreNames].join(', ')}`);

    try {
      const metadata = await db.get(STORES.SYNC_METADATA, METADATA_KEY);
      console.log('[SyncRepo] Metadata retrieved:', metadata ? 'found' : 'null');
      return metadata || null;
    } catch (error) {
      console.error('[SyncRepo] Error getting metadata:', error);
      throw error;
    }
  }

  /**
   * Update global sync metadata
   */
  async updateMetadata(metadata: Partial<SyncMetadata>): Promise<SyncMetadata> {
    console.log('[SyncRepo] Updating sync metadata...', metadata);
    const db = getDB();
    const current = await this.getMetadata() || {
      syncEnabled: false,
      syncEndpoint: '',
    };
    const updated = { ...current, ...metadata };
    console.log('[SyncRepo] Updated metadata:', updated);

    try {
      await db.put(STORES.SYNC_METADATA, updated, METADATA_KEY);
      console.log('[SyncRepo] Metadata saved successfully');
      return updated;
    } catch (error) {
      console.error('[SyncRepo] Error updating metadata:', error);
      throw error;
    }
  }

  /**
   * Get sync metadata for a specific note
   */
  async getNoteSyncMetadata(noteId: string): Promise<NoteSyncMetadata | null> {
    const db = getDB();
    const metadata = await db.get(STORES.SYNC_METADATA, `note:${noteId}`);
    return metadata || null;
  }

  /**
   * Update sync metadata for a specific note
   */
  async updateNoteSyncMetadata(noteId: string, metadata: Partial<NoteSyncMetadata>): Promise<void> {
    const db = getDB();
    const current = await this.getNoteSyncMetadata(noteId) || {
      noteId,
      syncedAt: new Date().toISOString(),
      syncHash: '',
      serverVersion: 1,
      lastSyncStatus: 'pending' as const,
    };
    const updated = { ...current, ...metadata };
    await db.put(STORES.SYNC_METADATA, updated, `note:${noteId}`);
  }

  /**
   * Get all notes with pending sync
   */
  async getPendingNotes(): Promise<string[]> {
    const db = getDB();
    const tx = db.transaction(STORES.SYNC_METADATA, 'readonly');
    const allKeys = await tx.store.getAllKeys();
    const noteKeys = allKeys.filter(key => typeof key === 'string' && key.startsWith('note:'));

    const pendingIds: string[] = [];
    for (const key of noteKeys) {
      const metadata = await db.get(STORES.SYNC_METADATA, key);
      if (metadata && metadata.lastSyncStatus === 'pending') {
        pendingIds.push(metadata.noteId);
      }
    }

    return pendingIds;
  }

  /**
   * Get count of notes with conflicts
   */
  async getConflictCount(): Promise<number> {
    const db = getDB();
    const tx = db.transaction(STORES.SYNC_METADATA, 'readonly');
    const allKeys = await tx.store.getAllKeys();
    const noteKeys = allKeys.filter(key => typeof key === 'string' && key.startsWith('note:'));

    let count = 0;
    for (const key of noteKeys) {
      const metadata = await db.get(STORES.SYNC_METADATA, key);
      if (metadata && metadata.lastSyncStatus === 'conflict') {
        count++;
      }
    }

    return count;
  }

  /**
   * Clear all sync metadata (for reset/re-registration)
   */
  async clearAll(): Promise<void> {
    const db = getDB();
    const tx = db.transaction(STORES.SYNC_METADATA, 'readwrite');
    await tx.store.clear();
    await tx.done;
  }

  /**
   * Delete sync metadata for a specific note
   */
  async deleteNoteSyncMetadata(noteId: string): Promise<void> {
    const db = getDB();
    await db.delete(STORES.SYNC_METADATA, `note:${noteId}`);
  }
}

export const syncRepository = new IndexedDBSyncRepository();
