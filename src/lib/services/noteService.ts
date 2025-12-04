/**
 * Note service providing CRUD operations with encryption
 * This is the main business logic layer for notes
 */

import type { Note, DecryptedNote, SortOrder, Attachment } from '../types';
import { DEFAULT_NOTE } from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { cryptoService, encryptStringArray, decryptStringArray } from './crypto';
import { keyManager } from './keyManager';

/**
 * Note service class
 */
class NoteService {
  /**
   * Create a new note
   */
  async createNote(
    content: string,
    tags: string[] = [],
    options?: {
      createdAt?: string;
      modifiedAt?: string;
      pinned?: boolean;
      wordWrap?: boolean;
      syntaxLanguage?: 'plain' | 'javascript' | 'python' | 'markdown' | 'json' | 'html' | 'css' | 'sql' | 'bash';
      attachments?: Attachment[];
    }
  ): Promise<Note> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked. Please unlock to create notes.');
    }

    const now = new Date().toISOString();
    const id = cryptoService.generateUUID();

    // Encrypt content and tags
    const encryptedContent = await cryptoService.encryptText(content, masterKey.key);
    const encryptedTags = await encryptStringArray(tags, masterKey.key);

    const note: Note = {
      ...DEFAULT_NOTE,
      id,
      createdAt: options?.createdAt || now,
      modifiedAt: options?.modifiedAt || now,
      pinned: options?.pinned || false,
      wordWrap: options?.wordWrap ?? true,
      syntaxLanguage: options?.syntaxLanguage || 'plain',
      content: JSON.stringify(encryptedContent),
      tags: [JSON.stringify(encryptedTags)],
      attachments: options?.attachments || [],
    };

    return await noteRepository.create(note);
  }

  /**
   * Get a decrypted note by ID
   */
  async getNote(id: string): Promise<DecryptedNote | null> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked. Please unlock to view notes.');
    }

    const note = await noteRepository.getById(id);
    if (!note) {
      return null;
    }

    return await this.decryptNote(note, masterKey.key);
  }

  /**
   * Get all active notes (decrypted)
   */
  async getAllNotes(sortOrder: SortOrder = 'recent'): Promise<DecryptedNote[]> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked. Please unlock to view notes.');
    }

    const notes = await noteRepository.getAllActive();
    const decrypted = await Promise.all(
      notes.map(note => this.decryptNote(note, masterKey.key))
    );

    return this.sortNotes(decrypted, sortOrder);
  }

  /**
   * Get pinned notes (decrypted)
   */
  async getPinnedNotes(): Promise<DecryptedNote[]> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked.');
    }

    const notes = await noteRepository.getPinned();
    return await Promise.all(
      notes.map(note => this.decryptNote(note, masterKey.key))
    );
  }

  /**
   * Update note content and/or tags
   */
  async updateNote(
    id: string,
    updates: {
      content?: string;
      tags?: string[];
      attachments?: Attachment[];
      pinned?: boolean;
      wordWrap?: boolean;
      syntaxLanguage?: 'plain' | 'javascript' | 'python' | 'markdown' | 'json' | 'html' | 'css' | 'sql' | 'bash';
    }
  ): Promise<Note> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked.');
    }

    const note = await noteRepository.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }

    // Update content if provided
    if (updates.content !== undefined) {
      const encryptedContent = await cryptoService.encryptText(
        updates.content,
        masterKey.key
      );
      note.content = JSON.stringify(encryptedContent);
    }

    // Update tags if provided
    if (updates.tags !== undefined) {
      const encryptedTags = await encryptStringArray(updates.tags, masterKey.key);
      note.tags = [JSON.stringify(encryptedTags)];
    }

    // Update attachments if provided
    if (updates.attachments !== undefined) {
      // Find removed attachments and clean them up
      const oldAttachmentIds = new Set(note.attachments.map(a => a.id));
      const newAttachmentIds = new Set(updates.attachments.map(a => a.id));

      // Delete attachments that were removed
      for (const oldId of oldAttachmentIds) {
        if (!newAttachmentIds.has(oldId)) {
          try {
            await attachmentRepository.deleteBlob(oldId);
            // Also try to delete thumbnail if it exists
            await attachmentRepository.deleteThumbnail(oldId);
          } catch (error) {
            console.error(`Failed to delete attachment blob ${oldId}:`, error);
          }
        }
      }

      note.attachments = updates.attachments;
    }

    // Update pinned status if provided
    if (updates.pinned !== undefined) {
      note.pinned = updates.pinned;
    }

    // Update word wrap if provided
    if (updates.wordWrap !== undefined) {
      note.wordWrap = updates.wordWrap;
    }

    // Update syntax language if provided
    if (updates.syntaxLanguage !== undefined) {
      note.syntaxLanguage = updates.syntaxLanguage;
    }

    return await noteRepository.update(note);
  }

  /**
   * Toggle pin status
   */
  async togglePin(id: string): Promise<Note> {
    const note = await noteRepository.getById(id);
    if (!note) {
      throw new Error(`Note ${id} not found`);
    }

    note.pinned = !note.pinned;
    return await noteRepository.update(note);
  }

  /**
   * Soft delete a note
   */
  async deleteNote(id: string): Promise<void> {
    console.log('[noteService] deleteNote called with ID:', id);
    await noteRepository.softDelete(id);
    console.log('[noteService] deleteNote completed');
  }

  /**
   * Restore a deleted note
   */
  async restoreNote(id: string): Promise<void> {
    await noteRepository.restore(id);
  }

  /**
   * Permanently delete a note and its attachments
   */
  async permanentlyDeleteNote(id: string): Promise<void> {
    const note = await noteRepository.getById(id);
    if (note && note.attachments.length > 0) {
      // Delete all attachments
      await Promise.all(
        note.attachments.map(attachment =>
          attachmentRepository.deleteAll(attachment.id)
        )
      );
    }

    await noteRepository.permanentDelete(id);
  }

  /**
   * Get deleted notes (recycle bin)
   */
  async getDeletedNotes(): Promise<DecryptedNote[]> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked.');
    }

    const notes = await noteRepository.getDeleted();
    return await Promise.all(
      notes.map(note => this.decryptNote(note, masterKey.key))
    );
  }

  /**
   * Purge old deleted notes (older than specified days)
   */
  async purgeOldDeletedNotes(daysOld: number = 30): Promise<number> {
    const notes = await noteRepository.getDeleted();
    const cutoffDate = new Date();
    cutoffDate.setDate(cutoffDate.getDate() - daysOld);
    const cutoffISO = cutoffDate.toISOString();

    let purgedCount = 0;
    for (const note of notes) {
      if (note.deletedAt && note.deletedAt < cutoffISO) {
        await this.permanentlyDeleteNote(note.id);
        purgedCount++;
      }
    }

    return purgedCount;
  }

  /**
   * Search notes by content or tags
   */
  async searchNotes(query: string): Promise<DecryptedNote[]> {
    const allNotes = await this.getAllNotes();
    const lowerQuery = query.toLowerCase();

    return allNotes.filter(note => {
      const contentMatch = note.content.toLowerCase().includes(lowerQuery);
      const tagsMatch = note.tags.some(tag =>
        tag.toLowerCase().includes(lowerQuery)
      );
      return contentMatch || tagsMatch;
    });
  }

  /**
   * Get notes statistics
   */
  async getStats(): Promise<{
    total: number;
    active: number;
    deleted: number;
    pinned: number;
  }> {
    const [active, deleted, pinned] = await Promise.all([
      noteRepository.countActive(),
      noteRepository.countDeleted(),
      noteRepository.getPinned().then(notes => notes.length),
    ]);

    return {
      total: active + deleted,
      active,
      deleted,
      pinned,
    };
  }

  /**
   * Decrypt a note (private helper)
   */
  private async decryptNote(note: Note, key: CryptoKey): Promise<DecryptedNote> {
    try {
      const encryptedContent = JSON.parse(note.content);
      const content = await cryptoService.decryptText(encryptedContent, key);

      let tags: string[] = [];
      if (note.tags.length > 0 && note.tags[0]) {
        const encryptedTags = JSON.parse(note.tags[0]);
        tags = await decryptStringArray(encryptedTags, key);
      }

      return {
        ...note,
        content,
        tags,
        decryptedAt: Date.now(),
      };
    } catch (error) {
      throw new Error(`Failed to decrypt note ${note.id}: ${error}`);
    }
  }

  /**
   * Sort notes by specified order
   */
  private sortNotes(notes: DecryptedNote[], sortOrder: SortOrder): DecryptedNote[] {
    const sorted = [...notes];

    // Separate pinned and unpinned
    const pinned = sorted.filter(n => n.pinned);
    const unpinned = sorted.filter(n => !n.pinned);

    // Sort each group
    const sortFn = this.getSortFunction(sortOrder);
    pinned.sort(sortFn);
    unpinned.sort(sortFn);

    // Pinned notes always come first
    return [...pinned, ...unpinned];
  }

  /**
   * Get sort function for specified order
   */
  private getSortFunction(sortOrder: SortOrder) {
    switch (sortOrder) {
      case 'recent':
        return (a: DecryptedNote, b: DecryptedNote) =>
          b.modifiedAt.localeCompare(a.modifiedAt);

      case 'oldest':
        return (a: DecryptedNote, b: DecryptedNote) =>
          a.modifiedAt.localeCompare(b.modifiedAt);

      case 'created':
        return (a: DecryptedNote, b: DecryptedNote) =>
          b.createdAt.localeCompare(a.createdAt);

      case 'alpha':
        // Sort alphabetically by first line of content
        return (a: DecryptedNote, b: DecryptedNote) => {
          const aTitle = a.content.split('\n')[0].toLowerCase();
          const bTitle = b.content.split('\n')[0].toLowerCase();
          return aTitle.localeCompare(bTitle);
        };

      default:
        return (a: DecryptedNote, b: DecryptedNote) =>
          b.modifiedAt.localeCompare(a.modifiedAt);
    }
  }
}

/**
 * Singleton instance
 */
export const noteService = new NoteService();
