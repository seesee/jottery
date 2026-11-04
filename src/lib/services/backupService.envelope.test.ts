/**
 * Backups of envelope vaults.
 *
 * Notes are encrypted with a random master key, which isn't derivable from the
 * password. A backup carries the master key wrapped with the password, as the
 * vault stores it, so it can be restored on a device that has nothing else.
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBackup, createBatchedBackup, getBackupStats, validateBackup, verifyBackupPassword } from './backupService';
import { initialize, lock, restoreFromBackup, unlock } from './initService';
import { noteService } from './noteService';
import { keyManager } from './keyManager';
import { closeDB, deleteDB, initDB } from './db';
import type { BackupData } from './backupService';

const PASSWORD = 'notes-password-correct-horse';

function asFile(backup: BackupData): File {
  return new File([JSON.stringify(backup)], 'vault.jottery-backup', { type: 'application/json' });
}

/** A vault holding one note, and a backup of it. */
async function backedUpVault(create: () => Promise<BackupData>): Promise<{ backup: BackupData; masterKey: Uint8Array }> {
  await initialize(PASSWORD);
  await noteService.createNote('kept safe in a backup', ['backed-up']);
  const masterKey = new Uint8Array(keyManager.getMasterKey()!.keyBytes);
  return { backup: await create(), masterKey };
}

/** Start again with nothing, as on a new device. */
async function wipe(): Promise<void> {
  keyManager.clearMasterKey();
  closeDB();
  await deleteDB();
  await initDB();
}

describe('envelope vault backups', () => {
  beforeEach(async () => {
    await initDB();
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
  });

  const formats: Array<[string, () => Promise<BackupData>]> = [
    ['batched (v3)', createBatchedBackup],
    ['legacy (v2)', createBackup],
  ];

  describe.each(formats)('%s', (_name, create) => {
    it('passes validation', async () => {
      const { backup } = await backedUpVault(create);
      const result = await validateBackup(asFile(backup));
      expect(result.error).toBeUndefined();
      expect(result.valid).toBe(true);
    });

    it('has a readable header, for the stats shown before restoring', async () => {
      const { backup } = await backedUpVault(create);
      const stats = await getBackupStats(asFile(backup));
      expect(stats.error).toBeUndefined();
      expect(stats.valid).toBe(true);
    });

    it('accepts the right password and yields the master key', async () => {
      const { backup, masterKey } = await backedUpVault(create);
      const result = await verifyBackupPassword(backup, PASSWORD);
      expect(result.error).toBeUndefined();
      expect(result.valid).toBe(true);
      expect(result.keyBytes).toEqual(masterKey);
    });

    it('refuses the wrong password', async () => {
      const { backup } = await backedUpVault(create);
      const result = await verifyBackupPassword(backup, 'not-the-password');
      expect(result.valid).toBe(false);
      expect(result.error).toBe('Incorrect password');
    });

    it('restores on an empty device, and the vault then unlocks normally', async () => {
      const { backup } = await backedUpVault(create);
      await wipe();

      await restoreFromBackup(backup, PASSWORD);
      expect((await noteService.getAllNotes()).map((note) => note.content)).toEqual(['kept safe in a backup']);

      lock();
      await unlock(PASSWORD);
      const [note] = await noteService.getAllNotes();
      expect(note.content).toBe('kept safe in a backup');
      expect(note.tags).toEqual(['backed-up']);
    });
  });

  it('refuses a backup with neither envelope nor legacy key material', async () => {
    const { backup } = await backedUpVault(createBatchedBackup);
    const broken = { ...backup, encryption: { algorithm: 'AES-256-GCM', createdAt: backup.createdAt } } as BackupData;
    const result = await validateBackup(asFile(broken));
    expect(result.valid).toBe(false);
    expect(result.error).toContain('Incomplete encryption metadata');
  });
});
