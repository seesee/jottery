/**
 * A restore replaces the vault, so a backup has to be known good before anything is
 * deleted, and a restore that didn't complete must never look as though it did.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  createBackup,
  createBatchedBackup,
  getBackupStats,
  validateBackup,
  verifyBackupContents,
  verifyBackupPassword,
  type BackupDataV2,
  type BackupDataV3,
} from './backupService';
import { initialize, restoreFromBackup } from './initService';
import { noteService } from './noteService';
import { noteRepository } from './noteRepository';
import { settingsRepository } from './settingsRepository';
import { attachmentRepository } from './attachmentRepository';
import { keyManager } from './keyManager';
import { closeDB, deleteDB, initDB } from './db';
import { BackupStorageFullError } from '../errors';
import { DEFAULT_SETTINGS } from '../types';

const PASSWORD = 'notes-password-correct-horse';
const TYPE = 'jottery-encrypted-backup';
const ENCRYPTION = { salt: 'c2FsdA==', iterations: 1000, algorithm: 'AES-256-GCM', createdAt: '2026-01-01T00:00:00Z' };

/** A string with the shape of a JWE compact token, of roughly `size` characters. */
function token(index: number, size = 200): string {
  const body = 'A'.repeat(Math.max(1, size - 60));
  return `eyJhbGciOiJkaXIifQ..iv${index}.${body}.tag${index}`;
}

function v2File(tokens: string[], { forceStreaming = true, suffix = '' } = {}): File {
  const backup = { version: '2.1', type: TYPE, createdAt: '2026-01-01T00:00:00Z', encryption: ENCRYPTION, data: tokens };
  // Anything after the JSON stops the fast path, which runs the streaming parser
  // on a file much smaller than the 100 MB it would otherwise take
  const text = JSON.stringify(backup) + suffix + (forceStreaming ? 'x' : '');
  return new File([text], 'vault.jottery-backup', { type: 'application/json' });
}

async function parsedV2(file: File): Promise<string[]> {
  const result = await validateBackup(file);
  expect(result.error).toBeUndefined();
  expect(result.valid).toBe(true);
  return (result.backup as BackupDataV2).data;
}

describe('reading a large v2.1 backup', () => {
  it('keeps records that cross a chunk boundary', async () => {
    const tokens = Array.from({ length: 12 }, (_, i) => token(i, 2 * 1024 * 1024));
    expect(await parsedV2(v2File(tokens))).toEqual(tokens);
  }, 30_000);

  it('keeps a record larger than a whole chunk', async () => {
    const tokens = [token(0), token(1, 11 * 1024 * 1024), token(2)];
    expect(await parsedV2(v2File(tokens))).toEqual(tokens);
  }, 30_000);

  it('never counts a small record twice', async () => {
    const tokens = Array.from({ length: 12_000 }, (_, i) => token(i, 1000));
    const parsed = await parsedV2(v2File(tokens));
    expect(parsed).toHaveLength(tokens.length);
    expect(new Set(parsed).size).toBe(tokens.length);
  }, 30_000);

  it('keeps a small file with a few records', async () => {
    const tokens = [token(0), token(1)];
    expect(await parsedV2(v2File(tokens))).toEqual(tokens);
  });

  it('refuses a truncated file rather than returning part of it', async () => {
    const whole = JSON.stringify({ version: '2.1', type: TYPE, createdAt: 'x', encryption: ENCRYPTION, data: [token(0), token(1)] });
    const cut = whole.slice(0, whole.length - 40);
    const result = await validateBackup(new File([cut], 'vault.jottery-backup'));
    expect(result.valid).toBe(false);
  });

  it('refuses a record that is not a JWE token, rather than dropping it', async () => {
    const result = await validateBackup(v2File([token(0), 'not-a-token', token(1)]));
    expect(result.valid).toBe(false);
  });
});

describe('getBackupStats', () => {
  it('counts every v2.1 record, including one that straddles a read', async () => {
    // Size the first token so the second starts just before the 1 MB read boundary
    const MEGABYTE = 1024 * 1024;
    const prefix = JSON.stringify({ version: '2.1', type: TYPE, createdAt: '2026-01-01T00:00:00Z', encryption: ENCRYPTION, data: [''] }).indexOf('""');
    const fixedParts = token(0, 61).length - 1;
    const bodyLength = MEGABYTE - 5 - prefix - 3 - fixedParts;
    const first = `eyJhbGciOiJkaXIifQ..iv0.${'A'.repeat(bodyLength)}.tag0`;
    const file = v2File([first, token(1)], { forceStreaming: false });

    const text = await file.text();
    const secondStart = text.indexOf('"eyJ', prefix + 1);
    expect(secondStart).toBeLessThan(MEGABYTE);
    expect(secondStart + 22).toBeGreaterThan(MEGABYTE);

    expect((await getBackupStats(file)).recordCount).toBe(2);
  });
});

