/**
 * Any client has to be able to restore any other client's backup. Each fixture is a
 * real file written by that client, so this fails if its form and the web's drift
 * apart again. A fixture's password sits beside it in a .password file, and its
 * contents are test data.
 */
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import { createBatchedBackup, getBackupStats, validateBackup, verifyBackupContents, verifyBackupPassword, type BackupData } from './backupService';
import { initialize, restoreFromBackup, unlock } from './initService';
import { settingsRepository } from './settingsRepository';
import { attachmentService } from './attachmentService';
import { noteService } from './noteService';
import { savedSearchRepository } from './savedSearchRepository';
import { versionRepository } from './versionRepository';
import { keyManager } from './keyManager';
import { closeDB, deleteDB, initDB } from './db';
import type { UserSettings } from '../types';

const fixtures = path.resolve(__dirname, '../../test/fixtures');
const readFixture = (name: string) => fs.readFileSync(path.join(fixtures, name), 'utf-8');

const PASSWORD = readFixture('ios-v3.jottery-backup.password');
const WRONG_PASSWORD = crypto.randomUUID();
const fixture = readFixture('ios-v3.jottery-backup');

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
    expect((await verifyBackupPassword(backup, WRONG_PASSWORD)).valid).toBe(false);
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

/** What each client put in its fixture. The Android pair are the same notes, in the two vault forms. */
interface Expected {
  file: string;
  summary: { notes: number; attachments: number; versions: number; savedSearches: number };
  note: { text: string; content: string; tags: string[]; pinned: boolean };
  plain: { text: string; tags: string[] } | null;
  archived: string | null;
  attachment: { name: string; text: string };
  search: { name: string; query: string } | null;
}

const fromAndroid = (file: string): Expected => ({
  file,
  summary: { notes: 2, attachments: 1, versions: 1, savedSearches: 1 },
  note: { text: 'From Android', content: 'From Android, edited once', tags: ['android', 'fixture'], pinned: true },
  plain: { text: 'no tags', tags: [] },
  archived: null,
  attachment: { name: 'hello.txt', text: 'attachment bytes' },
  search: { name: 'Android search', query: '#android' },
});

const others: [string, Expected][] = [
  ['the TUI', {
    file: 'tui-v3.jottery-backup',
    summary: { notes: 2, attachments: 1, versions: 0, savedSearches: 0 },
    note: { text: 'from the TUI', content: 'A note from the TUI, with a café and 日本語\n', tags: ['work', 'cross-client'], pinned: false },
    plain: null,
    archived: 'An archived note\n',
    attachment: { name: 'hello.txt', text: 'attachment body' },
    search: null,
  }],
  ['Android, from a legacy vault', fromAndroid('android-v3-legacy.jottery-backup')],
  ['Android, from an envelope vault', fromAndroid('android-v3-envelope.jottery-backup')],
];

describe.each(others)('a backup written by %s', (_client, expected) => {
  const password = readFixture(`${expected.file}.password`);
  const file = () => new File([readFixture(expected.file)], expected.file);
  let backup: BackupData;

  beforeEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
    await initDB();

    const result = await validateBackup(file());
    expect(result.error).toBeUndefined();
    backup = result.backup as BackupData;
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
  });

  const expectEverythingRestored = async () => {
    const notes = await noteService.getAllNotes();
    const note = notes.find(n => n.content.includes(expected.note.text));
    expect(note).toBeDefined();
    expect(note!.content).toBe(expected.note.content);
    expect(note!.tags).toEqual(expected.note.tags);
    expect(note!.pinned).toBe(expected.note.pinned);
    expect(note!.contentHash).toBeTruthy();

    if (expected.plain) {
      const { text, tags } = expected.plain;
      expect(notes.find(n => n.content.includes(text))!.tags).toEqual(tags);
    }

    const archived = await noteService.getArchivedNotes();
    expect(archived.map(n => n.content)).toEqual(expected.archived ? [expected.archived] : []);
    expect(notes.length + archived.length).toBe(expected.summary.notes);

    const [attachment] = note!.attachments;
    expect(await (await attachmentService.getAttachmentData(attachment)).text()).toBe(expected.attachment.text);
    expect(await attachmentService.getDecryptedFilename(attachment)).toBe(expected.attachment.name);

    expect(await versionRepository.getVersionsForNote(note!.id)).toHaveLength(expected.summary.versions);

    const searches = await savedSearchRepository.getAll();
    expect(searches.map(({ name, query }) => ({ name, query }))).toEqual(expected.search ? [expected.search] : []);
  };

  it('reads its header and summary', async () => {
    const stats = await getBackupStats(file());
    expect(stats.valid).toBe(true);
    expect(stats.version).toBe('3.0');
    expect(stats.summary).toEqual(expected.summary);
  });

  it('accepts its password and no other', async () => {
    expect((await verifyBackupPassword(backup, password)).valid).toBe(true);
    expect((await verifyBackupPassword(backup, WRONG_PASSWORD)).valid).toBe(false);
  }, 60_000);

  it('passes the full check a restore makes first', async () => {
    const { keyBytes } = await verifyBackupPassword(backup, password);
    await expect(verifyBackupContents(backup, keyBytes as Uint8Array)).resolves.toBeUndefined();
  }, 60_000);

  it('restores completely', async () => {
    await restoreFromBackup(backup, password);
    await expectEverythingRestored();
  }, 60_000);

  it('still opens after locking, which is the first thing that happens to it', async () => {
    await restoreFromBackup(backup, password);
    keyManager.clearMasterKey();

    await expect(unlock(WRONG_PASSWORD)).rejects.toThrow();
    keyManager.clearMasterKey();
    await unlock(password);

    await expectEverythingRestored();
  }, 60_000);

  it('restores the same way through the reader for large files', async () => {
    // Anything after the JSON stops the fast path, which runs the streaming parser
    const streamed = await validateBackup(new File([readFixture(expected.file), '\n#'], expected.file));
    expect(streamed.error).toBeUndefined();

    await restoreFromBackup(streamed.backup as BackupData, password);
    await expectEverythingRestored();
  }, 60_000);

  it('can be backed up again and restored here', async () => {
    await restoreFromBackup(backup, password);

    const again = await createBatchedBackup();
    await restoreFromBackup(again, password);

    await expectEverythingRestored();
  }, 60_000);
});

describe('the TUI settings field', () => {
  const password = crypto.randomUUID();

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

  it('survives a restore and another backup, as the format document says', async () => {
    const tuiSettings = { language: 'en-GB', theme: 'dark', tuiTheme: 'solarized-dark', sortOrder: 'recent', autoLockTimeout: 15 };
    await settingsRepository.update(tuiSettings as Partial<UserSettings>);

    const backup = await createBatchedBackup();
    const { keyBytes } = await verifyBackupPassword(backup, password);
    await verifyBackupContents(backup, keyBytes as Uint8Array);
    await restoreFromBackup(backup, password);

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
