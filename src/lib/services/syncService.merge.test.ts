/**
 * How a pull decides between the local and remote copy of a note, and what a push
 * may safely mark as synced. Each of these used to lose an edit or a state change
 * without any sign of it.
 */
import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll } from 'vitest';
import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import { get } from 'svelte/store';
import { syncService } from './syncService';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { noteService } from './noteService';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';
import { notes, settings } from '../stores/appStore';
import { searchService } from './searchService';
import { DEFAULT_SETTINGS } from '../types';
import type { Note, SyncNote, SyncPullResponse, SyncPushRequest, SyncPushResponse } from '../types';

const ENDPOINT = 'https://sync.example.com';
const NOTE_ID = 'a0000000-0000-4000-8000-000000000020';
const ATTACHMENT_A = 'b0000000-0000-4000-8000-000000000020';
const ATTACHMENT_B = 'b0000000-0000-4000-8000-000000000021';
const NOW = '2026-11-05T01:02:03.000Z';
const LATER = '2026-11-05T02:00:00.000Z';
const HASH_1 = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=';
const HASH_2 = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb=';
const HASH_3 = 'ccccccccccccccccccccccccccccccccccccccccccc=';

const server = setupServer();

describe('sync merge', () => {
  let masterKey: CryptoKey;

  beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
  afterAll(() => server.close());

  beforeEach(async () => {
    masterKey = (await initTestDB()).masterKey;
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
    settings.set({ ...DEFAULT_SETTINGS, syncEnabled: true });
    server.resetHandlers();

    await syncRepository.updateMetadata({
      apiKey: JSON.stringify(await cryptoService.encryptText('key', masterKey)),
      clientId: 'client',
      syncEnabled: true,
      syncEndpoint: ENDPOINT,
    });
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    settings.set(DEFAULT_SETTINGS);
    await cleanupTestDB();
  });

  const encrypted = async (text: string) => JSON.stringify(await cryptoService.encryptText(text, masterKey));

  async function localNote(overrides: Partial<Note> = {}): Promise<Note> {
    const note = createTestNote({
      id: NOTE_ID,
      content: await encrypted('local text'),
      contentHash: HASH_1,
      parentHash: null,
      hashChain: [HASH_1],
      needsSync: false,
      modifiedAt: NOW,
      ...overrides,
    });
    await noteRepository.create(note);
    return note;
  }

  async function remoteNote(overrides: Record<string, unknown> = {}): Promise<SyncNote> {
    return {
      id: NOTE_ID, createdAt: NOW, modifiedAt: LATER, content: await encrypted('remote text'), tags: [], attachments: [],
      pinned: false, archived: false, deleted: false, version: 2,
      contentHash: HASH_2, parentHash: HASH_3, hashChain: [HASH_2, HASH_3], ...overrides,
    } as SyncNote;
  }

  function serve(pull: Partial<SyncPullResponse>, onPush?: (body: SyncPushRequest) => Promise<Partial<SyncPushResponse>>) {
    server.use(
      http.get(`${ENDPOINT}/api/v1/sync/status`, () => HttpResponse.json({ serverTime: NOW, version: '1', pendingNotes: 0, totalNotes: 0 })),
      http.post(`${ENDPOINT}/api/v1/sync/push`, async ({ request }) => {
        const body = await request.json() as SyncPushRequest;
        return HttpResponse.json({ accepted: [], rejected: [], ...(await onPush?.(body)) });
      }),
      http.post(`${ENDPOINT}/api/v1/sync/pull`, () => HttpResponse.json({ notes: [], attachments: [], versions: [], syncedAt: NOW, ...pull })),
    );
  }

  const stored = () => noteRepository.getById(NOTE_ID) as Promise<Note>;
  const textOf = async (note: Note) => cryptoService.decryptText(JSON.parse(note.content), masterKey);

  describe('edits made while a push is in flight', () => {
    it('stay marked for sync', async () => {
      await localNote({ needsSync: true });

      serve({}, async () => {
        const note = await stored();
        await noteRepository.update({ ...note, content: await encrypted('typed during the push') });
        return { accepted: [{ id: NOTE_ID, serverVersion: 2, syncedAt: NOW }] };
      });

      await syncService.syncNow();

      expect((await stored()).needsSync).toBe(true);
    });

    it('are sent by the next sync', async () => {
      await localNote({ needsSync: true });
      const pushed: string[] = [];

      serve({}, async body => {
        pushed.push(...body.notes.map(n => n.content));
        if (pushed.length === 1) {
          const note = await stored();
          await noteRepository.update({ ...note, content: await encrypted('typed during the push') });
        }
        return { accepted: [{ id: NOTE_ID, serverVersion: 2, syncedAt: NOW }] };
      });

      await syncService.syncNow();
      await syncService.syncNow();

      expect(pushed).toHaveLength(2);
      expect(await cryptoService.decryptText(JSON.parse(pushed[1]), masterKey)).toBe('typed during the push');
      expect((await stored()).needsSync).toBe(false);
    });

    it('are not lost when the server rejects the push', async () => {
      await localNote({ needsSync: true });

      serve({}, async () => {
        const note = await stored();
        await noteRepository.update({ ...note, content: await encrypted('typed during the push') });
        return {
          rejected: [{
            id: NOTE_ID, reason: 'conflict', serverModifiedAt: NOW, serverContent: 'x', serverTags: [],
            serverVersion: 2, serverAttachments: [], serverPinned: false,
          }],
        };
      });

      await syncService.syncNow();

      expect((await stored()).needsSync).toBe(true);
    });

    it('clears the flag when nothing changed', async () => {
      await localNote({ needsSync: true });
      serve({}, async () => ({ accepted: [{ id: NOTE_ID, serverVersion: 2, syncedAt: NOW }] }));

      await syncService.syncNow();

      expect((await stored()).needsSync).toBe(false);
    });

    it('ignores a server claiming to accept a note that was not pushed', async () => {
      await localNote({ needsSync: true });
      const unpushed = 'a0000000-0000-4000-8000-000000000021';

      serve({}, async () => {
        await noteRepository.create(createTestNote({ id: unpushed, needsSync: true }));
        return {
          accepted: [
            { id: NOTE_ID, serverVersion: 2, syncedAt: NOW },
            { id: unpushed, serverVersion: 2, syncedAt: NOW },
          ],
        };
      });

      await syncService.syncNow();

      expect((await noteRepository.getById(unpushed))?.needsSync).toBe(true);
    });
  });

  describe('a remote update whose history cannot be related', () => {
    it('is accepted when the local copy has nothing unsent', async () => {
      await localNote();
      serve({ notes: [await remoteNote()] });

      await syncService.syncNow();

      const note = await stored();
      expect(await textOf(note)).toBe('remote text');
      expect(note.contentHash).toBe(HASH_2);
    });

    it('raises a conflict when the local copy has unsent changes', async () => {
      await localNote({ needsSync: true });
      serve({ notes: [await remoteNote()] }, async () => ({ accepted: [] }));

      await syncService.syncNow();

      expect(await textOf(await stored())).toBe('local text');
      expect((await syncRepository.getNoteSyncMetadata(NOTE_ID))?.lastSyncStatus).toBe('conflict');
    });

    it('is still ignored when the local copy is ahead of it', async () => {
      await localNote({ contentHash: HASH_1, hashChain: [HASH_1, HASH_3] });
      serve({ notes: [await remoteNote({ parentHash: HASH_3 })] });

      await syncService.syncNow();

      expect(await textOf(await stored())).toBe('local text');
    });

    it('is still a fast-forward when it descends from the local copy', async () => {
      await localNote();
      serve({ notes: [await remoteNote({ parentHash: HASH_3, hashChain: [HASH_2, HASH_3, HASH_1] })] });

      await syncService.syncNow();

      expect(await textOf(await stored())).toBe('remote text');
    });
  });

  describe('a remote change that leaves the hash the same', () => {
    // The same hash on both sides, and the state createTestNote gives the local copy
    const sameHash = { contentHash: HASH_1, parentHash: null, hashChain: [HASH_1], syntaxLanguage: 'plain', wordWrap: true };

    it.each([
      ['pinned', { pinned: true }, (n: Note) => n.pinned === true],
      ['archived', { archived: true, archivedAt: LATER }, (n: Note) => n.archived === true],
      ['locked', { locked: true, lockedAt: LATER }, (n: Note) => n.locked === true],
      ['deleted', { deleted: true, deletedAt: LATER }, (n: Note) => n.deleted === true],
      ['colour', { color: 'red' }, (n: Note) => n.color === 'red'],
    ])('applies %s to a clean note', async (_name, change, check) => {
      await localNote();
      serve({ notes: [await remoteNote({ ...sameHash, ...change })] });

      await syncService.syncNow();

      expect(check(await stored())).toBe(true);
    });

    it('applies an un-delete too', async () => {
      await localNote({ deleted: true, deletedAt: NOW });
      serve({ notes: [await remoteNote({ ...sameHash, deleted: false })] });

      await syncService.syncNow();

      expect((await stored()).deleted).toBe(false);
    });

    it('leaves a note with unsent changes alone', async () => {
      await localNote({ needsSync: true });
      serve({ notes: [await remoteNote({ ...sameHash, pinned: true })] }, async () => ({ accepted: [] }));

      await syncService.syncNow();

      expect((await stored()).pinned).toBe(false);
    });

    it('does nothing when the state already matches', async () => {
      const local = await localNote();
      serve({ notes: [await remoteNote({ ...sameHash })] });

      await syncService.syncNow();

      expect((await stored()).content).toBe(local.content);
      expect((await stored()).modifiedAt).toBe(NOW);
    });

    it('picks up an attachment added elsewhere', async () => {
      const a = { id: ATTACHMENT_A, filename: 'a', mimeType: 'image/png', size: 1, data: ATTACHMENT_A };
      const b = { id: ATTACHMENT_B, filename: 'b', mimeType: 'image/png', size: 1, data: ATTACHMENT_B };
      await localNote({ attachments: [a] });
      serve({ notes: [await remoteNote({ ...sameHash, attachments: [a, b] })] });

      await syncService.syncNow();

      expect((await stored()).attachments.map(x => x.id).sort()).toEqual([ATTACHMENT_A, ATTACHMENT_B]);
    });

    it('never loses an attachment because the server sent a shorter list', async () => {
      // Some clients send empty lists by mistake, and the server can return a note
      // without an attachment it holds, so a missing entry doesn't mean a removal
      const a = { id: ATTACHMENT_A, filename: 'a', mimeType: 'image/png', size: 1, data: ATTACHMENT_A };
      await localNote({ attachments: [a] });
      serve({ notes: [await remoteNote({ ...sameHash, pinned: true, attachments: [] })] });

      await syncService.syncNow();

      const note = await stored();
      expect(note.pinned).toBe(true);
      expect(note.attachments.map(x => x.id)).toEqual([ATTACHMENT_A]);
    });
  });

  describe('the note list after a sync', () => {
    async function archivedNote() {
      const note = createTestNote({
        content: await encrypted('archived text'),
        tags: [],
        archived: true,
        archivedAt: NOW,
      });
      await noteRepository.create(note);
      return note.id;
    }

    it('keeps archived notes after a pull', async () => {
      const archivedId = await archivedNote();
      serve({ notes: [await remoteNote()] });

      await syncService.syncNow();

      expect(get(notes).map(note => note.id)).toContain(archivedId);
    });

    it('keeps archived notes after a push', async () => {
      const archivedId = await archivedNote();
      await localNote({ needsSync: true });
      serve({}, async () => ({ accepted: [{ id: NOTE_ID, serverVersion: 2, syncedAt: NOW }] }));

      await syncService.syncNow();

      expect(get(notes).map(note => note.id)).toContain(archivedId);
    });

    it('keeps archived notes in the search index', async () => {
      const archivedId = await archivedNote();
      serve({ notes: [await remoteNote()] });

      await syncService.syncNow();

      const found = await searchService.searchNotes('archived', get(notes), 'recent', true);
      expect(found.map(note => note.id)).toContain(archivedId);
    });

    it('still leaves deleted notes out', async () => {
      const deleted = createTestNote({ content: await encrypted('gone'), tags: [], deleted: true, deletedAt: NOW });
      await noteRepository.create(deleted);
      serve({ notes: [await remoteNote()] });

      await syncService.syncNow();

      expect(get(notes).map(note => note.id)).not.toContain(deleted.id);
    });
  });

  describe('state changes made here', () => {
    async function expectHashAdvanced(change: () => Promise<unknown>) {
      const before = await stored();
      await change();
      const after = await stored();
      expect(after.contentHash).not.toBe(before.contentHash);
      expect(after.parentHash).toBe(before.contentHash);
      expect(after.hashChain?.[0]).toBe(after.contentHash);
      expect(after.hashChain).toContain(before.contentHash);
      expect(after.needsSync).toBe(true);
    }

    beforeEach(async () => {
      const note = createTestNote({ id: NOTE_ID, content: await encrypted('text'), needsSync: false });
      note.contentHash = await cryptoService.computeContentHash(note);
      note.parentHash = null;
      note.hashChain = [note.contentHash];
      await noteRepository.create(note);
    });

    it('advances the hash when pinned', () => expectHashAdvanced(() => noteService.togglePin(NOTE_ID)));
    it('advances the hash when archived', () => expectHashAdvanced(() => noteService.archiveNote(NOTE_ID)));
    it('advances the hash when locked', () => expectHashAdvanced(() => noteService.lockNote(NOTE_ID)));

    it('advances the hash when unarchived', async () => {
      await noteService.archiveNote(NOTE_ID);
      await expectHashAdvanced(() => noteService.unarchiveNote(NOTE_ID));
    });

    it('advances the hash when unlocked', async () => {
      await noteService.lockNote(NOTE_ID);
      await expectHashAdvanced(() => noteService.unlockNote(NOTE_ID));
    });

    it('advances the hash when a version is restored', async () => {
      const oldContent = await encrypted('older text');
      await expectHashAdvanced(() => noteService.restoreVersion(NOTE_ID, {
        content: oldContent, tags: [], attachments: [], syntaxLanguage: 'plain', wordWrap: true,
      }));
      expect((await stored()).content).toBe(oldContent);
    });
  });
});
