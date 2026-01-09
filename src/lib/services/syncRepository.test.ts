/**
 * Tests for syncRepository
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { syncRepository } from './syncRepository';
import type { SyncMetadata, NoteSyncMetadata, ConflictData } from '../types';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';

describe('syncRepository', () => {
  beforeEach(async () => {
    await initTestDB();
  });

  afterEach(async () => {
    await cleanupTestDB();
  });

  const createTestSyncMetadata = (): SyncMetadata => ({
    syncEnabled: true,
    syncEndpoint: 'https://sync.example.com',
    lastSyncAt: new Date().toISOString(),
    apiKey: 'test-api-key',
    clientId: 'test-client-id',
    userId: 'test-user-id',
    userEmail: 'test@example.com',
    autoSyncInterval: 5,
  });

  const createTestNoteSyncMetadata = (noteId: string): NoteSyncMetadata => ({
    noteId,
    syncedAt: new Date().toISOString(),
    syncHash: 'test-hash-' + noteId,
    serverVersion: 1,
    lastSyncStatus: 'synced',
  });

  describe('Global Sync Metadata', () => {
    describe('getMetadata', () => {
      it('should return null when no metadata exists', async () => {
        const metadata = await syncRepository.getMetadata();
        expect(metadata).toBeNull();
      });

      it('should return stored metadata when it exists', async () => {
        const testMetadata = createTestSyncMetadata();
        await syncRepository.updateMetadata(testMetadata);

        const retrieved = await syncRepository.getMetadata();
        expect(retrieved).toEqual(testMetadata);
      });

      it('should preserve all metadata fields', async () => {
        const testMetadata = createTestSyncMetadata();
        await syncRepository.updateMetadata(testMetadata);

        const retrieved = await syncRepository.getMetadata();
        expect(retrieved?.syncEnabled).toBe(testMetadata.syncEnabled);
        expect(retrieved?.syncEndpoint).toBe(testMetadata.syncEndpoint);
        expect(retrieved?.lastSyncAt).toBe(testMetadata.lastSyncAt);
        expect(retrieved?.apiKey).toBe(testMetadata.apiKey);
        expect(retrieved?.clientId).toBe(testMetadata.clientId);
        expect(retrieved?.userId).toBe(testMetadata.userId);
        expect(retrieved?.userEmail).toBe(testMetadata.userEmail);
        expect(retrieved?.autoSyncInterval).toBe(testMetadata.autoSyncInterval);
      });
    });

    describe('updateMetadata', () => {
      it('should store metadata correctly', async () => {
        const testMetadata = createTestSyncMetadata();
        const updated = await syncRepository.updateMetadata(testMetadata);

        expect(updated).toEqual(testMetadata);
      });

      it('should perform partial update', async () => {
        await syncRepository.updateMetadata({
          syncEnabled: false,
          syncEndpoint: 'https://old.example.com',
        });

        const updated = await syncRepository.updateMetadata({
          syncEnabled: true,
        });

        expect(updated.syncEnabled).toBe(true);
        expect(updated.syncEndpoint).toBe('https://old.example.com');
      });

      it('should merge with existing metadata', async () => {
        await syncRepository.updateMetadata({
          syncEnabled: true,
          syncEndpoint: 'https://sync.example.com',
        });

        await syncRepository.updateMetadata({
          apiKey: 'new-api-key',
          clientId: 'new-client-id',
        });

        const metadata = await syncRepository.getMetadata();
        expect(metadata?.syncEnabled).toBe(true);
        expect(metadata?.syncEndpoint).toBe('https://sync.example.com');
        expect(metadata?.apiKey).toBe('new-api-key');
        expect(metadata?.clientId).toBe('new-client-id');
      });

      it('should update timestamps independently', async () => {
        const now = new Date().toISOString();
        await syncRepository.updateMetadata({
          syncEnabled: true,
          syncEndpoint: 'https://sync.example.com',
          lastSyncAt: now,
        });

        const laterTime = new Date(Date.now() + 1000).toISOString();
        await syncRepository.updateMetadata({
          lastPushAt: laterTime,
        });

        const metadata = await syncRepository.getMetadata();
        expect(metadata?.lastSyncAt).toBe(now);
        expect(metadata?.lastPushAt).toBe(laterTime);
      });

      it('should handle empty update', async () => {
        const initial = await syncRepository.updateMetadata({
          syncEnabled: true,
          syncEndpoint: 'https://sync.example.com',
        });

        const updated = await syncRepository.updateMetadata({});
        expect(updated).toEqual(initial);
      });

      it('should update auto sync interval', async () => {
        await syncRepository.updateMetadata({
          syncEnabled: true,
          syncEndpoint: 'https://sync.example.com',
          autoSyncInterval: 10,
        });

        const metadata = await syncRepository.getMetadata();
        expect(metadata?.autoSyncInterval).toBe(10);
      });

      it('should disable auto sync when interval is 0', async () => {
        await syncRepository.updateMetadata({
          syncEnabled: true,
          syncEndpoint: 'https://sync.example.com',
          autoSyncInterval: 0,
        });

        const metadata = await syncRepository.getMetadata();
        expect(metadata?.autoSyncInterval).toBe(0);
      });
    });
  });

  describe('Note Sync Metadata', () => {
    describe('getNoteSyncMetadata', () => {
      it('should return null when no metadata exists', async () => {
        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata).toBeNull();
      });

      it('should return stored metadata when it exists', async () => {
        const testMetadata = createTestNoteSyncMetadata('note-1');
        await syncRepository.updateNoteSyncMetadata('note-1', testMetadata);

        const retrieved = await syncRepository.getNoteSyncMetadata('note-1');
        expect(retrieved).toEqual(testMetadata);
      });

      it('should preserve all note metadata fields', async () => {
        const testMetadata = createTestNoteSyncMetadata('note-1');
        await syncRepository.updateNoteSyncMetadata('note-1', testMetadata);

        const retrieved = await syncRepository.getNoteSyncMetadata('note-1');
        expect(retrieved?.noteId).toBe(testMetadata.noteId);
        expect(retrieved?.syncedAt).toBe(testMetadata.syncedAt);
        expect(retrieved?.syncHash).toBe(testMetadata.syncHash);
        expect(retrieved?.serverVersion).toBe(testMetadata.serverVersion);
        expect(retrieved?.lastSyncStatus).toBe(testMetadata.lastSyncStatus);
      });

      it('should handle multiple notes independently', async () => {
        const metadata1 = createTestNoteSyncMetadata('note-1');
        const metadata2 = createTestNoteSyncMetadata('note-2');

        await syncRepository.updateNoteSyncMetadata('note-1', metadata1);
        await syncRepository.updateNoteSyncMetadata('note-2', metadata2);

        const retrieved1 = await syncRepository.getNoteSyncMetadata('note-1');
        const retrieved2 = await syncRepository.getNoteSyncMetadata('note-2');

        expect(retrieved1?.noteId).toBe('note-1');
        expect(retrieved2?.noteId).toBe('note-2');
      });
    });

    describe('updateNoteSyncMetadata', () => {
      it('should create metadata if it does not exist', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          syncHash: 'new-hash',
          lastSyncStatus: 'synced',
        });

        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.syncHash).toBe('new-hash');
        expect(metadata?.lastSyncStatus).toBe('synced');
      });

      it('should update existing metadata', async () => {
        const original = createTestNoteSyncMetadata('note-1');
        await syncRepository.updateNoteSyncMetadata('note-1', original);

        await syncRepository.updateNoteSyncMetadata('note-1', {
          serverVersion: 2,
          lastSyncStatus: 'pending',
        });

        const updated = await syncRepository.getNoteSyncMetadata('note-1');
        expect(updated?.serverVersion).toBe(2);
        expect(updated?.lastSyncStatus).toBe('pending');
        expect(updated?.noteId).toBe('note-1');
      });

      it('should handle all sync statuses', async () => {
        const statuses: NoteSyncMetadata['lastSyncStatus'][] = ['synced', 'pending', 'conflict', 'error'];

        for (const status of statuses) {
          await syncRepository.updateNoteSyncMetadata('note-1', {
            lastSyncStatus: status,
          });

          const metadata = await syncRepository.getNoteSyncMetadata('note-1');
          expect(metadata?.lastSyncStatus).toBe(status);
        }
      });

      it('should store error message', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'error',
          errorMessage: 'Network timeout',
        });

        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.lastSyncStatus).toBe('error');
        expect(metadata?.errorMessage).toBe('Network timeout');
      });

      it('should update sync hash', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          syncHash: 'hash1',
        });

        let metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.syncHash).toBe('hash1');

        await syncRepository.updateNoteSyncMetadata('note-1', {
          syncHash: 'hash2',
        });

        metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.syncHash).toBe('hash2');
      });

      it('should increment server version', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          serverVersion: 1,
        });

        await syncRepository.updateNoteSyncMetadata('note-1', {
          serverVersion: 2,
        });

        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.serverVersion).toBe(2);
      });
    });

    describe('deleteNoteSyncMetadata', () => {
      it('should delete note metadata', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', createTestNoteSyncMetadata('note-1'));

        await syncRepository.deleteNoteSyncMetadata('note-1');

        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata).toBeNull();
      });

      it('should not throw when deleting non-existent metadata', async () => {
        await expect(syncRepository.deleteNoteSyncMetadata('non-existent')).resolves.not.toThrow();
      });

      it('should not affect other notes', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', createTestNoteSyncMetadata('note-1'));
        await syncRepository.updateNoteSyncMetadata('note-2', createTestNoteSyncMetadata('note-2'));

        await syncRepository.deleteNoteSyncMetadata('note-1');

        const metadata1 = await syncRepository.getNoteSyncMetadata('note-1');
        const metadata2 = await syncRepository.getNoteSyncMetadata('note-2');

        expect(metadata1).toBeNull();
        expect(metadata2).not.toBeNull();
      });
    });
  });

  describe('Batch Operations', () => {
    describe('getPendingNotes', () => {
      it('should return empty array when no pending notes', async () => {
        const pending = await syncRepository.getPendingNotes();
        expect(pending).toEqual([]);
      });

      it('should return notes with pending status', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'pending',
        });
        await syncRepository.updateNoteSyncMetadata('note-2', {
          lastSyncStatus: 'synced',
        });
        await syncRepository.updateNoteSyncMetadata('note-3', {
          lastSyncStatus: 'pending',
        });

        const pending = await syncRepository.getPendingNotes();
        expect(pending).toHaveLength(2);
        expect(pending).toContain('note-1');
        expect(pending).toContain('note-3');
      });

      it('should not include synced notes', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'synced',
        });
        await syncRepository.updateNoteSyncMetadata('note-2', {
          lastSyncStatus: 'synced',
        });

        const pending = await syncRepository.getPendingNotes();
        expect(pending).toEqual([]);
      });

      it('should not include conflict notes', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'conflict',
        });
        await syncRepository.updateNoteSyncMetadata('note-2', {
          lastSyncStatus: 'pending',
        });

        const pending = await syncRepository.getPendingNotes();
        expect(pending).toHaveLength(1);
        expect(pending).toContain('note-2');
      });

      it('should not include error notes', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'error',
        });
        await syncRepository.updateNoteSyncMetadata('note-2', {
          lastSyncStatus: 'pending',
        });

        const pending = await syncRepository.getPendingNotes();
        expect(pending).toHaveLength(1);
        expect(pending).toContain('note-2');
      });
    });

    describe('getConflictCount', () => {
      it('should return 0 when no conflicts', async () => {
        const count = await syncRepository.getConflictCount();
        expect(count).toBe(0);
      });

      it('should count notes with conflict status', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'conflict',
        });
        await syncRepository.updateNoteSyncMetadata('note-2', {
          lastSyncStatus: 'synced',
        });
        await syncRepository.updateNoteSyncMetadata('note-3', {
          lastSyncStatus: 'conflict',
        });

        const count = await syncRepository.getConflictCount();
        expect(count).toBe(2);
      });

      it('should not count pending notes', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'pending',
        });

        const count = await syncRepository.getConflictCount();
        expect(count).toBe(0);
      });

      it('should not count error notes', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'error',
        });

        const count = await syncRepository.getConflictCount();
        expect(count).toBe(0);
      });

      it('should update count after resolving conflicts', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'conflict',
        });

        let count = await syncRepository.getConflictCount();
        expect(count).toBe(1);

        await syncRepository.updateNoteSyncMetadata('note-1', {
          lastSyncStatus: 'synced',
        });

        count = await syncRepository.getConflictCount();
        expect(count).toBe(0);
      });
    });

    describe('clearAll', () => {
      it('should clear all sync metadata', async () => {
        await syncRepository.updateMetadata(createTestSyncMetadata());
        await syncRepository.updateNoteSyncMetadata('note-1', createTestNoteSyncMetadata('note-1'));
        await syncRepository.updateNoteSyncMetadata('note-2', createTestNoteSyncMetadata('note-2'));

        await syncRepository.clearAll();

        const globalMetadata = await syncRepository.getMetadata();
        const noteMetadata1 = await syncRepository.getNoteSyncMetadata('note-1');
        const noteMetadata2 = await syncRepository.getNoteSyncMetadata('note-2');

        expect(globalMetadata).toBeNull();
        expect(noteMetadata1).toBeNull();
        expect(noteMetadata2).toBeNull();
      });

      it('should not throw when clearing empty store', async () => {
        await expect(syncRepository.clearAll()).resolves.not.toThrow();
      });

      it('should allow re-initialization after clear', async () => {
        await syncRepository.updateMetadata(createTestSyncMetadata());
        await syncRepository.clearAll();

        const newMetadata = {
          syncEnabled: true,
          syncEndpoint: 'https://new.example.com',
        };
        await syncRepository.updateMetadata(newMetadata);

        const retrieved = await syncRepository.getMetadata();
        expect(retrieved?.syncEnabled).toBe(true);
        expect(retrieved?.syncEndpoint).toBe('https://new.example.com');
      });
    });
  });

  describe('Integration Scenarios', () => {
    it('should support full sync setup workflow', async () => {
      // Setup sync
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: 'https://sync.example.com',
        apiKey: 'test-key',
        clientId: 'client-123',
      });

      // Create note sync metadata
      await syncRepository.updateNoteSyncMetadata('note-1', {
        syncHash: 'hash-1',
        serverVersion: 1,
        lastSyncStatus: 'synced',
      });

      // Verify setup
      const globalMeta = await syncRepository.getMetadata();
      expect(globalMeta?.syncEnabled).toBe(true);

      const noteMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(noteMeta?.lastSyncStatus).toBe('synced');
    });

    it('should support sync conflict workflow', async () => {
      // Note synced initially
      await syncRepository.updateNoteSyncMetadata('note-1', {
        lastSyncStatus: 'synced',
        syncHash: 'hash-1',
        serverVersion: 1,
      });

      // Conflict detected
      await syncRepository.updateNoteSyncMetadata('note-1', {
        lastSyncStatus: 'conflict',
      });

      // Check conflict count
      let count = await syncRepository.getConflictCount();
      expect(count).toBe(1);

      // Resolve conflict
      await syncRepository.updateNoteSyncMetadata('note-1', {
        lastSyncStatus: 'synced',
        syncHash: 'hash-2',
        serverVersion: 2,
      });

      // Verify resolved
      count = await syncRepository.getConflictCount();
      expect(count).toBe(0);
    });

    it('should support pending sync workflow', async () => {
      // Create pending notes
      await syncRepository.updateNoteSyncMetadata('note-1', {
        lastSyncStatus: 'pending',
      });
      await syncRepository.updateNoteSyncMetadata('note-2', {
        lastSyncStatus: 'pending',
      });

      // Get pending
      let pending = await syncRepository.getPendingNotes();
      expect(pending).toHaveLength(2);

      // Sync first note
      await syncRepository.updateNoteSyncMetadata('note-1', {
        lastSyncStatus: 'synced',
      });

      // Check remaining pending
      pending = await syncRepository.getPendingNotes();
      expect(pending).toHaveLength(1);
      expect(pending).toContain('note-2');
    });

    it('should support device re-registration workflow', async () => {
      // Initial setup
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: 'https://sync.example.com',
        apiKey: 'old-key',
        clientId: 'old-client',
      });

      // Create some note metadata
      await syncRepository.updateNoteSyncMetadata('note-1', {
        lastSyncStatus: 'synced',
      });

      // Re-register (clear all and start fresh)
      await syncRepository.clearAll();

      // New registration
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: 'https://sync.example.com',
        apiKey: 'new-key',
        clientId: 'new-client',
      });

      const metadata = await syncRepository.getMetadata();
      expect(metadata?.apiKey).toBe('new-key');
      expect(metadata?.clientId).toBe('new-client');

      const noteMeta = await syncRepository.getNoteSyncMetadata('note-1');
      expect(noteMeta).toBeNull();
    });

    it('should handle mixed sync statuses', async () => {
      await syncRepository.updateNoteSyncMetadata('note-1', {
        lastSyncStatus: 'synced',
      });
      await syncRepository.updateNoteSyncMetadata('note-2', {
        lastSyncStatus: 'pending',
      });
      await syncRepository.updateNoteSyncMetadata('note-3', {
        lastSyncStatus: 'conflict',
      });
      await syncRepository.updateNoteSyncMetadata('note-4', {
        lastSyncStatus: 'error',
        errorMessage: 'Network error',
      });

      const pending = await syncRepository.getPendingNotes();
      const conflicts = await syncRepository.getConflictCount();

      expect(pending).toHaveLength(1);
      expect(conflicts).toBe(1);
    });
  });

  describe('Conflict Data', () => {
    const createTestConflictData = (noteId: string): ConflictData => ({
      noteId,
      serverContent: 'encrypted-server-content',
      serverTags: ['tag1', 'tag2'],
      serverModifiedAt: new Date().toISOString(),
      serverVersion: 2,
      serverAttachments: [],
      serverPinned: true,
      serverSyntaxLanguage: 'markdown',
      serverWordWrap: false,
      detectedAt: new Date().toISOString(),
    });

    describe('storing conflict data', () => {
      it('should store conflict data with note sync metadata', async () => {
        const conflictData = createTestConflictData('note-1');

        await syncRepository.updateNoteSyncMetadata('note-1', {
          noteId: 'note-1',
          lastSyncStatus: 'conflict',
          conflictData,
        });

        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.conflictData).toBeDefined();
        expect(metadata?.conflictData?.serverContent).toBe('encrypted-server-content');
        expect(metadata?.conflictData?.serverVersion).toBe(2);
        expect(metadata?.conflictData?.serverPinned).toBe(true);
      });

      it('should preserve all conflict data fields', async () => {
        const conflictData = createTestConflictData('note-1');

        await syncRepository.updateNoteSyncMetadata('note-1', {
          noteId: 'note-1',
          lastSyncStatus: 'conflict',
          conflictData,
        });

        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.conflictData?.noteId).toBe(conflictData.noteId);
        expect(metadata?.conflictData?.serverContent).toBe(conflictData.serverContent);
        expect(metadata?.conflictData?.serverTags).toEqual(conflictData.serverTags);
        expect(metadata?.conflictData?.serverModifiedAt).toBe(conflictData.serverModifiedAt);
        expect(metadata?.conflictData?.serverVersion).toBe(conflictData.serverVersion);
        expect(metadata?.conflictData?.serverAttachments).toEqual(conflictData.serverAttachments);
        expect(metadata?.conflictData?.serverPinned).toBe(conflictData.serverPinned);
        expect(metadata?.conflictData?.serverSyntaxLanguage).toBe(conflictData.serverSyntaxLanguage);
        expect(metadata?.conflictData?.serverWordWrap).toBe(conflictData.serverWordWrap);
        expect(metadata?.conflictData?.detectedAt).toBe(conflictData.detectedAt);
      });

      it('should handle conflict data with attachments', async () => {
        const conflictData = createTestConflictData('note-1');
        conflictData.serverAttachments = [
          { id: 'att-1', filename: 'file.txt', mimeType: 'text/plain', size: 100, data: 'ref-1' },
        ];

        await syncRepository.updateNoteSyncMetadata('note-1', {
          noteId: 'note-1',
          lastSyncStatus: 'conflict',
          conflictData,
        });

        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.conflictData?.serverAttachments).toHaveLength(1);
        expect(metadata?.conflictData?.serverAttachments?.[0].id).toBe('att-1');
      });
    });

    describe('clearing conflict data', () => {
      it('should clear conflict data when set to undefined', async () => {
        const conflictData = createTestConflictData('note-1');

        await syncRepository.updateNoteSyncMetadata('note-1', {
          noteId: 'note-1',
          lastSyncStatus: 'conflict',
          conflictData,
        });

        // Verify conflict exists
        let metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.conflictData).toBeDefined();

        // Clear conflict
        await syncRepository.updateNoteSyncMetadata('note-1', {
          noteId: 'note-1',
          lastSyncStatus: 'synced',
          conflictData: undefined,
        });

        metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.conflictData).toBeUndefined();
      });

      it('should clear conflict data when status changes to synced', async () => {
        await syncRepository.updateNoteSyncMetadata('note-1', {
          noteId: 'note-1',
          lastSyncStatus: 'conflict',
          conflictData: createTestConflictData('note-1'),
        });

        await syncRepository.updateNoteSyncMetadata('note-1', {
          noteId: 'note-1',
          lastSyncStatus: 'synced',
          conflictData: undefined,
        });

        const metadata = await syncRepository.getNoteSyncMetadata('note-1');
        expect(metadata?.lastSyncStatus).toBe('synced');
        expect(metadata?.conflictData).toBeUndefined();
      });
    });

    describe('getConflictNotes', () => {
      it('should return notes with conflict status and data', async () => {
        // Create note with conflict
        await syncRepository.updateNoteSyncMetadata('note-1', {
          noteId: 'note-1',
          lastSyncStatus: 'conflict',
          conflictData: createTestConflictData('note-1'),
        });

        // Create note without conflict
        await syncRepository.updateNoteSyncMetadata('note-2', {
          noteId: 'note-2',
          lastSyncStatus: 'synced',
        });

        const count = await syncRepository.getConflictCount();
        expect(count).toBe(1);
      });
    });
  });

  describe('Edge Cases', () => {
    it('should handle very long endpoint URLs', async () => {
      const longUrl = 'https://' + 'a'.repeat(1000) + '.example.com';
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: longUrl,
      });

      const metadata = await syncRepository.getMetadata();
      expect(metadata?.syncEndpoint).toBe(longUrl);
    });

    it('should handle special characters in note IDs', async () => {
      const noteIds = ['note-with-dash', 'note_with_underscore', 'note.with.dot'];

      for (const noteId of noteIds) {
        await syncRepository.updateNoteSyncMetadata(noteId, {
          lastSyncStatus: 'synced',
        });

        const metadata = await syncRepository.getNoteSyncMetadata(noteId);
        expect(metadata?.noteId).toBe(noteId);
      }
    });

    it('should handle large server versions', async () => {
      await syncRepository.updateNoteSyncMetadata('note-1', {
        serverVersion: 999999,
      });

      const metadata = await syncRepository.getNoteSyncMetadata('note-1');
      expect(metadata?.serverVersion).toBe(999999);
    });

    it('should handle empty error messages', async () => {
      await syncRepository.updateNoteSyncMetadata('note-1', {
        lastSyncStatus: 'error',
        errorMessage: '',
      });

      const metadata = await syncRepository.getNoteSyncMetadata('note-1');
      expect(metadata?.errorMessage).toBe('');
    });

    it('should handle undefined optional fields', async () => {
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: 'https://sync.example.com',
        apiKey: undefined,
        clientId: undefined,
        userId: undefined,
      });

      const metadata = await syncRepository.getMetadata();
      expect(metadata?.apiKey).toBeUndefined();
      expect(metadata?.clientId).toBeUndefined();
      expect(metadata?.userId).toBeUndefined();
    });
  });
});
