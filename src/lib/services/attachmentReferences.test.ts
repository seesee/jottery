/**
 * A blob can be referenced by more than one note, and by version snapshots. Deleting
 * it while something still points at it loses the file: a combined note used to lose
 * its attachments when the bin was emptied, and restoring a version could bring back
 * a reference to a file that had gone.
 */
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { attachmentRepository } from './attachmentRepository';
import { combineNotes } from './bulkOperationsService';
import { keyManager } from './keyManager';
import { noteRepository } from './noteRepository';
import { noteService } from './noteService';
import { versionRepository } from './versionRepository';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';
import { notes } from '../stores/appStore';
import type { Attachment, Note } from '../types';

const BLOB = 'b0000000-0000-4000-8000-000000000040';

function attachment(id = BLOB): Attachment {
  return { id, filename: '{}', mimeType: 'image/png', size: 4, data: id };
}

describe('attachment blobs', () => {
  beforeEach(async () => {
    const { masterKey } = await initTestDB();
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
    await attachmentRepository.storeBlob(BLOB, new ArrayBuffer(4));
    notes.set([]);
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    notes.set([]);
    await cleanupTestDB();
  });

  const blobExists = async () => (await attachmentRepository.getBlob(BLOB)) !== null && (await attachmentRepository.getBlob(BLOB)) !== undefined;

  async function noteWithAttachment(text = 'text'): Promise<Note> {
    return noteService.createNote(text, [], { attachments: [attachment()] });
  }

  describe('removing an attachment from a note', () => {
    it('deletes the blob when nothing else refers to it', async () => {
      // Added after the first edit's snapshot, so no version holds it
      const note = await noteService.createNote('text', []);
      await noteService.updateNote(note.id, { attachments: [attachment()] });

      await noteService.updateNote(note.id, { attachments: [] });

      expect(await blobExists()).toBe(false);
    });

    it('keeps the blob held by the snapshot a first edit takes', async () => {
      const note = await noteWithAttachment();

      await noteService.updateNote(note.id, { attachments: [] });

      expect(await blobExists()).toBe(true);
    });

    it('keeps the blob while a version snapshot refers to it', async () => {
      const note = await noteWithAttachment();
      await versionRepository.createVersion(note, { syncedAt: new Date().toISOString(), reason: 'manual-sync' });

      await noteService.updateNote(note.id, { attachments: [] });

      expect(await blobExists()).toBe(true);
    });

    it('keeps the blob while another note refers to it', async () => {
      const note = await noteWithAttachment();
      await noteWithAttachment('another note');

      await noteService.updateNote(note.id, { attachments: [] });

      expect(await blobExists()).toBe(true);
    });
  });

  describe('permanently deleting a note', () => {
    it('deletes the blob when nothing else refers to it', async () => {
      const note = await noteWithAttachment();

      await noteService.permanentlyDeleteNote(note.id);

      expect(await blobExists()).toBe(false);
    });

    it('keeps the blob while another note refers to it', async () => {
      const note = await noteWithAttachment();
      await noteWithAttachment('another note');

      await noteService.permanentlyDeleteNote(note.id);

      expect(await blobExists()).toBe(true);
    });

    it('takes its versions with it, so they do not keep the blob in use', async () => {
      const note = await noteWithAttachment();
      await versionRepository.createVersion(note, { syncedAt: new Date().toISOString(), reason: 'manual-sync' });

      await noteService.permanentlyDeleteNote(note.id);

      expect(await versionRepository.getVersionsForNote(note.id)).toEqual([]);
      expect(await blobExists()).toBe(false);
    });

    it('also removes the note itself', async () => {
      const note = await noteWithAttachment();
      await noteService.permanentlyDeleteNote(note.id);
      expect(await noteRepository.getById(note.id)).toBeNull();
    });
  });

  describe('combining notes', () => {
    it('keeps the attachments when the originals are removed from the bin', async () => {
      const first = await noteWithAttachment('first');
      const second = await noteService.createNote('second', []);
      notes.set(await noteService.getAllNotes());

      const combined = await combineNotes([first.id, second.id]);
      for (const deleted of await noteService.getDeletedNotes()) {
        await noteService.permanentlyDeleteNote(deleted.id);
      }

      expect(await blobExists()).toBe(true);
      expect((await noteRepository.getById(combined.id))?.attachments.map(a => a.id)).toEqual([BLOB]);
    });
  });
});
