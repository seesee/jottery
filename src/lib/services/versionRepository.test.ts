/**
 * Comprehensive Version Repository tests
 * Tests note version history management
 */

import { describe, test, expect, beforeEach, afterEach } from 'vitest';
import { versionRepository } from './versionRepository';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';

describe('VersionRepository', () => {
  beforeEach(async () => {
    await initTestDB();
  });

  afterEach(async () => {
    await cleanupTestDB();
  });

  describe('createVersion', () => {
    test('should create a version snapshot', async () => {
      const note = createTestNote({ version: 1 });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      const version = await versionRepository.createVersion(note, metadata);

      expect(version).toBeDefined();
      expect(version?.noteId).toBe(note.id);
      expect(version?.version).toBe(1);
      expect(version?.content).toBe(note.content);
      expect(version?.tags).toEqual(note.tags);
      expect(version?.reason).toBe('sync');
    });

    test('should generate correct versionKey', async () => {
      const note = createTestNote({ version: 5 });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      const version = await versionRepository.createVersion(note, metadata);

      expect(version?.versionKey).toBe(`${note.id}:5`);
    });

    test('should include all note fields', async () => {
      const note = createTestNote({
        version: 1,
        content: 'Test content',
        tags: ['tag1', 'tag2'],
        attachments: [],
        syntaxLanguage: 'javascript',
        wordWrap: false,
      });
      const metadata = {
        syncedAt: '2024-01-01T10:00:00Z',
        reason: 'manual-sync' as const,
      };

      const version = await versionRepository.createVersion(note, metadata);

      expect(version?.content).toBe('Test content');
      expect(version?.tags).toEqual(['tag1', 'tag2']);
      expect(version?.syntaxLanguage).toBe('javascript');
      expect(version?.wordWrap).toBe(false);
      expect(version?.syncedAt).toBe('2024-01-01T10:00:00Z');
      expect(version?.reason).toBe('manual-sync');
    });

    test('should skip duplicate version if content unchanged', async () => {
      const note = createTestNote({ version: 1, content: 'Same content' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      // Create first version
      const version1 = await versionRepository.createVersion(note, metadata);
      expect(version1).toBeDefined();

      // Try to create duplicate (same content)
      const version2 = await versionRepository.createVersion(note, metadata);
      expect(version2).toBeNull(); // Should skip
    });

    test('should create new version if content changed', async () => {
      const note = createTestNote({ version: 1, content: 'Original content' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      // Create first version
      await versionRepository.createVersion(note, metadata);

      // Change content and create new version
      note.content = 'Updated content';
      note.version = 2;
      const version2 = await versionRepository.createVersion(note, metadata);

      expect(version2).toBeDefined();
      expect(version2?.content).toBe('Updated content');
    });

    test('should create multiple versions with incrementing version numbers', async () => {
      const note = createTestNote({ version: 1, content: 'Version 1' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      note.content = 'Version 2';
      note.version = 2;
      await versionRepository.createVersion(note, metadata);

      note.content = 'Version 3';
      note.version = 3;
      await versionRepository.createVersion(note, metadata);

      const versions = await versionRepository.getVersionsForNote(note.id);
      expect(versions).toHaveLength(3);
    });
  });

  describe('getVersionsForNote', () => {
    test('should return empty array when no versions exist', async () => {
      const versions = await versionRepository.getVersionsForNote('non-existent-id');

      expect(versions).toEqual([]);
    });

    test('should return all versions for a note', async () => {
      const note = createTestNote({ version: 1, content: 'V1' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      note.content = 'V2';
      note.version = 2;
      await versionRepository.createVersion(note, metadata);

      note.content = 'V3';
      note.version = 3;
      await versionRepository.createVersion(note, metadata);

      const versions = await versionRepository.getVersionsForNote(note.id);

      expect(versions).toHaveLength(3);
    });

    test('should return versions sorted by version number (newest first)', async () => {
      const note = createTestNote({ version: 1, content: 'V1' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      note.content = 'V2';
      note.version = 2;
      await versionRepository.createVersion(note, metadata);

      note.content = 'V3';
      note.version = 3;
      await versionRepository.createVersion(note, metadata);

      const versions = await versionRepository.getVersionsForNote(note.id);

      expect(versions[0].version).toBe(3);
      expect(versions[1].version).toBe(2);
      expect(versions[2].version).toBe(1);
    });

    test('should only return versions for specified note', async () => {
      const note1 = createTestNote({ version: 1 });
      const note2 = createTestNote({ version: 1 });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note1, metadata);
      await versionRepository.createVersion(note2, metadata);

      const versionsForNote1 = await versionRepository.getVersionsForNote(note1.id);
      const versionsForNote2 = await versionRepository.getVersionsForNote(note2.id);

      expect(versionsForNote1).toHaveLength(1);
      expect(versionsForNote2).toHaveLength(1);
      expect(versionsForNote1[0].noteId).toBe(note1.id);
      expect(versionsForNote2[0].noteId).toBe(note2.id);
    });
  });

  describe('getVersion', () => {
    test('should retrieve specific version', async () => {
      const note = createTestNote({ version: 1, content: 'Version 1' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      note.content = 'Version 2';
      note.version = 2;
      await versionRepository.createVersion(note, metadata);

      const version1 = await versionRepository.getVersion(note.id, 1);
      const version2 = await versionRepository.getVersion(note.id, 2);

      expect(version1?.content).toBe('Version 1');
      expect(version2?.content).toBe('Version 2');
    });

    test('should return null for non-existent version', async () => {
      const note = createTestNote({ version: 1 });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      const version = await versionRepository.getVersion(note.id, 999);

      expect(version).toBeNull();
    });

    test('should return null for non-existent note', async () => {
      const version = await versionRepository.getVersion('non-existent', 1);

      expect(version).toBeNull();
    });
  });

  describe('getLatestVersion', () => {
    test('should return the latest version', async () => {
      const note = createTestNote({ version: 1, content: 'V1' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      note.content = 'V2';
      note.version = 2;
      await versionRepository.createVersion(note, metadata);

      note.content = 'V3 - Latest';
      note.version = 3;
      await versionRepository.createVersion(note, metadata);

      const latest = await versionRepository.getLatestVersion(note.id);

      expect(latest).toBeDefined();
      expect(latest?.version).toBe(3);
      expect(latest?.content).toBe('V3 - Latest');
    });

    test('should return null when no versions exist', async () => {
      const latest = await versionRepository.getLatestVersion('non-existent');

      expect(latest).toBeNull();
    });

    test('should return only version when single version exists', async () => {
      const note = createTestNote({ version: 1 });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      const latest = await versionRepository.getLatestVersion(note.id);

      expect(latest).toBeDefined();
      expect(latest?.version).toBe(1);
    });
  });

  describe('deleteVersionsForNote', () => {
    test('should delete all versions for a note', async () => {
      const note = createTestNote({ version: 1, content: 'V1' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      note.content = 'V2';
      note.version = 2;
      await versionRepository.createVersion(note, metadata);

      await versionRepository.deleteVersionsForNote(note.id);

      const versions = await versionRepository.getVersionsForNote(note.id);
      expect(versions).toHaveLength(0);
    });

    test('should not affect other notes versions', async () => {
      const note1 = createTestNote({ version: 1 });
      const note2 = createTestNote({ version: 1 });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note1, metadata);
      await versionRepository.createVersion(note2, metadata);

      await versionRepository.deleteVersionsForNote(note1.id);

      const versionsNote1 = await versionRepository.getVersionsForNote(note1.id);
      const versionsNote2 = await versionRepository.getVersionsForNote(note2.id);

      expect(versionsNote1).toHaveLength(0);
      expect(versionsNote2).toHaveLength(1);
    });

    test('should handle deletion of non-existent note gracefully', async () => {
      await expect(
        versionRepository.deleteVersionsForNote('non-existent')
      ).resolves.not.toThrow();
    });
  });

  describe('countVersionsForNote', () => {
    test('should return 0 when no versions exist', async () => {
      const count = await versionRepository.countVersionsForNote('non-existent');

      expect(count).toBe(0);
    });

    test('should count all versions for a note', async () => {
      const note = createTestNote({ version: 1, content: 'V1' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      note.content = 'V2';
      note.version = 2;
      await versionRepository.createVersion(note, metadata);

      note.content = 'V3';
      note.version = 3;
      await versionRepository.createVersion(note, metadata);

      const count = await versionRepository.countVersionsForNote(note.id);

      expect(count).toBe(3);
    });

    test('should update count after deletion', async () => {
      const note = createTestNote({ version: 1 });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      note.content = 'V2';
      note.version = 2;
      await versionRepository.createVersion(note, metadata);

      await versionRepository.deleteVersionsForNote(note.id);

      const count = await versionRepository.countVersionsForNote(note.id);
      expect(count).toBe(0);
    });
  });

  describe('Edge Cases', () => {
    test('should handle versions with identical content but different metadata', async () => {
      const note = createTestNote({ version: 1, content: 'Same content' });

      // First version
      const version1 = await versionRepository.createVersion(note, {
        syncedAt: '2024-01-01T10:00:00Z',
        reason: 'sync',
      });

      // Second version (skipped - same content)
      const version2 = await versionRepository.createVersion(note, {
        syncedAt: '2024-01-01T11:00:00Z',
        reason: 'manual-sync',
      });

      expect(version1).toBeDefined();
      expect(version2).toBeNull(); // Deduplication
    });

    test('should handle unicode content in versions', async () => {
      const note = createTestNote({
        version: 1,
        content: '🔒 Test 中文 Ελληνικά',
        tags: ['🏷️', 'unicode'],
      });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      const retrieved = await versionRepository.getVersion(note.id, 1);
      expect(retrieved?.content).toBe('🔒 Test 中文 Ελληνικά');
      expect(retrieved?.tags).toEqual(['🏷️', 'unicode']);
    });

    test('should handle large content in versions', async () => {
      const note = createTestNote({
        version: 1,
        content: 'A'.repeat(100000), // 100KB
      });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      const retrieved = await versionRepository.getVersion(note.id, 1);
      expect(retrieved?.content.length).toBe(100000);
    });

    test('should handle many versions for single note', async () => {
      const note = createTestNote({ version: 1, content: 'V1' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      // Create 20 versions
      for (let i = 1; i <= 20; i++) {
        note.content = `Version ${i}`;
        note.version = i;
        await versionRepository.createVersion(note, metadata);
      }

      const versions = await versionRepository.getVersionsForNote(note.id);
      expect(versions).toHaveLength(20);
      expect(versions[0].version).toBe(20); // Newest first
    });

    test('should handle version with empty tags', async () => {
      const note = createTestNote({ version: 1, tags: [] });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await versionRepository.createVersion(note, metadata);

      const retrieved = await versionRepository.getVersion(note.id, 1);
      expect(retrieved?.tags).toEqual([]);
    });

    test('should handle concurrent version creation', async () => {
      const note1 = createTestNote({ version: 1, content: 'Note 1' });
      const note2 = createTestNote({ version: 1, content: 'Note 2' });
      const metadata = {
        syncedAt: new Date().toISOString(),
        reason: 'sync' as const,
      };

      await Promise.all([
        versionRepository.createVersion(note1, metadata),
        versionRepository.createVersion(note2, metadata),
      ]);

      const versions1 = await versionRepository.getVersionsForNote(note1.id);
      const versions2 = await versionRepository.getVersionsForNote(note2.id);

      expect(versions1).toHaveLength(1);
      expect(versions2).toHaveLength(1);
    });
  });

  describe('Integration Scenarios', () => {
    test('should support typical sync version workflow', async () => {
      const note = createTestNote({ version: 1, content: 'Original' });

      // Create initial version during first sync
      const v1 = await versionRepository.createVersion(note, {
        syncedAt: '2024-01-01T10:00:00Z',
        reason: 'sync',
      });
      expect(v1).toBeDefined();

      // User modifies note locally
      note.content = 'Modified locally';
      note.version = 2;

      // Create version during second sync
      const v2 = await versionRepository.createVersion(note, {
        syncedAt: '2024-01-01T11:00:00Z',
        reason: 'sync',
      });
      expect(v2).toBeDefined();

      // Verify version history
      const versions = await versionRepository.getVersionsForNote(note.id);
      expect(versions).toHaveLength(2);

      // Can restore to previous version if needed
      const oldVersion = await versionRepository.getVersion(note.id, 1);
      expect(oldVersion?.content).toBe('Original');
    });

    test('should deduplicate when sync happens with no changes', async () => {
      const note = createTestNote({ version: 1, content: 'Unchanged' });

      // First sync
      await versionRepository.createVersion(note, {
        syncedAt: '2024-01-01T10:00:00Z',
        reason: 'sync',
      });

      // Second sync with same content (should skip)
      const v2 = await versionRepository.createVersion(note, {
        syncedAt: '2024-01-01T11:00:00Z',
        reason: 'sync',
      });

      expect(v2).toBeNull();

      const versions = await versionRepository.getVersionsForNote(note.id);
      expect(versions).toHaveLength(1); // Only one version
    });
  });
});
