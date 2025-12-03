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
   */
  async update(note: Note): Promise<Note> {
    const db = getDB();
    // Update modified timestamp
    note.modifiedAt = new Date().toISOString();
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
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted');
    const notes = await index.getAll(0); // 0 = not deleted
    await tx.done;
    return notes;
  }

  /**
   * Get deleted notes (recycle bin)
   */
  async getDeleted(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted');
    const notes = await index.getAll(1); // 1 = deleted
    await tx.done;
    return notes;
  }

  /**
   * Get pinned notes
   */
  async getPinned(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('pinned');
    const notes = await index.getAll(1); // 1 = pinned
    await tx.done;
    // Filter out deleted notes
    return notes.filter(note => !note.deleted);
  }

  /**
   * Soft delete a note
   */
  async softDelete(id: string): Promise<void> {
    const note = await this.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }
    note.deleted = true;
    note.deletedAt = new Date().toISOString();
    await this.update(note);
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
   * Get notes sorted by modified date (most recent first)
   */
  async getAllActiveByModified(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted-modifiedAt');

    // Get all non-deleted notes
    const range = IDBKeyRange.bound([0, ''], [0, '\uffff']);
    let cursor = await index.openCursor(range, 'prev'); // Descending order

    const notes: Note[] = [];
    while (cursor) {
      notes.push(cursor.value);
      cursor = await cursor.continue();
    }

    await tx.done;
    return notes;
  }

  /**
   * Count all active notes
   */
  async countActive(): Promise<number> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted');
    const count = await index.count(0);
    await tx.done;
    return count;
  }

  /**
   * Count deleted notes
   */
  async countDeleted(): Promise<number> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');
    const index = tx.store.index('deleted');
    const count = await index.count(1);
    await tx.done;
    return count;
  }
}

/**
 * Singleton instance
 */
export const noteRepository = new IndexedDBNoteRepository();
