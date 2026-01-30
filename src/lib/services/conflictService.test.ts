/**
 * Tests for conflictService
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';
import { storeConflict, getConflicts, getConflictInfo, resolveKeepMine, resolveKeepServer, resolveKeepBoth } from './conflictService';
import { noteRepository } from './noteRepository';
import { syncRepository } from './syncRepository';
import { keyManager } from './keyManager';
import { cryptoService, decryptStringArray } from './crypto';
import type { ConflictData } from '../types';

describe('conflictService', () => {
  let masterKey: CryptoKey;

  beforeEach(async () => {
    const { masterKey: key } = await initTestDB();
    masterKey = key;
    // Mock keyManager to return our test key
    vi.spyOn(keyManager, 'getMasterKey').mockReturnValue({ key: masterKey, derivedAt: Date.now() });
  });

  afterEach(async () => {
    vi.restoreAllMocks();
    await cleanupTestDB();
  });

  const createTestConflictData = async (noteId: string): Promise<ConflictData> => {
    // Create encrypted content for testing
    const serverContent = 'Server version of the note';
    const encryptedContent = await cryptoService.encryptText(serverContent, masterKey);

    return {
      noteId,
      serverContent: JSON.stringify(encryptedContent),
      serverTags: [],
      serverModifiedAt: new Date().toISOString(),
      serverVersion: 2,
      serverAttachments: [],
      serverPinned: false,
      serverSyntaxLanguage: 'plain',
      serverWordWrap: true,
      detectedAt: new Date().toISOString(),
    };
  };

  describe('storeConflict', () => {
    it('should store conflict data for a note', async () => {
      const note = createTestNote({ content: 'Local content' });
      await noteRepository.create(note);

      await storeConflict(note.id, {
        serverContent: 'encrypted-server-content',
        serverTags: ['tag1'],
        serverModifiedAt: new Date().toISOString(),
        serverVersion: 2,
        serverAttachments: [],
        serverPinned: true,
        serverSyntaxLanguage: 'markdown',
        serverWordWrap: false,
      });

      const syncMeta = await syncRepository.getNoteSyncMetadata(note.id);
      expect(syncMeta).not.toBeNull();
      expect(syncMeta?.lastSyncStatus).toBe('conflict');
      expect(syncMeta?.conflictData).toBeDefined();
      expect(syncMeta?.conflictData?.serverContent).toBe('encrypted-server-content');
      expect(syncMeta?.conflictData?.serverPinned).toBe(true);
    });

    it('should set error message when storing conflict', async () => {
      const note = createTestNote();
      await noteRepository.create(note);

      await storeConflict(note.id, {
        serverContent: 'content',
        serverTags: [],
        serverModifiedAt: new Date().toISOString(),
        serverVersion: 1,
        serverAttachments: [],
        serverPinned: false,
      });

      const syncMeta = await syncRepository.getNoteSyncMetadata(note.id);
      expect(syncMeta?.errorMessage).toContain('manual resolution required');
    });
  });

  describe('getConflicts', () => {
    it('should return empty array when no conflicts exist', async () => {
      const conflicts = await getConflicts();
      expect(conflicts).toEqual([]);
    });

    it('should return note IDs with conflict status', async () => {
      const note1 = createTestNote({ id: 'note-1' });
      const note2 = createTestNote({ id: 'note-2' });
      await noteRepository.create(note1);
      await noteRepository.create(note2);

      // Set note1 as conflict
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      // Set note2 as synced (no conflict)
      await syncRepository.updateNoteSyncMetadata('note-2', {
        noteId: 'note-2',
        lastSyncStatus: 'synced',
      });

      const conflicts = await getConflicts();
      expect(conflicts).toHaveLength(1);
      expect(conflicts).toContain('note-1');
    });

    it('should not include notes without conflictData', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      // Set conflict status but no conflictData
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        // No conflictData
      });

      const conflicts = await getConflicts();
      expect(conflicts).toHaveLength(0);
    });
  });

  describe('getConflictInfo', () => {
    it('should return null if note does not exist', async () => {
      const info = await getConflictInfo('non-existent');
      expect(info).toBeNull();
    });

    it('should return null if no conflict data exists', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      const info = await getConflictInfo('note-1');
      expect(info).toBeNull();
    });

    it('should return conflict info with decrypted content', async () => {
      const note = createTestNote({ id: 'note-1', content: 'Local content' });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-1');
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const info = await getConflictInfo('note-1');
      expect(info).not.toBeNull();
      expect(info?.noteId).toBe('note-1');
      expect(info?.localNote.content).toBe('Local content');
      expect(info?.serverContent).toBe('Server version of the note');
    });

    it('should throw if application is locked', async () => {
      vi.spyOn(keyManager, 'getMasterKey').mockReturnValue(null);

      await expect(getConflictInfo('note-1')).rejects.toThrow('Application is locked');
    });
  });

  describe('resolveKeepMine', () => {
    it('should clear conflict and mark note for sync', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      await resolveKeepMine('note-1');

      const syncMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(syncMeta?.lastSyncStatus).toBe('pending');
      expect(syncMeta?.conflictData).toBeUndefined();
    });

    it('should update note modifiedAt timestamp', async () => {
      const oldDate = '2020-01-01T00:00:00.000Z';
      const note = createTestNote({ id: 'note-1', modifiedAt: oldDate });
      await noteRepository.create(note);

      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      await resolveKeepMine('note-1');

      const updatedNote = await noteRepository.getById('note-1');
      expect(new Date(updatedNote!.modifiedAt).getTime()).toBeGreaterThan(new Date(oldDate).getTime());
    });

    it('should throw if note does not exist', async () => {
      await expect(resolveKeepMine('non-existent')).rejects.toThrow('not found');
    });
  });

  describe('resolveKeepServer', () => {
    it('should update note with server content', async () => {
      const note = createTestNote({ id: 'note-1', content: 'Local content', pinned: false });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-1');
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      await resolveKeepServer('note-1');

      const updatedNote = await noteRepository.getById('note-1');
      // Note: Content will be encrypted server content
      expect(updatedNote?.content).toBe(conflictData.serverContent);
    });

    it('should clear conflict and mark as synced', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      await resolveKeepServer('note-1');

      const syncMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(syncMeta?.lastSyncStatus).toBe('synced');
      expect(syncMeta?.conflictData).toBeUndefined();
    });

    it('should throw if no conflict data exists', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await expect(resolveKeepServer('note-1')).rejects.toThrow('not found');
    });

    it('should throw if application is locked', async () => {
      vi.spyOn(keyManager, 'getMasterKey').mockReturnValue(null);

      await expect(resolveKeepServer('note-1')).rejects.toThrow('Application is locked');
    });
  });

  describe('resolveKeepBoth', () => {
    it('should create new note from server version', async () => {
      const note = createTestNote({ id: 'note-1', content: 'Local content' });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-1');
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const newNoteId = await resolveKeepBoth('note-1');

      expect(newNoteId).toBeDefined();
      expect(newNoteId).not.toBe('note-1');

      // Original note should still exist
      const originalNote = await noteRepository.getById('note-1');
      expect(originalNote).not.toBeNull();

      // New note should exist
      const newNote = await noteRepository.getById(newNoteId);
      expect(newNote).not.toBeNull();
    });

    it('should add conflict-copy tag to new note', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      const conflictData = await createTestConflictData('note-1');
      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData,
      });

      const newNoteId = await resolveKeepBoth('note-1');

      const newNote = await noteRepository.getById(newNoteId);
      expect(newNote).not.toBeNull();

      // Decrypt tags to verify the conflict-copy tag was added
      const encryptedTags = JSON.parse(newNote!.tags[0]);
      const decryptedTags = await decryptStringArray(encryptedTags, masterKey);
      expect(decryptedTags).toContain('conflict-copy');
    });

    it('should clear conflict on original note', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await syncRepository.updateNoteSyncMetadata('note-1', {
        noteId: 'note-1',
        lastSyncStatus: 'conflict',
        conflictData: await createTestConflictData('note-1'),
      });

      await resolveKeepBoth('note-1');

      const syncMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(syncMeta?.lastSyncStatus).toBe('pending');
      expect(syncMeta?.conflictData).toBeUndefined();
    });

    it('should throw if no conflict data exists', async () => {
      const note = createTestNote({ id: 'note-1' });
      await noteRepository.create(note);

      await expect(resolveKeepBoth('note-1')).rejects.toThrow('not found');
    });
  });

  describe('Integration Scenarios', () => {
    it('should handle full conflict detection and resolution workflow', async () => {
      // Create note
      const note = createTestNote({ id: 'note-1', content: 'Original content' });
      await noteRepository.create(note);

      // Simulate conflict detected during sync
      await storeConflict('note-1', {
        serverContent: 'encrypted-server-content',
        serverTags: [],
        serverModifiedAt: new Date().toISOString(),
        serverVersion: 2,
        serverAttachments: [],
        serverPinned: false,
      });

      // Verify conflict exists
      let conflicts = await getConflicts();
      expect(conflicts).toContain('note-1');

      // Resolve conflict
      await resolveKeepMine('note-1');

      // Verify conflict is resolved
      conflicts = await getConflicts();
      expect(conflicts).not.toContain('note-1');

      const syncMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(syncMeta?.lastSyncStatus).toBe('pending');
    });

    it('should handle multiple concurrent conflicts', async () => {
      // Create multiple notes
      const note1 = createTestNote({ id: 'note-1' });
      const note2 = createTestNote({ id: 'note-2' });
      const note3 = createTestNote({ id: 'note-3' });
      await noteRepository.create(note1);
      await noteRepository.create(note2);
      await noteRepository.create(note3);

      // Set conflicts on all
      for (const noteId of ['note-1', 'note-2', 'note-3']) {
        await storeConflict(noteId, {
          serverContent: `server-${noteId}`,
          serverTags: [],
          serverModifiedAt: new Date().toISOString(),
          serverVersion: 2,
          serverAttachments: [],
          serverPinned: false,
        });
      }

      // Verify all conflicts exist
      let conflicts = await getConflicts();
      expect(conflicts).toHaveLength(3);

      // Resolve conflicts with different strategies
      await resolveKeepMine('note-1');
      await resolveKeepServer('note-2');
      await resolveKeepBoth('note-3');

      // Verify all resolved
      conflicts = await getConflicts();
      expect(conflicts).toHaveLength(0);
    });
  });
});
