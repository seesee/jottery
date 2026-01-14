/**
 * Version repository for managing note version history
 * Versions are created during sync events to maintain historical snapshots
 */

import type { Note, NoteVersion } from '../types';
import { getDB, STORES } from './db';

class VersionRepository {
  /**
   * Create a new version snapshot of a note
   * Includes deduplication - skips if content unchanged from latest version
   */
  async createVersion(
    note: Note,
    metadata: { syncedAt: string; reason: 'sync' | 'manual-sync' }
  ): Promise<NoteVersion | null> {
    // Deduplication check
    const latestVersion = await this.getLatestVersion(note.id);
    if (latestVersion && latestVersion.content === note.content) {
      return null; // Skip duplicate
    }

    const versionKey = `${note.id}:${note.version}`;
    const version: NoteVersion = {
      versionKey,
      noteId: note.id,
      version: note.version,
      createdAt: new Date().toISOString(),
      syncedAt: metadata.syncedAt,
      content: note.content,
      tags: note.tags,
      attachments: note.attachments,
      syntaxLanguage: note.syntaxLanguage,
      wordWrap: note.wordWrap,
      reason: metadata.reason,
    };

    await getDB().put(STORES.NOTE_VERSIONS, version);
    return version;
  }

  /**
   * Get all versions for a note, sorted by version number (newest first)
   */
  async getVersionsForNote(noteId: string): Promise<NoteVersion[]> {
    const db = getDB();
    const tx = db.transaction(STORES.NOTE_VERSIONS, 'readonly');
    const versions = await tx.store.index('noteId').getAll(noteId);
    await tx.done;
    return versions.sort((a, b) => b.version - a.version);
  }

  /**
   * Get a specific version of a note
   */
  async getVersion(noteId: string, version: number): Promise<NoteVersion | null> {
    const versionKey = `${noteId}:${version}`;
    const result = await getDB().get(STORES.NOTE_VERSIONS, versionKey);
    return result || null;
  }

  /**
   * Get the latest version for a note
   */
  async getLatestVersion(noteId: string): Promise<NoteVersion | null> {
    const versions = await this.getVersionsForNote(noteId);
    return versions[0] || null;
  }

  /**
   * Delete all versions for a note (cascade delete when note is permanently deleted)
   */
  async deleteVersionsForNote(noteId: string): Promise<void> {
    const versions = await this.getVersionsForNote(noteId);
    const tx = getDB().transaction(STORES.NOTE_VERSIONS, 'readwrite');
    for (const v of versions) {
      await tx.store.delete(v.versionKey);
    }
    await tx.done;
    console.log(`[versionRepository] Deleted ${versions.length} versions for note ${noteId}`);
  }

  /**
   * Count total versions for a note
   */
  async countVersionsForNote(noteId: string): Promise<number> {
    const versions = await this.getVersionsForNote(noteId);
    return versions.length;
  }
}

/**
 * Singleton instance
 */
export const versionRepository = new VersionRepository();
