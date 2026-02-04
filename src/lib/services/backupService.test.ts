/**
 * Tests for backupService
 * Focuses on backup creation, progress callbacks, and chunked JSON generation
 * Covers both v2.1 (legacy) and v3.0 (batched) formats
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  createBackup,
  createBatchedBackup,
  downloadBackup,
  type BackupProgressCallback,
  type BackupDataV2,
  type BackupDataV3,
} from './backupService';
import { noteService } from './noteService';
import { keyManager } from './keyManager';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';

// Type guard helpers for tests
function isBackupV2(backup: unknown): backup is BackupDataV2 {
  return typeof backup === 'object' && backup !== null && 'data' in backup && Array.isArray((backup as BackupDataV2).data);
}

function isBackupV3(backup: unknown): backup is BackupDataV3 {
  return typeof backup === 'object' && backup !== null && 'batches' in backup && Array.isArray((backup as BackupDataV3).batches);
}

describe('backupService', () => {
  let masterKey: CryptoKey;
  let keyBytes: Uint8Array;

  beforeEach(async () => {
    const testDB = await initTestDB();
    masterKey = testDB.masterKey;
    keyBytes = new Uint8Array(32);

    // Set up authenticated key manager
    keyManager.setMasterKey({
      key: masterKey,
      keyBytes,
      derivedAt: Date.now(),
    });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  describe('createBackup (v2.1 legacy)', () => {
    it('should create a backup with correct structure', async () => {
      // Create a test note
      await noteService.createNote('Test note content', ['tag1', 'tag2']);

      const backup = await createBackup();

      expect(backup.version).toBe('2.1');
      expect(backup.type).toBe('jottery-encrypted-backup');
      expect(backup.createdAt).toBeDefined();
      expect(backup.encryption).toBeDefined();
      expect(backup.encryption.salt).toBeDefined();
      expect(backup.encryption.iterations).toBeDefined();
      expect(isBackupV2(backup)).toBe(true);
      if (isBackupV2(backup)) {
        expect(Array.isArray(backup.data)).toBe(true);
        expect(backup.data.length).toBeGreaterThan(0);
      }
    });

    it('should call progress callback during backup creation', async () => {
      // Create multiple test notes
      await noteService.createNote('Note 1', ['tag1']);
      await noteService.createNote('Note 2', ['tag2']);
      await noteService.createNote('Note 3', ['tag3']);

      const progressCalls: Array<{
        phase: string;
        current?: number;
        total?: number;
        item?: string;
      }> = [];

      const onProgress: BackupProgressCallback = (progress) => {
        progressCalls.push({ ...progress });
      };

      await createBackup(onProgress);

      // Should have counting phase
      expect(progressCalls.some(p => p.phase === 'counting')).toBe(true);

      // Should have encrypting phases
      expect(progressCalls.some(p => p.phase === 'encrypting')).toBe(true);

      // Should have complete phase
      expect(progressCalls.some(p => p.phase === 'complete')).toBe(true);

      // Progress should increase
      const encryptingCalls = progressCalls.filter(p => p.phase === 'encrypting');
      expect(encryptingCalls.length).toBeGreaterThan(0);

      // Current should increase over time
      const currents = encryptingCalls.map(p => p.current).filter(c => c !== undefined);
      for (let i = 1; i < currents.length; i++) {
        expect(currents[i]).toBeGreaterThanOrEqual(currents[i - 1]!);
      }
    });

    it('should include all notes in backup', async () => {
      // Create multiple test notes
      await noteService.createNote('Note 1', ['tag1']);
      await noteService.createNote('Note 2', ['tag2']);

      const backup = await createBackup();

      // Each note becomes a JWE record, plus settings and potentially sync metadata
      // So we expect at least 2 records (notes) + 1 (settings) = 3
      expect(isBackupV2(backup)).toBe(true);
      if (isBackupV2(backup)) {
        expect(backup.data.length).toBeGreaterThanOrEqual(3);
      }
    });

    it('should throw error when app is locked', async () => {
      keyManager.clearMasterKey();

      await expect(createBackup()).rejects.toThrow('Application is locked');
    });
  });

  describe('downloadBackup', () => {
    it('should create downloadable blob from backup data', async () => {
      await noteService.createNote('Test note', ['test']);
      const backup = await createBackup();

      // Mock document methods
      const mockLink = {
        href: '',
        download: '',
        click: vi.fn(),
      };
      const createElementSpy = vi.spyOn(document, 'createElement').mockReturnValue(mockLink as unknown as HTMLAnchorElement);
      const appendChildSpy = vi.spyOn(document.body, 'appendChild').mockImplementation(() => mockLink as unknown as Node);
      const removeChildSpy = vi.spyOn(document.body, 'removeChild').mockImplementation(() => mockLink as unknown as Node);
      const createObjectURLSpy = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:test');
      const revokeObjectURLSpy = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});

      downloadBackup(backup);

      expect(createElementSpy).toHaveBeenCalledWith('a');
      expect(mockLink.click).toHaveBeenCalled();
      expect(mockLink.download).toMatch(/^jottery-backup-\d{4}-\d{2}-\d{2}\.jottery-backup$/);
      expect(createObjectURLSpy).toHaveBeenCalled();

      // revokeObjectURL is called after a 1 second delay to ensure download initiates
      // Wait for the delayed cleanup
      await new Promise(resolve => setTimeout(resolve, 1100));
      expect(revokeObjectURLSpy).toHaveBeenCalled();

      // Cleanup
      createElementSpy.mockRestore();
      appendChildSpy.mockRestore();
      removeChildSpy.mockRestore();
      createObjectURLSpy.mockRestore();
      revokeObjectURLSpy.mockRestore();
    });

    it('should use chunked JSON generation for large backups', async () => {
      // Create multiple notes to ensure chunking is used
      for (let i = 0; i < 10; i++) {
        await noteService.createNote(`Note ${i} with some content`, [`tag${i}`]);
      }

      const backup = await createBackup();

      // The downloadBackup function should handle this without throwing
      // We're mainly testing that it doesn't fail with large data
      const mockLink = {
        href: '',
        download: '',
        click: vi.fn(),
      };
      vi.spyOn(document, 'createElement').mockReturnValue(mockLink as unknown as HTMLAnchorElement);
      vi.spyOn(document.body, 'appendChild').mockImplementation(() => mockLink as unknown as Node);
      vi.spyOn(document.body, 'removeChild').mockImplementation(() => mockLink as unknown as Node);

      let blobContent: Blob | undefined;
      vi.spyOn(URL, 'createObjectURL').mockImplementation((obj: Blob | MediaSource) => {
        blobContent = obj as Blob;
        return 'blob:test';
      });
      vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});

      expect(() => downloadBackup(backup)).not.toThrow();
      expect(blobContent).toBeDefined();
      expect(blobContent!.type).toBe('application/json');

      vi.restoreAllMocks();
    });
  });

  describe('backup data integrity (v2.1)', () => {
    it('should produce valid JSON when reconstructed from chunks', async () => {
      await noteService.createNote('Test note', ['test']);
      const backup = await createBackup();

      expect(isBackupV2(backup)).toBe(true);
      if (!isBackupV2(backup)) return;

      // Simulate what downloadBackup does - build chunks
      const chunks: string[] = [];
      chunks.push('{"version":');
      chunks.push(JSON.stringify(backup.version));
      chunks.push(',"type":');
      chunks.push(JSON.stringify(backup.type));
      chunks.push(',"createdAt":');
      chunks.push(JSON.stringify(backup.createdAt));
      chunks.push(',"encryption":');
      chunks.push(JSON.stringify(backup.encryption));
      chunks.push(',"data":[');

      for (let i = 0; i < backup.data.length; i++) {
        if (i > 0) chunks.push(',');
        chunks.push(JSON.stringify(backup.data[i]));
      }

      chunks.push(']}');

      const reconstructed = chunks.join('');

      // Should be valid JSON
      const parsed = JSON.parse(reconstructed);
      expect(parsed.version).toBe(backup.version);
      expect(parsed.type).toBe(backup.type);
      expect(parsed.data.length).toBe(backup.data.length);
    });
  });

  describe('createBatchedBackup (v3.0)', () => {
    it('should create a batched backup with correct structure', async () => {
      // Create a test note
      await noteService.createNote('Test note content', ['tag1', 'tag2']);

      const backup = await createBatchedBackup();

      expect(backup.version).toBe('3.0');
      expect(backup.type).toBe('jottery-encrypted-backup');
      expect(backup.createdAt).toBeDefined();
      expect(backup.encryption).toBeDefined();
      expect(backup.encryption.salt).toBeDefined();
      expect(backup.encryption.iterations).toBeDefined();
      expect(isBackupV3(backup)).toBe(true);
      expect(Array.isArray(backup.batches)).toBe(true);
      expect(backup.summary).toBeDefined();
      expect(backup.summary.notes).toBeGreaterThan(0);
    });

    it('should call progress callback during batched backup creation', async () => {
      // Create multiple test notes
      await noteService.createNote('Note 1', ['tag1']);
      await noteService.createNote('Note 2', ['tag2']);
      await noteService.createNote('Note 3', ['tag3']);

      const progressCalls: Array<{
        phase: string;
        current?: number;
        total?: number;
        item?: string;
      }> = [];

      const onProgress: BackupProgressCallback = (progress) => {
        progressCalls.push({ ...progress });
      };

      await createBatchedBackup(onProgress);

      // Should have counting phase
      expect(progressCalls.some(p => p.phase === 'counting')).toBe(true);

      // Should have encrypting phases
      expect(progressCalls.some(p => p.phase === 'encrypting')).toBe(true);

      // Should have complete phase
      expect(progressCalls.some(p => p.phase === 'complete')).toBe(true);
    });

    it('should include summary with correct counts', async () => {
      // Create multiple test notes
      await noteService.createNote('Note 1', ['tag1']);
      await noteService.createNote('Note 2', ['tag2']);

      const backup = await createBatchedBackup();

      expect(backup.summary.notes).toBe(2);
      expect(backup.summary.attachments).toBe(0);
      expect(backup.summary.versions).toBeGreaterThanOrEqual(0);
      expect(backup.summary.savedSearches).toBeGreaterThanOrEqual(0);
    });

    it('should batch notes correctly', async () => {
      // Create a few notes (less than batch size)
      await noteService.createNote('Note 1', ['tag1']);
      await noteService.createNote('Note 2', ['tag2']);

      const backup = await createBatchedBackup();

      // With 2 notes, should have 1 notes batch
      const noteBatches = backup.batches.filter(b => b.type === 'notes');
      expect(noteBatches.length).toBe(1);
      expect(noteBatches[0].count).toBe(2);
      expect(noteBatches[0].index).toBe(0);
    });

    it('should throw error when app is locked', async () => {
      keyManager.clearMasterKey();

      await expect(createBatchedBackup()).rejects.toThrow('Application is locked');
    });
  });

  describe('downloadBackup (v3.0)', () => {
    it('should create downloadable blob from batched backup data', async () => {
      await noteService.createNote('Test note', ['test']);
      const backup = await createBatchedBackup();

      // Mock document methods
      const mockLink = {
        href: '',
        download: '',
        click: vi.fn(),
      };
      const createElementSpy = vi.spyOn(document, 'createElement').mockReturnValue(mockLink as unknown as HTMLAnchorElement);
      const appendChildSpy = vi.spyOn(document.body, 'appendChild').mockImplementation(() => mockLink as unknown as Node);
      const removeChildSpy = vi.spyOn(document.body, 'removeChild').mockImplementation(() => mockLink as unknown as Node);
      const createObjectURLSpy = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:test');
      const revokeObjectURLSpy = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});

      downloadBackup(backup);

      expect(createElementSpy).toHaveBeenCalledWith('a');
      expect(mockLink.click).toHaveBeenCalled();
      expect(mockLink.download).toMatch(/^jottery-backup-\d{4}-\d{2}-\d{2}\.jottery-backup$/);
      expect(createObjectURLSpy).toHaveBeenCalled();

      // revokeObjectURL is called after a 1 second delay to ensure download initiates
      // Wait for the delayed cleanup
      await new Promise(resolve => setTimeout(resolve, 1100));
      expect(revokeObjectURLSpy).toHaveBeenCalled();

      // Cleanup
      createElementSpy.mockRestore();
      appendChildSpy.mockRestore();
      removeChildSpy.mockRestore();
      createObjectURLSpy.mockRestore();
      revokeObjectURLSpy.mockRestore();
    });
  });

  describe('backup data integrity (v3.0)', () => {
    it('should produce valid JSON when reconstructed from chunks', async () => {
      await noteService.createNote('Test note', ['test']);
      const backup = await createBatchedBackup();

      // Simulate what downloadBackup does - build chunks
      const chunks: string[] = [];
      chunks.push('{"version":');
      chunks.push(JSON.stringify(backup.version));
      chunks.push(',"type":');
      chunks.push(JSON.stringify(backup.type));
      chunks.push(',"createdAt":');
      chunks.push(JSON.stringify(backup.createdAt));
      chunks.push(',"encryption":');
      chunks.push(JSON.stringify(backup.encryption));
      chunks.push(',"batches":[');

      for (let i = 0; i < backup.batches.length; i++) {
        if (i > 0) chunks.push(',');
        chunks.push(JSON.stringify(backup.batches[i]));
      }

      chunks.push('],"summary":');
      chunks.push(JSON.stringify(backup.summary));
      chunks.push('}');

      const reconstructed = chunks.join('');

      // Should be valid JSON
      const parsed = JSON.parse(reconstructed);
      expect(parsed.version).toBe(backup.version);
      expect(parsed.type).toBe(backup.type);
      expect(parsed.batches.length).toBe(backup.batches.length);
      expect(parsed.summary.notes).toBe(backup.summary.notes);
    });
  });
});
