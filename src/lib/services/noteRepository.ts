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
   */
  async getAllActive(): Promise<Note[]> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes.filter((note) => !note.deleted);
  }

  /**
   * Get deleted notes (recycle bin)
   */
  async getDeleted(): Promise<Note[]> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes.filter((note) => note.deleted);
  }

  /**
   * Get pinned notes (active only)
   */
  async getPinned(): Promise<Note[]> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes.filter((note) => note.pinned && !note.deleted);
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
   */
  async getAllActiveByModified(): Promise<Note[]> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes
      .filter((note) => !note.deleted)
      .sort((a, b) => b.modifiedAt.localeCompare(a.modifiedAt));
  }

  /**
   * Count all active notes
   */
  async countActive(): Promise<number> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes.filter((note) => !note.deleted).length;
  }

  /**
   * Count deleted notes
   */
  async countDeleted(): Promise<number> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes.filter((note) => note.deleted).length;
  }
}

/**
 * Singleton instance
 */
export const noteRepository = new IndexedDBNoteRepository();
