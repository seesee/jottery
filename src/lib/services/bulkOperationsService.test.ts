import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';
import { keyManager } from './keyManager';
import { noteService } from './noteService';
import { noteRepository } from './noteRepository';
import { addTagsToNotes, removeTagsFromNotes } from './bulkOperationsService';
import { notes } from '../stores/appStore';
import type { DecryptedNote } from '../types';

describe('bulk operations', () => {
  let created: DecryptedNote[];

  beforeEach(async () => {
    const { masterKey } = await initTestDB();
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
    created = [];
    for (let i = 0; i < 5; i++) {
      const note = await noteService.createNote(`note ${i}`, ['old']);
      created.push(await noteService.getNote(note.id) as DecryptedNote);
    }
    notes.set(created);
  });

  afterEach(async () => {
    notes.set([]);
    keyManager.clearMasterKey();
    vi.restoreAllMocks();
    await cleanupTestDB();
  });

  /** How many times the store changes while `run` does */
  async function storeUpdates(run: () => Promise<void>): Promise<number> {
    let count = -1; // Subscribing reports the current value once
    const stop = notes.subscribe(() => { count++; });
    await run();
    stop();
    return count;
  }

  const ids = () => created.map(note => note.id);

  it('updates the store once however many notes are selected', async () => {
    // Each update re-runs search and sorting over the whole vault
    expect(await storeUpdates(() => addTagsToNotes(ids(), ['new']))).toBe(1);
    expect(await storeUpdates(() => removeTagsFromNotes(ids(), ['new']))).toBe(1);
  });

  it('still gives every note its tags', async () => {
    await addTagsToNotes(ids(), ['new']);

    expect(get(notes).every(note => note.tags.includes('new'))).toBe(true);
  });

  it('skips a read-only note rather than stopping partway', async () => {
    await noteRepository.archive(created[0].id);
    notes.set(created.map((note, i) => (i === 0 ? { ...note, archived: true } : note)));

    await addTagsToNotes(ids(), ['new']);

    const after = get(notes);
    expect(after[0].tags).not.toContain('new');
    expect(after[1].tags).toContain('new');
  });
});
