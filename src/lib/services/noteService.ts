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
      showPreview?: boolean;
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
      syntaxLanguage: options?.syntaxLanguage || 'markdown',
      showPreview: options?.showPreview ?? false,
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
   * Get all active notes with batched decryption for better perceived performance
   * Decrypts first batch immediately, then continues in background
   * @param sortOrder - How to sort notes
   * @param batchSize - Number of notes in first batch (default: 50)
   * @param onProgress - Callback for subsequent batches
   * @returns First batch of decrypted notes
   */
  async getAllNotesBatched(
    sortOrder: SortOrder = 'recent',
    batchSize: number = 50,
    onProgress?: (notes: DecryptedNote[]) => void
  ): Promise<DecryptedNote[]> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked. Please unlock to view notes.');
    }

    // Get all encrypted notes
    const notes = await noteRepository.getAllActive();

    // Sort encrypted notes first (by metadata) to maintain correct order
    const sortedNotes = this.sortEncryptedNotes(notes, sortOrder);

    // Decrypt first batch immediately
    const firstBatch = sortedNotes.slice(0, batchSize);
    const firstDecrypted = await Promise.all(
      firstBatch.map(note => this.decryptNote(note, masterKey.key))
    );

    // Decrypt remaining batches in background
    if (sortedNotes.length > batchSize && onProgress) {
      // Don't await - let this run in background
      this.decryptRemainingBatches(
        sortedNotes.slice(batchSize),
        masterKey.key,
        batchSize,
        onProgress
      );
    }

    return firstDecrypted;
  }

  /**
   * Decrypt remaining notes in batches (runs in background)
   */
  private async decryptRemainingBatches(
    notes: Note[],
    key: CryptoKey,
    batchSize: number,
    onProgress: (notes: DecryptedNote[]) => void
  ): Promise<void> {
    const totalBatches = Math.ceil(notes.length / batchSize);

    for (let i = 0; i < totalBatches; i++) {
      const start = i * batchSize;
      const end = Math.min(start + batchSize, notes.length);
      const batch = notes.slice(start, end);

      try {
        const decrypted = await Promise.all(
          batch.map(note => this.decryptNote(note, key))
        );

        // Report progress
        onProgress(decrypted);

        // Small delay between batches to avoid blocking UI
        await new Promise(resolve => setTimeout(resolve, 10));
      } catch (error) {
        console.error(`Failed to decrypt batch ${i + 1}/${totalBatches}:`, error);
      }
    }
  }

  /**
   * Sort encrypted notes by metadata (for batched decryption)
   */
  private sortEncryptedNotes(notes: Note[], sortOrder: SortOrder): Note[] {
    const sorted = [...notes];

    // Separate pinned and unpinned
    const pinned = sorted.filter(n => n.pinned);
    const unpinned = sorted.filter(n => !n.pinned);

    // Sort each group by metadata
    const sortFn = this.getEncryptedSortFunction(sortOrder);
    pinned.sort(sortFn);
    unpinned.sort(sortFn);

    // Pinned notes always come first
    return [...pinned, ...unpinned];
  }

  /**
   * Get sort function for encrypted notes (uses metadata only)
   */
  private getEncryptedSortFunction(sortOrder: SortOrder) {
    switch (sortOrder) {
      case 'recent':
        return (a: Note, b: Note) => b.modifiedAt.localeCompare(a.modifiedAt);
      case 'oldest':
        return (a: Note, b: Note) => a.modifiedAt.localeCompare(b.modifiedAt);
      case 'created':
        return (a: Note, b: Note) => b.createdAt.localeCompare(a.createdAt);
      case 'alpha':
        // For alpha sort, we need decrypted content, so just use modified for now
        // The final sort will happen after all notes are decrypted
        return (a: Note, b: Note) => b.modifiedAt.localeCompare(a.modifiedAt);
      default:
        return (a: Note, b: Note) => b.modifiedAt.localeCompare(a.modifiedAt);
    }
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
      showPreview?: boolean;
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

    // Track if actual content changed (not just UI state)
    let hasContentChange = false;

    // Decrypt current content to compare
    let currentContent = '';
    let currentTags: string[] = [];
    try {
      const contentData = JSON.parse(note.content);
      currentContent = await cryptoService.decryptText(contentData, masterKey.key);

      if (note.tags.length > 0 && note.tags[0]) {
        const tagsData = JSON.parse(note.tags[0]);
        currentTags = await decryptStringArray(tagsData, masterKey.key);
      }
    } catch (error) {
      console.error('Failed to decrypt current note for comparison:', error);
    }

    // Update content if provided and changed
    if (updates.content !== undefined && updates.content !== currentContent) {
      const encryptedContent = await cryptoService.encryptText(
        updates.content,
        masterKey.key
      );
      note.content = JSON.stringify(encryptedContent);
      hasContentChange = true;
    }

    // Update tags if provided and changed
    if (updates.tags !== undefined) {
      const tagsChanged =
        updates.tags.length !== currentTags.length ||
        updates.tags.some((tag, i) => tag !== currentTags[i]);

      if (tagsChanged) {
        const encryptedTags = await encryptStringArray(updates.tags, masterKey.key);
        note.tags = [JSON.stringify(encryptedTags)];
        hasContentChange = true;
      }
    }

    // Update attachments if provided and changed
    if (updates.attachments !== undefined) {
      // Check if attachments changed
      const oldAttachmentIds = new Set(note.attachments.map(a => a.id));
      const newAttachmentIds = new Set(updates.attachments.map(a => a.id));
      const attachmentsChanged =
        oldAttachmentIds.size !== newAttachmentIds.size ||
        [...oldAttachmentIds].some(id => !newAttachmentIds.has(id));

      if (attachmentsChanged) {
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
        hasContentChange = true;
      }
    }

    // Update pinned status if provided and changed
    if (updates.pinned !== undefined && updates.pinned !== note.pinned) {
      note.pinned = updates.pinned;
      hasContentChange = true;
    }

    // Update word wrap if provided
    if (updates.wordWrap !== undefined) {
      note.wordWrap = updates.wordWrap;
    }

    // Update syntax language if provided
    if (updates.syntaxLanguage !== undefined) {
      note.syntaxLanguage = updates.syntaxLanguage;
    }

    // Update show preview if provided
    if (updates.showPreview !== undefined) {
      note.showPreview = updates.showPreview;
    }

    // Only update modifiedAt if there was an actual content change
    return await noteRepository.update(note, hasContentChange);
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

      // Defensive check: ensure note.tags is an array
      if (!Array.isArray(note.tags)) {
        console.error(`[NoteService] Note ${note.id} has invalid tags type:`, typeof note.tags, note.tags);
        // Force it to be an empty array
        note.tags = [];
      }

      if (note.tags.length > 0) {
        try {
          // Check format: single blob (length 1) vs individual tags (length > 1)
          if (note.tags.length === 1) {
            // NEW FORMAT: Single encrypted blob containing all tags
            const encryptedTags = JSON.parse(note.tags[0]);
            const decryptedTags = await decryptStringArray(encryptedTags, key);

            // Defensive check: ensure decrypted tags is an array
            if (!Array.isArray(decryptedTags)) {
              console.error(`[NoteService] Note ${note.id} decrypted tags is not an array:`, typeof decryptedTags, decryptedTags);
              tags = [];
            } else {
              tags = decryptedTags;
            }
          } else {
            // OLD/TUI FORMAT: Multiple individually encrypted tags (from before conversion was added)
            const decryptedTags: string[] = [];

            for (let i = 0; i < note.tags.length; i++) {
              try {
                const encryptedTag = JSON.parse(note.tags[i]);
                const tagJson = await cryptoService.decryptText(encryptedTag, key);
                const tag = JSON.parse(tagJson);

                if (typeof tag === 'string' && tag.trim().length > 0) {
                  decryptedTags.push(tag);
                }
              } catch (error) {
                console.error(`[NoteService] Note ${note.id} failed to decrypt tag[${i}]:`, error);
              }
            }

            tags = decryptedTags;
          }
        } catch (error) {
          console.error(`[NoteService] Failed to decrypt tags for note ${note.id}:`, error);
          tags = [];
        }
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
