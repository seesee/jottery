/**
 * Note service providing CRUD operations with encryption
 * This is the main business logic layer for notes
 */

import type { Note, DecryptedNote, SortOrder, Attachment } from '../types';
import { DEFAULT_NOTE } from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { versionRepository } from './versionRepository';
import { cryptoService, encryptStringArray, decryptStringArray, updateHashChain } from './crypto';
import { keyManager } from './keyManager';
import { ApplicationLockedError, NotFoundError, CryptoError } from '../errors';
import { backupSchedulerService } from './backupSchedulerService';
import { getNoteTitle } from '../utils/noteTitle';
import { localeIncludes, localeSort } from '../utils/stringUtils';

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
      id?: string; // Optional: preserve ID during import
      createdAt?: string;
      modifiedAt?: string;
      pinned?: boolean;
      locked?: boolean;
      wordWrap?: boolean;
      syntaxLanguage?: string;
      showPreview?: boolean;
      color?: string; // Semantic color name (e.g., 'red', 'blue')
      attachments?: Attachment[];
    }
  ): Promise<Note> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new ApplicationLockedError();
    }

    const now = new Date().toISOString();
    const id = options?.id || cryptoService.generateUUID();

    // Encrypt content and tags
    const encryptedContent = await cryptoService.encryptText(content, masterKey.key);
    const encryptedTags = await encryptStringArray(tags, masterKey.key);

    const note: Note = {
      ...DEFAULT_NOTE,
      id,
      createdAt: options?.createdAt || now,
      modifiedAt: options?.modifiedAt || now,
      pinned: options?.pinned || false,
      locked: options?.locked || false,
      wordWrap: options?.wordWrap ?? true,
      syntaxLanguage: options?.syntaxLanguage || 'markdown',
      showPreview: options?.showPreview ?? false,
      color: options?.color,
      content: JSON.stringify(encryptedContent),
      tags: [JSON.stringify(encryptedTags)],
      attachments: options?.attachments || [],
      // Hash chain fields - new notes start with no parent
      parentHash: null,
      hashChain: [],
    };

    // Compute content hash for the new note
    const contentHash = await cryptoService.computeContentHash(note);
    note.contentHash = contentHash;
    note.hashChain = updateHashChain(contentHash, []);

    const createdNote = await noteRepository.create(note);

    // Notify backup scheduler of note creation (fire and forget)
    backupSchedulerService.onNoteUpdated().catch(() => {});

    return createdNote;
  }

  /**
   * Get a decrypted note by ID
   */
  async getNote(id: string): Promise<DecryptedNote | null> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new ApplicationLockedError();
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
      throw new ApplicationLockedError();
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
      throw new ApplicationLockedError();
    }

    // Get all non-deleted notes (includes archived for filtering)
    const notes = await noteRepository.getAllNonDeleted();

    // Sort encrypted notes first (by metadata) to maintain correct order
    const sortedNotes = this.sortEncryptedNotes(notes, sortOrder);

    // Decrypt first batch immediately (skip notes that fail to decrypt)
    const firstBatch = sortedNotes.slice(0, batchSize);
    const firstResults = await Promise.allSettled(
      firstBatch.map(note => this.decryptNote(note, masterKey.key))
    );
    const firstDecrypted: DecryptedNote[] = [];
    for (const result of firstResults) {
      if (result.status === 'fulfilled') {
        firstDecrypted.push(result.value);
      } else {
        console.error('[NoteService] Skipping undecryptable note:', result.reason);
      }
    }

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

      const results = await Promise.allSettled(
        batch.map(note => this.decryptNote(note, key))
      );
      const decrypted: DecryptedNote[] = [];
      for (const result of results) {
        if (result.status === 'fulfilled') {
          decrypted.push(result.value);
        } else {
          console.error(`[NoteService] Skipping undecryptable note in batch ${i + 1}/${totalBatches}:`, result.reason);
        }
      }

      if (decrypted.length > 0) {
        onProgress(decrypted);
      }

      // Small delay between batches to avoid blocking UI
      await new Promise(resolve => setTimeout(resolve, 10));
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
      throw new ApplicationLockedError();
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
      syntaxLanguage?: string;
      showPreview?: boolean;
      color?: string; // Semantic color name (or undefined to remove)
    }
  ): Promise<Note> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new ApplicationLockedError();
    }

    const note = await noteRepository.getById(id);
    if (!note) {
      throw new NotFoundError('Note', id);
    }

    // Before editing, create a version snapshot if one doesn't exist for version 0
    // This ensures we can revert to the original state before any edits
    // Only do this once - when the note has never been edited (version 0)
    if (note.version === 0) {
      const existingVersion = await versionRepository.getVersion(note.id, 0);
      if (!existingVersion) {
        // No snapshot exists for version 0, create one to preserve original state
        await versionRepository.createVersion(note, {
          syncedAt: note.syncedAt || new Date().toISOString(),
          reason: 'manual-sync',
        });
      }
      // Increment version since we've created a snapshot of the original state
      // All subsequent edits will be at version 1+
      note.version = 1;
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

    // Update word wrap if provided and changed
    if (updates.wordWrap !== undefined && updates.wordWrap !== note.wordWrap) {
      note.wordWrap = updates.wordWrap;
      hasContentChange = true; // UI state changes should also sync
    }

    // Update syntax language if provided and changed
    if (updates.syntaxLanguage !== undefined && updates.syntaxLanguage !== note.syntaxLanguage) {
      note.syntaxLanguage = updates.syntaxLanguage;
      hasContentChange = true; // UI state changes should also sync
    }

    // Update show preview if provided and changed
    if (updates.showPreview !== undefined && updates.showPreview !== note.showPreview) {
      note.showPreview = updates.showPreview;
      hasContentChange = true; // UI state changes should also sync
    }

    // Update color if provided and changed
    if ('color' in updates && updates.color !== note.color) {
      note.color = updates.color || undefined; // Allow removing color by setting to empty string or undefined
      hasContentChange = true; // UI state changes should also sync
    }

    // Update hash chain if content changed
    if (hasContentChange) {
      // Store old hash as parent
      const oldContentHash = note.contentHash;
      const oldHashChain = note.hashChain || [];

      // Compute new content hash
      const newContentHash = await cryptoService.computeContentHash(note);
      note.contentHash = newContentHash;
      note.parentHash = oldContentHash || null;
      note.hashChain = updateHashChain(newContentHash, oldHashChain);
    }

    // Update modifiedAt for any change (content or UI state) so it syncs properly
    const updatedNote = await noteRepository.update(note, hasContentChange);

    // Notify backup scheduler of note update (fire and forget)
    if (hasContentChange) {
      backupSchedulerService.onNoteUpdated().catch(() => {});
    }

    return updatedNote;
  }

  /**
   * Toggle pin status
   */
  async togglePin(id: string): Promise<Note> {
    const note = await noteRepository.getById(id);
    if (!note) {
      throw new NotFoundError('Note', id);
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
   * Archive a note
   */
  async archiveNote(id: string): Promise<void> {
    await noteRepository.archive(id);
  }

  /**
   * Unarchive a note
   */
  async unarchiveNote(id: string): Promise<void> {
    await noteRepository.unarchive(id);
  }

  /**
   * Lock a note (prevent editing)
   */
  async lockNote(id: string): Promise<void> {
    await noteRepository.lock(id);
  }

  /**
   * Unlock a note (allow editing)
   */
  async unlockNote(id: string): Promise<void> {
    await noteRepository.unlock(id);
  }

  /**
   * Get archived notes
   */
  async getArchivedNotes(): Promise<DecryptedNote[]> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new ApplicationLockedError();
    }

    const notes = await noteRepository.getArchived();
    return await Promise.all(
      notes.map(note => this.decryptNote(note, masterKey.key))
    );
  }

  /**
   * Duplicate an existing note
   * Creates a copy with new ID and timestamps, preserving all content and settings
   */
  async duplicateNote(id: string): Promise<Note> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new ApplicationLockedError();
    }

    // Get the original note (decrypted)
    const original = await this.getNote(id);
    if (!original) {
      throw new NotFoundError('Note', id);
    }

    // Create a new note with the same content and settings
    return await this.createNote(original.content, original.tags, {
      pinned: false, // Don't duplicate pinned status
      wordWrap: original.wordWrap,
      syntaxLanguage: original.syntaxLanguage,
      showPreview: original.showPreview,
      // Note: attachments are not duplicated - they reference the same blobs
      // If we wanted to duplicate attachments, we'd need to copy the blobs too
    });
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
      throw new ApplicationLockedError();
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

    return allNotes.filter(note => {
      const contentMatch = localeIncludes(note.content, query);
      const tagsMatch = note.tags.some(tag =>
        localeIncludes(tag, query)
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
            // (handles edge cases from malformed data or format mismatches)
            if (!Array.isArray(decryptedTags)) {
              console.warn(`[NoteService] Note ${note.id} decrypted tags is not an array:`, typeof decryptedTags);
              // Try to salvage: if it's a non-empty string, wrap in array
              const tagsValue = decryptedTags as unknown;
              if (typeof tagsValue === 'string' && (tagsValue as string).trim()) {
                tags = [(tagsValue as string).trim()];
              } else {
                tags = [];
              }
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
      throw new CryptoError('decrypt', `note ${note.id}`, error instanceof Error ? error : undefined);
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
        // Sort alphabetically by title (custom t: tag or first line)
        return (a: DecryptedNote, b: DecryptedNote) =>
          localeSort(getNoteTitle(a), getNoteTitle(b));

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
