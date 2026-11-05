/**
 * Exporting selected notes has to produce what any other export produces: readable
 * filenames and the attachment's content, not the stored ciphertext and blob id.
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { exportNotes } from './bulkOperationsService';
import { attachmentService } from './attachmentService';
import { noteService } from './noteService';
import { noteRepository } from './noteRepository';
import { keyManager } from './keyManager';
import { notes } from '../stores/appStore';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';
import type { Note } from '../types';

describe('bulk export', () => {
  beforeEach(async () => {
    const testDB = await initTestDB();
    keyManager.setMasterKey({
      key: testDB.masterKey,
      keyBytes: new Uint8Array(32),
      derivedAt: Date.now(),
    });
  });

  afterEach(async () => {
    notes.set([]);
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  async function noteWithAttachment(): Promise<string> {
    const note = await noteService.createNote('has a file');
    const attachment = await attachmentService.addAttachment(
      new File(['hello there'], 'report.txt', { type: 'text/plain' })
    );
    const stored = (await noteRepository.getById(note.id)) as Note;
    await noteRepository.update({ ...stored, attachments: [attachment] }, false, true);
    notes.set(await noteService.getAllNotes());
    return note.id;
  }

  it('writes readable filenames and the attachment content', async () => {
    const noteId = await noteWithAttachment();

    const exported = JSON.parse(await exportNotes([noteId]));
    const attachment = exported.notes[0].attachments[0];

    expect(attachment.filename).toBe('report.txt');
    expect(atob(attachment.data)).toBe('hello there');
  });

  it('reports progress through to completion', async () => {
    const noteId = await noteWithAttachment();
    const statuses: string[] = [];

    await exportNotes([noteId], progress => statuses.push(progress.status));

    expect(statuses[statuses.length - 1]).toBe('complete');
  });
});
