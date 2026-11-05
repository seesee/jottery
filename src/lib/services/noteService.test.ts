/**
 * Tests for noteService
 * Focuses on UI state persistence and sync-related behaviour
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { noteService } from './noteService';
import { noteRepository } from './noteRepository';
import { keyManager } from './keyManager';
import { attachmentRepository } from './attachmentRepository';
import { cryptoService, MAX_HASH_CHAIN_LENGTH } from './crypto';
import { NoteReadOnlyError } from '../errors';
import { undecryptableNotes, resetVaultHealth } from './vaultHealthService';
import type { Note } from '../types';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';

describe('noteService', () => {
  let masterKey: CryptoKey;

  beforeEach(async () => {
    const testDB = await initTestDB();
    masterKey = testDB.masterKey;

    // Set up authenticated key manager
    keyManager.setMasterKey({
      key: masterKey,
      keyBytes: new Uint8Array(32),
      derivedAt: Date.now(),
    });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  describe('updateNote', () => {
    describe('UI state changes should update modifiedAt for sync', () => {
      it('should update modifiedAt when showPreview changes', async () => {
        // Create a note
        const note = await noteService.createNote('Test content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        // Wait a bit to ensure time difference
        await new Promise(resolve => setTimeout(resolve, 10));

        // Update showPreview
        const updated = await noteService.updateNote(note.id, {
          content: 'Test content', // Same content
          tags: ['test'], // Same tags
          showPreview: true, // Changed
        });

        // modifiedAt should be updated
        expect(updated.modifiedAt).not.toBe(originalModifiedAt);
        expect(new Date(updated.modifiedAt).getTime()).toBeGreaterThan(
          new Date(originalModifiedAt).getTime()
        );

        // showPreview should be saved
        expect(updated.showPreview).toBe(true);
      });

      it('should update modifiedAt when wordWrap changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        const updated = await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          wordWrap: false, // Changed from default true
        });

        expect(updated.modifiedAt).not.toBe(originalModifiedAt);
        expect(updated.wordWrap).toBe(false);
      });

      it('should update modifiedAt when syntaxLanguage changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        const updated = await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          syntaxLanguage: 'javascript', // Changed from default markdown
        });

        expect(updated.modifiedAt).not.toBe(originalModifiedAt);
        expect(updated.syntaxLanguage).toBe('javascript');
      });

      it('should NOT update modifiedAt when UI state is unchanged', async () => {
        const note = await noteService.createNote('Test content', ['test'], {
          showPreview: true,
          wordWrap: false,
          syntaxLanguage: 'javascript',
        });
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        // Update with same values
        const updated = await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          showPreview: true, // Same
          wordWrap: false, // Same
          syntaxLanguage: 'javascript', // Same
        });

        // modifiedAt should NOT be updated since nothing changed
        expect(updated.modifiedAt).toBe(originalModifiedAt);
      });
    });

    describe('needsSync flag', () => {
      it('should set needsSync when showPreview changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);

        // Clear needsSync by simulating a sync
        const rawNote = await noteRepository.getById(note.id);
        if (rawNote) {
          rawNote.needsSync = false;
          await noteRepository.update(rawNote, false, true);
        }

        // Update showPreview
        await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          showPreview: true,
        });

        // Check needsSync is set
        const updatedNote = await noteRepository.getById(note.id);
        expect(updatedNote?.needsSync).toBe(true);
      });

      it('should set needsSync when wordWrap changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);

        const rawNote = await noteRepository.getById(note.id);
        if (rawNote) {
          rawNote.needsSync = false;
          await noteRepository.update(rawNote, false, true);
        }

        await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          wordWrap: false,
        });

        const updatedNote = await noteRepository.getById(note.id);
        expect(updatedNote?.needsSync).toBe(true);
      });

      it('should set needsSync when syntaxLanguage changes', async () => {
        const note = await noteService.createNote('Test content', ['test']);

        const rawNote = await noteRepository.getById(note.id);
        if (rawNote) {
          rawNote.needsSync = false;
          await noteRepository.update(rawNote, false, true);
        }

        await noteService.updateNote(note.id, {
          content: 'Test content',
          tags: ['test'],
          syntaxLanguage: 'python',
        });

        const updatedNote = await noteRepository.getById(note.id);
        expect(updatedNote?.needsSync).toBe(true);
      });
    });

    describe('content changes', () => {
      it('should update modifiedAt when content changes', async () => {
        const note = await noteService.createNote('Original content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        const updated = await noteService.updateNote(note.id, {
          content: 'Updated content',
          tags: ['test'],
        });

        expect(updated.modifiedAt).not.toBe(originalModifiedAt);
      });

      it('should NOT update modifiedAt when content is unchanged', async () => {
        const note = await noteService.createNote('Same content', ['test']);
        const originalModifiedAt = note.modifiedAt;

        await new Promise(resolve => setTimeout(resolve, 10));

        const updated = await noteService.updateNote(note.id, {
          content: 'Same content', // Same
          tags: ['test'], // Same
        });

        expect(updated.modifiedAt).toBe(originalModifiedAt);
      });
    });
  });

  describe('togglePin', () => {
    it('should update modifiedAt when pinning', async () => {
      const note = await noteService.createNote('Test content', ['test']);
      const originalModifiedAt = note.modifiedAt;

      await new Promise(resolve => setTimeout(resolve, 10));

      await noteService.togglePin(note.id);

      const updatedNote = await noteRepository.getById(note.id);
      expect(updatedNote?.pinned).toBe(true);
      expect(updatedNote?.modifiedAt).not.toBe(originalModifiedAt);
    });

    it('should set needsSync when pinning', async () => {
      const note = await noteService.createNote('Test content', ['test']);

      // Clear needsSync
      const rawNote = await noteRepository.getById(note.id);
      if (rawNote) {
        rawNote.needsSync = false;
        await noteRepository.update(rawNote, false, true);
      }

      await noteService.togglePin(note.id);

      const updatedNote = await noteRepository.getById(note.id);
      expect(updatedNote?.needsSync).toBe(true);
    });
  });

  describe('decrypt failure reporting', () => {
    it('records undecryptable notes in vaultHealthService during batched load', async () => {
      resetVaultHealth();
      const good = await noteService.createNote('readable note', []);

      // Insert a note whose ciphertext cannot be decrypted with the vault key
      const corrupt: Note = {
        ...good,
        id: 'corrupt-note-id',
        content: JSON.stringify({
          ciphertext: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
          iv: 'AAAAAAAAAAAAAAAA',
        }),
        tags: [],
      };
      await noteRepository.create(corrupt);

      const firstBatch = await noteService.getAllNotesBatched('recent', 200);

      expect(firstBatch.map(n => n.id)).toContain(good.id);
      expect(firstBatch.map(n => n.id)).not.toContain('corrupt-note-id');
      const failures = get(undecryptableNotes);
      expect(failures).toHaveLength(1);
      expect(failures[0].id).toBe('corrupt-note-id');
      expect(failures[0].error).toBeTruthy();
    });
  });

  describe('restoreVersion', () => {
    const syncTag = async (tag: string) => JSON.stringify(await cryptoService.encryptText(JSON.stringify(tag), masterKey));

    it.each([
      ['one ciphertext per tag', async () => [await syncTag('a'), await syncTag('b')], ['a', 'b']],
      ['a single tag in that format, which looks like a stored blob', async () => [await syncTag('a')], ['a']],
    ])('stores a version\'s tags held as %s the way a note keeps them', async (_name, tags, expected) => {
      const created = await noteService.createNote('now');
      const version = await noteRepository.getById(created.id) as Note;

      await noteService.restoreVersion(created.id, { ...version, tags: await tags() });

      const restored = await noteRepository.getById(created.id) as Note;
      expect(restored.tags).toHaveLength(1);
      expect((await noteService.getNote(created.id))?.tags).toEqual(expected);
    });
  });

  describe('locked and archived notes', () => {
    const lockedNote = async (state: 'lock' | 'archive') => {
      const note = await noteService.createNote('protected', ['tag']);
      await noteRepository[state](note.id);
      return note;
    };

    for (const state of ['lock', 'archive'] as const) {
      it(`refuses a change to the text once you ${state} it`, async () => {
        const note = await lockedNote(state);
        await expect(noteService.updateNote(note.id, { content: 'changed' })).rejects.toBeInstanceOf(NoteReadOnlyError);
      });

      it(`refuses a change to the tags once you ${state} it`, async () => {
        const note = await lockedNote(state);
        await expect(noteService.updateNote(note.id, { tags: ['other'] })).rejects.toBeInstanceOf(NoteReadOnlyError);
      });

      it(`refuses a change to the attachments once you ${state} it`, async () => {
        const note = await lockedNote(state);
        const attachment = { id: 'a', filename: 'f', mimeType: 'text/plain', size: 1, data: 'a' };
        await expect(noteService.updateNote(note.id, { attachments: [attachment] })).rejects.toBeInstanceOf(NoteReadOnlyError);
      });

      it(`refuses a version restore once you ${state} it`, async () => {
        const note = await lockedNote(state);
        const stored = await noteRepository.getById(note.id) as Note;
        await expect(noteService.restoreVersion(note.id, stored)).rejects.toBeInstanceOf(NoteReadOnlyError);
      });
    }

    it('allows a save that changes nothing, which is all an autosave does', async () => {
      const note = await lockedNote('lock');
      await expect(noteService.updateNote(note.id, { content: 'protected', tags: ['tag'] })).resolves.toBeTruthy();
    });

    it('still allows what doesn\'t touch the note\'s contents', async () => {
      const note = await lockedNote('lock');
      await expect(noteService.updateNote(note.id, { pinned: true })).resolves.toBeTruthy();
    });

    it('allows editing again once unlocked', async () => {
      const note = await lockedNote('lock');
      await noteRepository.unlock(note.id);
      await expect(noteService.updateNote(note.id, { content: 'changed' })).resolves.toBeTruthy();
    });
  });

  describe('a long run of saves between syncs', () => {
    it('keeps the hash the server holds, or the push comes back as a conflict', async () => {
      const created = await noteService.createNote('start');
      const synced = await noteRepository.getById(created.id) as Note;
      await noteRepository.update({ ...synced, needsSync: false }, false, true);

      for (let save = 0; save < 60; save++) {
        await noteService.updateNote(created.id, { content: `save ${save}` });
      }

      const note = await noteRepository.getById(created.id) as Note;
      expect(note.hashChain).toContain(synced.contentHash);
      expect(note.hashChain?.[0]).toBe(note.contentHash);
    });

    it('goes back to the usual length once it has synced', async () => {
      const created = await noteService.createNote('start');
      const synced = await noteRepository.getById(created.id) as Note;
      await noteRepository.update({ ...synced, needsSync: false }, false, true);
      for (let save = 0; save < 60; save++) {
        await noteService.updateNote(created.id, { content: `save ${save}` });
      }

      const pushed = await noteRepository.getById(created.id) as Note;
      await noteRepository.update({ ...pushed, needsSync: false }, false, true);
      await noteService.updateNote(created.id, { content: 'after' });

      expect((await noteRepository.getById(created.id) as Note).hashChain?.length).toBe(MAX_HASH_CHAIN_LENGTH);
    });
  });
});

describe('duplicateNote', () => {
  const FILE_ID = '5b1d3a7c-2f4e-4c6b-8a9d-0e1f2a3b4c5d';
  const OTHER_ID = '6c2e4b8d-3a5f-4d7c-9bae-1f2a3b4c5d6e';
  const attachment = (id: string, thumbnailData?: string) => ({
    id, filename: '{"ciphertext":"x","iv":"y"}', mimeType: 'image/png', size: 3, data: id, thumbnailData,
  });

  beforeEach(async () => {
    const { masterKey } = await initTestDB();
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  async function noteWithFile(content: string, thumbnail = false) {
    await attachmentRepository.storeBlob(FILE_ID, new Uint8Array([1, 2, 3]).buffer);
    if (thumbnail) {
      await attachmentRepository.storeThumbnail(FILE_ID, new Uint8Array([9]).buffer);
    }
    return noteService.createNote(content, [], { attachments: [attachment(FILE_ID, thumbnail ? FILE_ID : undefined)] });
  }

  it('copies each attachment under a new id, so that the two notes share nothing', async () => {
    const original = await noteWithFile('text');

    const copy = await noteService.duplicateNote(original.id);

    expect(copy.attachments).toHaveLength(1);
    expect(copy.attachments[0].id).not.toBe(FILE_ID);
    expect(copy.attachments[0].data).toBe(copy.attachments[0].id);
    expect(new Uint8Array((await attachmentRepository.getBlob(copy.attachments[0].data))!)).toEqual(new Uint8Array([1, 2, 3]));
  });

  it('points the copied text at the copies', async () => {
    const original = await noteWithFile(`![a](attachment:${FILE_ID}) and [b](attachment:${FILE_ID})`);

    const copy = await noteService.duplicateNote(original.id);

    const newId = copy.attachments[0].id;
    expect((await noteService.getNote(copy.id))?.content).toBe(`![a](attachment:${newId}) and [b](attachment:${newId})`);
  });

  it('survives the original being emptied from the bin', async () => {
    const original = await noteWithFile('text');
    const copy = await noteService.duplicateNote(original.id);

    await noteService.permanentlyDeleteNote(original.id);

    expect(await attachmentRepository.getBlob(copy.attachments[0].data)).toBeTruthy();
  });

  it('copies a thumbnail too', async () => {
    const original = await noteWithFile('text', true);

    const copy = await noteService.duplicateNote(original.id);

    const { id, thumbnailData } = copy.attachments[0];
    expect(thumbnailData).toBe(id);
    expect(await attachmentRepository.getThumbnail(id)).toBeTruthy();
  });

  it('keeps the colour', async () => {
    const original = await noteService.createNote('text', [], { color: 'red' });

    const copy = await noteService.duplicateNote(original.id);

    expect(copy.color).toBe(original.color);
  });

  it('changes nothing if a blob is missing, rather than leaving a broken copy', async () => {
    const original = await noteWithFile('text');
    await attachmentRepository.deleteBlob(FILE_ID);
    const before = await noteRepository.getAll();

    await expect(noteService.duplicateNote(original.id)).rejects.toThrow();

    expect(await noteRepository.getAll()).toHaveLength(before.length);
  });

  it('leaves nothing behind when a later attachment turns out to be missing', async () => {
    await attachmentRepository.storeBlob(FILE_ID, new Uint8Array([1]).buffer);
    const original = await noteService.createNote('text', [], { attachments: [attachment(FILE_ID), attachment(OTHER_ID)] });
    const before = (await attachmentRepository.listAllIds()).length;

    await expect(noteService.duplicateNote(original.id)).rejects.toThrow();

    expect(await attachmentRepository.listAllIds()).toHaveLength(before);
  });
});

describe('purgeOldDeletedNotes', () => {
  const DAY = 24 * 60 * 60 * 1000;
  const daysAgo = (days: number) => new Date(Date.now() - days * DAY).toISOString();

  beforeEach(async () => {
    const { masterKey } = await initTestDB();
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  async function inTheBin(fields: Partial<Note>) {
    const { id } = await noteService.createNote('text');
    const note = await noteRepository.getById(id);
    await noteRepository.update({ ...note!, deleted: true, ...fields }, false, true);
    return id;
  }

  it('removes a note that has been in the bin here for 30 days', async () => {
    const id = await inTheBin({ deletedAt: daysAgo(31), binnedAt: daysAgo(31) });

    expect(await noteService.purgeOldDeletedNotes()).toBe(1);

    expect(await noteRepository.getById(id)).toBeFalsy();
  });

  it('tells the server, so that it goes from other devices too', async () => {
    const id = await inTheBin({ deletedAt: daysAgo(31), binnedAt: daysAgo(31), syncedAt: daysAgo(40) });

    await noteService.purgeOldDeletedNotes();

    expect((await noteRepository.getPendingDeletions()).map(deletion => deletion.id)).toContain(id);
  });

  it('leaves one that has been there less time', async () => {
    const id = await inTheBin({ deletedAt: daysAgo(29), binnedAt: daysAgo(29) });

    expect(await noteService.purgeOldDeletedNotes()).toBe(0);

    expect(await noteRepository.getById(id)).toBeTruthy();
  });

  it('does not trust deletedAt, which a server can set, so nothing skips the bin', async () => {
    const id = await inTheBin({ deletedAt: daysAgo(90) });

    expect(await noteService.purgeOldDeletedNotes()).toBe(0);

    const note = await noteRepository.getById(id);
    expect(note).toBeTruthy();
    expect(Date.now() - Date.parse(note!.binnedAt as string)).toBeLessThan(60_000);
  });

  it('starts the clock again for a note restored and deleted later', async () => {
    const id = await inTheBin({ deletedAt: daysAgo(31), binnedAt: daysAgo(31) });

    await noteRepository.restore(id);

    expect((await noteRepository.getById(id))?.binnedAt).toBeUndefined();
  });
});

