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
   * Get all active (non-deleted, non-archived) notes
   */
  async getAllActive(): Promise<Note[]> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes.filter((note) => !note.deleted && !note.archived);
  }

  /**
   * Get all non-deleted notes (includes archived)
   */
  async getAllNonDeleted(): Promise<Note[]> {
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
   * Get archived notes
   */
  async getArchived(): Promise<Note[]> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes.filter((note) => note.archived && !note.deleted);
  }

  /**
   * Get pinned notes (active only)
   */
  async getPinned(): Promise<Note[]> {
    const db = getDB();
    const notes = await db.getAll(STORES.NOTES);
    return notes.filter((note) => note.pinned && !note.deleted && !note.archived);
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
   * Archive a note
   */
  async archive(id: string): Promise<void> {
    const note = await this.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }
    note.archived = true;
    note.archivedAt = new Date().toISOString();
    await this.update(note);
  }

  /**
   * Unarchive a note
   */
  async unarchive(id: string): Promise<void> {
    const note = await this.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }
    note.archived = false;
    note.archivedAt = undefined;
    await this.update(note);
  }

  /**
   * Lock a note (prevent editing)
   */
  async lock(id: string): Promise<void> {
    const note = await this.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }
    note.locked = true;
    note.lockedAt = new Date().toISOString();
    await this.update(note);
  }

  /**
   * Unlock a note (allow editing)
   */
  async unlock(id: string): Promise<void> {
    const note = await this.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }
    note.locked = false;
    note.lockedAt = undefined;
    await this.update(note);
  }

  /**
   * Permanently delete a note (records deletion for sync)
   */
  async permanentDelete(id: string): Promise<void> {
    const db = getDB();

    // Record the deletion for sync before removing the note
    await db.put(STORES.DELETIONS, {
      id,
      deletedAt: new Date().toISOString(),
      synced: false,
    });

    // Now delete the note
    await this.delete(id);
  }

  /**
   * Get pending deletions that need to be synced to server
   */
  async getPendingDeletions(): Promise<Array<{ id: string; deletedAt: string }>> {
    const db = getDB();
    const all = await db.getAll(STORES.DELETIONS);
    return all
      .filter(d => !d.synced)
      .map(d => ({ id: d.id, deletedAt: d.deletedAt }));
  }

  /**
   * Mark deletions as synced after successful push
   */
  async markDeletionsSynced(ids: string[]): Promise<void> {
    const db = getDB();
    const tx = db.transaction(STORES.DELETIONS, 'readwrite');

    for (const id of ids) {
      const deletion = await tx.store.get(id);
      if (deletion) {
        deletion.synced = true;
        await tx.store.put(deletion);
      }
    }

    await tx.done;
  }

  /**
   * Apply a remote deletion (from server pull)
   * Hard-deletes the note locally without recording it for sync
   */
  async applyRemoteDeletion(id: string): Promise<boolean> {
    const db = getDB();

    // Check if note exists
    const note = await db.get(STORES.NOTES, id);
    if (note) {
      // Delete the note without recording in deletions store
      await db.delete(STORES.NOTES, id);
      return true;
    }

    return false;
  }

  /**
   * Clean up old synced deletions (older than 30 days)
   */
  async cleanupOldDeletions(): Promise<number> {
    const db = getDB();
    const thirtyDaysAgo = new Date();
    thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);
    const cutoff = thirtyDaysAgo.toISOString();

    const all = await db.getAll(STORES.DELETIONS);
    let count = 0;

    const tx = db.transaction(STORES.DELETIONS, 'readwrite');
    for (const deletion of all) {
      if (deletion.synced && deletion.deletedAt < cutoff) {
        await tx.store.delete(deletion.id);
        count++;
      }
    }
    await tx.done;

    return count;
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
   * Returns notes that either:
   * - Have needsSync flag set to true (modified locally)
   * - Have never been synced (syncedAt is null/undefined)
   * This ensures notes are never "lost" if needsSync was cleared without successful sync
   */
  async getNotesNeedingSync(): Promise<Note[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTES, 'readonly');

    try {
      const allNotes = await tx.store.getAll();
      await tx.done;
      // Include notes with needsSync=true OR notes that have never been synced
      return allNotes.filter(note => note.needsSync === true || note.syncedAt == null);
    } catch (error) {
      // Fallback to full scan if index doesn't exist yet (during migration)
      console.warn('[noteRepository] needsSync index not available, using full scan');
      const allNotes = await db.getAll(STORES.NOTES);
      return allNotes.filter(note => note.needsSync === true || note.syncedAt == null);
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
