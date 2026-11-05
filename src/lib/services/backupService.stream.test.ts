/**
 * A vault can hold a gigabyte of attachments, so a backup can't hold them all at once.
 * The file has to come out the same as ever, because every client reads it.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import pako from 'pako';
import { createBackupFile, createBatchedBackup, validateBackup, verifyBackupContents, verifyBackupPassword, type BackupDataV3 } from './backupService';
import { initialize, restoreFromBackup } from './initService';
import { attachmentRepository } from './attachmentRepository';
import { noteRepository } from './noteRepository';
import { noteService } from './noteService';
import { keyManager } from './keyManager';
import { closeDB, deleteDB, initDB } from './db';
import { createTestNote } from '../../test/db-utils';

const password = crypto.randomUUID();
const MB = 1024 * 1024;

describe('writing a backup', () => {
  beforeEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
    await initDB();
    await initialize(password);
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
  });

  /** A note with `count` attachments of `bytes` random bytes, so that none compress away */
  async function noteWithAttachments(count: number, bytes: number): Promise<string[]> {
    const ids = Array.from({ length: count }, () => crypto.randomUUID());
    for (const id of ids) {
      const data = new Uint8Array(bytes);
      for (let offset = 0; offset < bytes; offset += 65536) {
        crypto.getRandomValues(data.subarray(offset, Math.min(offset + 65536, bytes)));
      }
      await attachmentRepository.storeBlob(id, data.buffer);
    }
    await noteRepository.create(createTestNote({
      tags: [],
      attachments: ids.map(id => ({ id, filename: '{}', mimeType: 'application/octet-stream', size: bytes, data: id })),
    }));
    return ids;
  }

  async function parsed(blob: Blob): Promise<BackupDataV3> {
    return JSON.parse(await blob.text()) as BackupDataV3;
  }

  it('encrypts attachments as it reads them, not after reading them all', async () => {
    await noteWithAttachments(60, 16);
    const order: string[] = [];
    const getBlob = attachmentRepository.getBlob.bind(attachmentRepository);
    vi.spyOn(attachmentRepository, 'getBlob').mockImplementation(id => {
      order.push('read');
      return getBlob(id);
    });
    const deflate = pako.deflate;
    vi.spyOn(pako, 'deflate').mockImplementation(((data: Uint8Array) => {
      order.push('encrypt');
      return deflate(data);
    }) as typeof pako.deflate);

    await createBackupFile();

    // The notes are encrypted before any attachment is read, so they don't count
    expect(order.slice(order.indexOf('read'), order.lastIndexOf('read'))).toContain('encrypt');
  });

  it('keeps a batch of large attachments to a sensible size', async () => {
    await noteWithAttachments(3, 7 * MB);

    const { batches, summary } = await parsed(await createBackupFile());

    const attachmentBatches = batches.filter(batch => batch.type === 'attachments');
    expect(attachmentBatches.length).toBeGreaterThan(1);
    expect(attachmentBatches.map(batch => batch.index)).toEqual(attachmentBatches.map((_, index) => index));
    expect(attachmentBatches.reduce((sum, batch) => sum + batch.count, 0)).toBe(3);
    expect(summary.attachments).toBe(3);
  }, 60_000);

  it('writes the same file a backup object would', async () => {
    await noteWithAttachments(3, 64);

    const file = await parsed(await createBackupFile());
    const backup = await createBatchedBackup();

    expect(Object.keys(file)).toEqual(['version', 'type', 'createdAt', 'encryption', 'batches', 'summary']);
    expect(file.summary).toEqual(backup.summary);
    expect(file.encryption).toEqual(backup.encryption);
    expect(file.batches.map(({ type, index, count }) => ({ type, index, count })))
      .toEqual(backup.batches.map(({ type, index, count }) => ({ type, index, count })));
  });

  it('can be restored, through both readers', async () => {
    const note = await noteService.createNote('Round trip', ['one']);
    const [id] = await noteWithAttachments(1, 64);
    const file = await createBackupFile();

    for (const parts of [[file], [file, '\n#']]) {
      const result = await validateBackup(new File(parts, 'vault.jottery-backup'));
      expect(result.error).toBeUndefined();

      const backup = result.backup as BackupDataV3;
      const { keyBytes } = await verifyBackupPassword(backup, password);
      await verifyBackupContents(backup, keyBytes as Uint8Array);
      await restoreFromBackup(backup, password);

      expect((await noteService.getNote(note.id))?.content).toBe('Round trip');
      expect((await attachmentRepository.getBlob(id))?.byteLength).toBe(64);
    }
  }, 60_000);

  it('leaves out a blob that has gone, and says so in the summary', async () => {
    const [gone] = await noteWithAttachments(2, 64);
    await attachmentRepository.deleteBlob(gone);

    const { summary } = await parsed(await createBackupFile());

    expect(summary.attachments).toBe(1);
  });

  it('refuses to run while locked', async () => {
    keyManager.clearMasterKey();

    await expect(createBackupFile()).rejects.toThrow('locked');
  });
});
