/**
 * Any client has to be able to restore any other client's backup. The fixture is a
 * real file written by the iOS app's BackupService, so this fails if the web and iOS
 * forms drift apart again. Its password and contents are test data.
 */
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import { createBatchedBackup, getBackupStats, validateBackup, verifyBackupContents, verifyBackupPassword, type BackupData } from './backupService';
import { initialize, restoreFromBackup } from './initService';
import { settingsRepository } from './settingsRepository';
import { attachmentService } from './attachmentService';
import { noteService } from './noteService';
import { savedSearchRepository } from './savedSearchRepository';
import { versionRepository } from './versionRepository';
import { keyManager } from './keyManager';
import { closeDB, deleteDB, initDB } from './db';
import type { UserSettings } from '../types';

const PASSWORD = 'notes-password-correct-horse';
const fixture = fs.readFileSync(path.resolve(__dirname, '../../test/fixtures/ios-v3.jottery-backup'), 'utf-8');

function iosFile(name = 'ios-v3.jottery-backup'): File {
  return new File([fixture], name);
}

describe('a backup written by the iOS app', () => {
  let backup: BackupData;

  beforeEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
    await initDB();

    const result = await validateBackup(iosFile());
    expect(result.error).toBeUndefined();
    backup = result.backup as BackupData;
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
  });

  it('reads its header and summary', async () => {
    const stats = await getBackupStats(iosFile());
    expect(stats.valid).toBe(true);
    expect(stats.version).toBe('3.0');
    expect(stats.summary).toEqual({ notes: 2, attachments: 1, versions: 1, savedSearches: 1 });
  });

  it('accepts its password and no other', async () => {
    expect((await verifyBackupPassword(backup, PASSWORD)).valid).toBe(true);
    expect((await verifyBackupPassword(backup, 'not-the-password')).valid).toBe(false);
  }, 60_000);

  it('passes the full check a restore makes first', async () => {
    const { keyBytes } = await verifyBackupPassword(backup, PASSWORD);
    await expect(verifyBackupContents(backup, keyBytes as Uint8Array)).resolves.toBeUndefined();
  }, 60_000);

  describe('once restored', () => {
    beforeEach(async () => {
      await restoreFromBackup(backup, PASSWORD);
    }, 60_000);

    const noteWith = async (text: string) => {
      const found = (await noteService.getAllNotes()).find(note => note.content.includes(text));
      expect(found).toBeDefined();
      return found!;
    };

    it('has its notes, with their text and tags', async () => {
      const note = await noteWith('From iOS');
      expect(note.content).toBe('From iOS\nwith a file');
      expect(note.tags).toEqual(['ios', 'two tags']);
      expect(note.pinned).toBe(true);
      expect(note.contentHash).toBeTruthy();
    });

    it('copes with a note that has no tags', async () => {
      expect((await noteWith('no tags at all')).tags).toEqual([]);
    });

    it('can open its attachment', async () => {
      const [attachment] = (await noteWith('From iOS')).attachments;
      const blob = await attachmentService.getAttachmentData(attachment);
      expect(blob.size).toBeGreaterThan(0);
      expect(await attachmentService.getDecryptedFilename(attachment)).toBeTruthy();
    });

    it('has its versions', async () => {
      const note = await noteWith('From iOS');
      expect(await versionRepository.getVersionsForNote(note.id)).toHaveLength(1);
    });

    it('can read its saved search', async () => {
      const [search] = await savedSearchRepository.getAll();
      expect(search.name).toBe('iOS search');
      expect(search.query).toBe('#ios');
    });
  });
});

describe('the TUI settings field', () => {
  beforeEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
    await initDB();
    await initialize(PASSWORD);
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
  });

  it('survives a restore and another backup, as the format document says', async () => {
    const tuiSettings = { language: 'en-GB', theme: 'dark', tuiTheme: 'solarized-dark', sortOrder: 'recent', autoLockTimeout: 15 };
    await settingsRepository.update(tuiSettings as Partial<UserSettings>);

    const backup = await createBatchedBackup();
    const { keyBytes } = await verifyBackupPassword(backup, PASSWORD);
    await verifyBackupContents(backup, keyBytes as Uint8Array);
    await restoreFromBackup(backup, PASSWORD);

    const settings = await settingsRepository.get() as UserSettings & { tuiTheme?: string };
    expect(settings.tuiTheme).toBe('solarized-dark');
    expect(settings.theme).toBe('dark');
  }, 60_000);
});

describe('the backup file name', () => {
  it.each([['.jottery-backup', 'vault.jottery-backup'], ['.json', 'vault.json']])('accepts %s', async (_kind, name) => {
    expect((await validateBackup(iosFile(name))).valid).toBe(true);
  });

  it('refuses anything else, as the format document says', async () => {
    expect((await validateBackup(iosFile('vault.jottery'))).valid).toBe(false);
  });
});