describe('checking a backup before restoring', () => {
  let keyBytes: Uint8Array;

  beforeEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
    await initDB();
    await initialize(PASSWORD);
    await noteService.createNote('first note', ['tag']);
    await noteService.createNote('second note');
    keyBytes = keyManager.getMasterKey()!.keyBytes;
  });

  afterEach(async () => {
    vi.restoreAllMocks();
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
  });

  const noteCount = async () => (await noteRepository.getAll()).length;

  it('accepts a good backup', async () => {
    await expect(verifyBackupContents(await createBatchedBackup(), keyBytes)).resolves.toBeUndefined();
  });

  it('refuses a damaged batch that is not the first', async () => {
    const backup = await createBatchedBackup();
    const last = backup.batches[backup.batches.length - 1];
    last.data = backup.batches[0].data.replace(/\.[^.]+$/, '.AAAA');
    await expect(verifyBackupContents(backup, keyBytes)).rejects.toThrow();
  });

  it('refuses a batch holding fewer items than it says', async () => {
    const backup = await createBatchedBackup();
    backup.batches[0].count += 1;
    await expect(verifyBackupContents(backup, keyBytes)).rejects.toThrow(/count/i);
  });

  it('refuses a backup that disagrees with its summary', async () => {
    const backup = await createBatchedBackup();
    backup.summary.notes += 1;
    await expect(verifyBackupContents(backup, keyBytes)).rejects.toThrow(/summary/i);
  });

  it('refuses a backup missing a batch of notes', async () => {
    const backup = await createBatchedBackup();
    backup.batches = backup.batches.filter(batch => batch.type !== 'notes');
    await expect(verifyBackupContents(backup, keyBytes)).rejects.toThrow(/summary/i);
  });

  describe('restoreFromBackup', () => {
    it('leaves the vault alone when a later batch is damaged', async () => {
      const backup = await createBatchedBackup();
      backup.batches[backup.batches.length - 1].data = backup.batches[0].data.replace(/\.[^.]+$/, '.AAAA');

      // Only in the vault, so a restore that got anywhere would lose it
      await noteService.createNote('made after the backup');

      await expect(restoreFromBackup(backup, PASSWORD)).rejects.toThrow();

      expect(await noteCount()).toBe(3);
    }, 60_000);

    it('leaves the vault alone when the summary is wrong', async () => {
      const backup = await createBatchedBackup();
      backup.summary.notes = 99;

      await expect(restoreFromBackup(backup, PASSWORD)).rejects.toThrow();

      expect(await noteCount()).toBe(2);
    }, 60_000);

    it.each([
      ['v3', () => createBatchedBackup()],
      ['v2.1', () => createBackup()],
    ])('leaves per-device security settings behind, in a %s backup', async (_version, create) => {
      await settingsRepository.update({ rememberPassword: true, persistSession: true, persistSessionTimeout: 999, theme: 'dark' });
      const backup = await create();

      await restoreFromBackup(backup, PASSWORD);

      const restored = await settingsRepository.get();
      expect(restored.rememberPassword).toBe(false);
      expect(restored.persistSession).toBe(false);
      expect(restored.persistSessionTimeout).toBe(DEFAULT_SETTINGS.persistSessionTimeout);
      expect(restored.theme).toBe('dark');
    }, 60_000);

    it('still restores a good backup', async () => {
      const backup = await createBatchedBackup();
      await noteService.createNote('made after the backup');
      expect(await noteCount()).toBe(3);

      await restoreFromBackup(backup, PASSWORD);

      expect(await noteCount()).toBe(2);
    }, 60_000);
  });

  describe('a backup with nothing in it', () => {
    const legacyEmptyV3 = (): BackupDataV3 => ({
      version: '3.0', type: TYPE as never, createdAt: 'x', encryption: ENCRYPTION as never, batches: [],
      summary: { notes: 0, attachments: 0, versions: 0, savedSearches: 0 },
    });
    const legacyEmptyV2 = (): BackupDataV2 => ({
      version: '2.1', type: TYPE as never, createdAt: 'x', encryption: ENCRYPTION as never, data: [],
    });

    it('never passes the password check, since nothing proves the password', async () => {
      expect((await verifyBackupPassword(legacyEmptyV3(), 'anything')).valid).toBe(false);
      expect((await verifyBackupPassword(legacyEmptyV2(), 'anything')).valid).toBe(false);
    }, 60_000);

    it('cannot wipe the vault', async () => {
      await expect(restoreFromBackup(legacyEmptyV3(), 'anything')).rejects.toThrow();
      expect(await noteCount()).toBe(2);

      await expect(restoreFromBackup(legacyEmptyV2(), 'anything')).rejects.toThrow();
      expect(await noteCount()).toBe(2);
    }, 60_000);
  });

  describe('when the browser runs out of storage during a restore', () => {
    function exhaustStorage() {
      vi.spyOn(attachmentRepository, 'storeBlob').mockRejectedValue(
        Object.assign(new Error('quota'), { name: 'QuotaExceededError' })
      );
    }

    it('reports failure instead of a partial success', async () => {
      const blobId = 'b0000000-0000-4000-8000-000000000001';
      const note = (await noteRepository.getAll())[0];
      await noteRepository.update({
        ...note,
        attachments: [{ id: blobId, filename: '{}', mimeType: 'image/png', size: 1, data: blobId }],
      }, false, true);
      await attachmentRepository.storeBlob(blobId, new ArrayBuffer(4));

      const backup = await createBatchedBackup();
      exhaustStorage();

      await expect(restoreFromBackup(backup, PASSWORD)).rejects.toBeInstanceOf(BackupStorageFullError);
    }, 60_000);

    it('does not leave a partial vault behind to be mistaken for the real one', async () => {
      const blobId = 'b0000000-0000-4000-8000-000000000002';
      const note = (await noteRepository.getAll())[0];
      await noteRepository.update({
        ...note,
        attachments: [{ id: blobId, filename: '{}', mimeType: 'image/png', size: 1, data: blobId }],
      }, false, true);
      await attachmentRepository.storeBlob(blobId, new ArrayBuffer(4));

      const backup = await createBatchedBackup();
      exhaustStorage();

      await expect(restoreFromBackup(backup, PASSWORD)).rejects.toThrow();

      expect(await noteCount()).toBe(0);
    }, 60_000);
  });
});
