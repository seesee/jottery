/**
 * Version service for decrypting and managing note version history
 * Handles version retrieval, decryption, and restoration
 */

import type { DecryptedNoteVersion, Note } from '../types';
import { versionRepository } from './versionRepository';
import { noteRepository } from './noteRepository';
import { cryptoService, decryptStringArray } from './crypto';
import { keyManager } from './keyManager';

class VersionService {
  /**
   * Get all versions for a note (decrypted)
   */
  async getVersionsForNote(noteId: string): Promise<DecryptedNoteVersion[]> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    const versions = await versionRepository.getVersionsForNote(noteId);
    return await Promise.all(
      versions.map(v => this.decryptVersion(v, masterKey.key))
    );
  }

  /**
   * Get a specific version (decrypted)
   */
  async getVersion(noteId: string, version: number): Promise<DecryptedNoteVersion | null> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    const versionData = await versionRepository.getVersion(noteId, version);
    if (!versionData) {
      return null;
    }

    return await this.decryptVersion(versionData, masterKey.key);
  }

  /**
   * Restore a note to a specific version
   * Creates a snapshot of the current state before restoring
   */
  async restoreVersion(noteId: string, targetVersion: number): Promise<void> {
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    const version = await versionRepository.getVersion(noteId, targetVersion);
    if (!version) {
      throw new Error(`Version ${targetVersion} not found`);
    }

    const currentNote = await noteRepository.getById(noteId);
    if (!currentNote) {
      throw new Error(`Note ${noteId} not found`);
    }

    // Snapshot current state before restoring
    await versionRepository.createVersion(currentNote, {
      syncedAt: new Date().toISOString(),
      reason: 'manual-sync',
    });

    // Restore version data (all fields are already encrypted)
    currentNote.content = version.content;
    currentNote.tags = version.tags;
    currentNote.attachments = version.attachments;
    currentNote.syntaxLanguage = version.syntaxLanguage;
    currentNote.wordWrap = version.wordWrap;

    await noteRepository.update(currentNote);
    console.log(`[versionService] Restored note ${noteId} to version ${targetVersion}`);
  }

  /**
   * Decrypt a version for display
   * Handles both new (single blob) and old (individual) tag formats
   */
  private async decryptVersion(version: any, key: CryptoKey): Promise<DecryptedNoteVersion> {
    try {
      // Decrypt content
      const encryptedContent = JSON.parse(version.content);
      const content = await cryptoService.decryptText(encryptedContent, key);

      // Decrypt tags
      let tags: string[] = [];

      // Defensive check: ensure version.tags is an array
      if (!Array.isArray(version.tags)) {
        console.error(`[versionService] Version has invalid tags type:`, typeof version.tags);
        version.tags = [];
      }

      if (version.tags.length > 0) {
        try {
          if (version.tags.length === 1) {
            // NEW FORMAT: Single encrypted blob containing all tags
            const encryptedTags = JSON.parse(version.tags[0]);
            const decryptedTags = await decryptStringArray(encryptedTags, key);

            // Defensive check: ensure decrypted tags is an array
            if (!Array.isArray(decryptedTags)) {
              console.error(`[versionService] Decrypted tags is not an array:`, typeof decryptedTags);
              tags = [];
            } else {
              tags = decryptedTags;
            }
          } else {
            // OLD FORMAT: Multiple individually encrypted tags
            console.log(`[versionService] Version has old format (${version.tags.length} individual tags)`);
            const decryptedTags: string[] = [];

            for (let i = 0; i < version.tags.length; i++) {
              try {
                const encryptedTag = JSON.parse(version.tags[i]);
                const tagJson = await cryptoService.decryptText(encryptedTag, key);
                const tag = JSON.parse(tagJson);

                if (typeof tag === 'string' && tag.trim().length > 0) {
                  decryptedTags.push(tag);
                }
              } catch (error) {
                console.error(`[versionService] Failed to decrypt tag[${i}]:`, error);
              }
            }

            tags = decryptedTags;
          }
        } catch (error) {
          console.error(`[versionService] Error decrypting tags:`, error);
          tags = [];
        }
      }

      return {
        ...version,
        content,
        tags,
        characterCount: content.length,
      };
    } catch (error) {
      console.error(`[versionService] Error decrypting version:`, error);
      throw new Error('Failed to decrypt version');
    }
  }

  /**
   * Count total versions for a note
   */
  async countVersionsForNote(noteId: string): Promise<number> {
    return await versionRepository.countVersionsForNote(noteId);
  }

  /**
   * Delete all versions for a note (when note is permanently deleted)
   */
  async deleteVersionsForNote(noteId: string): Promise<void> {
    await versionRepository.deleteVersionsForNote(noteId);
  }
}

/**
 * Singleton instance
 */
export const versionService = new VersionService();
