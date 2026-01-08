/**
 * Note repository implementation for IndexedDB
 */

import type { NoteRepository } from '../types';
import type { Note } from '../types';
import { getDB, STORES } from './db';

class IndexedDBNoteRepository implements NoteRepository {
  /**
   * Get all notes (including deleted)
   */
  async getAll(): Promise<Note[]> {
    const db = getDB();
    return await db.getAll(STORES.NOTES);
  }

  /**
   * Get note by ID
   */
  async getById(id: string): Promise<Note | null> {
    const db = getDB();
    const note = await db.get(STORES.NOTES, id);
    return note || null;
  }

  /**
   * Get multiple notes by IDs
   */
  async getByIds(ids: string[]): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const promises = ids.map(id => tx.store.get(id));
    const results = await Promise.all(promises);
    await tx.done;
    return results.filter((note): note is Note => note !== undefined);
  }

  /**
   * Create a new note
   */
  async create(note: Note): Promise<Note> {
    const db = getDB();
    await db.add(STORES.NOTES, note);
    return note;
  }

  /**
   * Update an existing note
   * @param updateModifiedAt - If false, preserves the existing modifiedAt timestamp (for UI-only changes)
   * @param skipSyncFlag - If true, doesn't set needsSync flag (used when clearing flag after sync)
   */
  async update(note: Note, updateModifiedAt: boolean = true, skipSyncFlag: boolean = false): Promise<Note> {
    const db = getDB();
    // Update modified timestamp only for content changes, not UI state changes
    if (updateModifiedAt) {
      note.modifiedAt = new Date().toISOString();
    }
    // Mark note as needing sync when any change is made (unless explicitly skipped)
    if (!skipSyncFlag) {
      note.needsSync = true;
    }
    // Note: version is NOT incremented here - only when creating version snapshots
    await db.put(STORES.NOTES, note);
    return note;
  }

  /**
   * Delete a note permanently
   */
  async delete(id: string): Promise<void> {
    const db = getDB();
    await db.delete(STORES.NOTES, id);
  }

  /**
   * Get all active (non-deleted) notes
   * Uses 'deleted' index for efficient querying at scale
   */
  async getAllActive(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted');
    // IndexedDB coerces false to 0, but we query with false for type safety
    // The index stores boolean values, so query with false directly
    const notes = await index.getAll(IDBKeyRange.only(false));
    await tx.done;
    return notes;
  }

  /**
   * Get deleted notes (recycle bin)
   * Uses 'deleted' index for efficient querying at scale
   */
  async getDeleted(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted');
    const notes = await index.getAll(IDBKeyRange.only(true));
    await tx.done;
    return notes;
  }

  /**
   * Get pinned notes (active only)
   * Uses 'pinned' index, then filters for non-deleted
   */
  async getPinned(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('pinned');
    const pinnedNotes = await index.getAll(IDBKeyRange.only(true));
    await tx.done;
    // Filter out deleted notes (pinned count is typically small)
    return pinnedNotes.filter(note => !note.deleted);
  }

  /**
   * Soft delete a note
   */
  async softDelete(id: string): Promise<void> {
    console.log('[noteRepository] softDelete called with ID:', id);
    const note = await this.getById(id);
    if (!note) {
      console.error('[noteRepository] Note not found:', id);
      throw new Error(`Note ${id} not found`);
    }
    console.log('[noteRepository] Note found, marking as deleted');
    note.deleted = true;
    note.deletedAt = new Date().toISOString();
    await this.update(note);
    console.log('[noteRepository] Note updated as deleted');
  }

  /**
   * Restore a soft-deleted note
   */
  async restore(id: string): Promise<void> {
    const note = await this.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }
    note.deleted = false;
    note.deletedAt = undefined;
    await this.update(note);
  }

  /**
   * Permanently delete a note
   */
  async permanentDelete(id: string): Promise<void> {
    await this.delete(id);
  }

  /**
   * Update note's modified timestamp
   */
  async touch(id: string): Promise<void> {
    const note = await this.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }
    await this.update(note);
  }

  /**
   * Get notes modified after a timestamp (for sync)
   * @deprecated Use getNotesNeedingSync() instead for more reliable sync tracking
   */
  async getModifiedAfter(timestamp: string): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('modifiedAt');
    const range = IDBKeyRange.lowerBound(timestamp, true); // Exclude the timestamp itself
    const notes = await index.getAll(range);
    await tx.done;
    return notes;
  }

  /**
   * Get all notes that need to be synced
   * Uses needsSync flag to identify notes with any changes (content or UI state)
   * Optimised with index for large datasets
   */
  async getNotesNeedingSync(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');

    // Try to use the needsSync index for better performance
    // Note: IndexedDB doesn't support boolean queries directly, so we get all and filter
    try {
      const allNotes = await tx.store.getAll();
      await tx.done;
      return allNotes.filter(note => note.needsSync === true);
    } catch (error) {
      // Fallback to full scan if index doesn't exist yet (during migration)
      console.warn('[noteRepository] needsSync index not available, using full scan');
      const allNotes = await db.getAll(STORES.NOTES);
      return allNotes.filter(note => note.needsSync === true);
    }
  }

  /**
   * Get notes sorted by modified date (most recent first)
   * Uses 'deleted-modifiedAt' compound index for efficient querying
   */
  async getAllActiveByModified(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted-modifiedAt');

    // Compound index range: [deleted=false, any modifiedAt]
    // Use bound from [false, ''] to [false, '\uffff'] to get all non-deleted
    // Results come back sorted by [deleted, modifiedAt] ascending
    const range = IDBKeyRange.bound([false, ''], [false, '\uffff']);
    const notes = await index.getAll(range);
    await tx.done;

    // Reverse to get most recent first (index returns ascending order)
    return notes.reverse();
  }

  /**
   * Count all active notes
   * Uses 'deleted' index count for efficiency (doesn't load note data)
   */
  async countActive(): Promise<number> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted');
    const count = await index.count(IDBKeyRange.only(false));
    await tx.done;
    return count;
  }

  /**
   * Count deleted notes
   * Uses 'deleted' index count for efficiency (doesn't load note data)
   */
  async countDeleted(): Promise<number> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted');
    const count = await index.count(IDBKeyRange.only(true));
    await tx.done;
    return count;
  }
}

/**
 * Singleton instance
 */
export const noteRepository = new IndexedDBNoteRepository();
