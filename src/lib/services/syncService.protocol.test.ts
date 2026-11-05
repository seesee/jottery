/**
 * docs/sync-protocol.md, as a client must follow it: the pull cursor, authoritative
 * attachment lists and refused deletions, against both a server that has them and
 * one that doesn't. Released servers stay in service, so neither is a special case.
 */
import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll, vi } from 'vitest';

// The message key is what matters here, not its translation
vi.mock('svelte-i18n', () => ({
  _: { subscribe: (run: (format: (key: string) => string) => void) => { run((key: string) => key); return () => undefined; } },
}));

import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import { get } from 'svelte/store';
import { syncService } from './syncService';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';
import { inboxItems, settings } from '../stores/appStore';
import { DEFAULT_SETTINGS } from '../types';
import type { Note, SyncNote, SyncPullRequest, SyncPullResponse, SyncPushRequest, SyncPushResponse } from '../types';

const ENDPOINT = 'https://sync.example.com';
const ID = (n: number) => `a0000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const NOW = '2026-11-05T01:02:03.000Z';
const LATER = '2026-11-05T02:00:00.000Z';
const HASH = (c: string) => c.repeat(43) + '=';

const server = setupServer();

type PullResponse = Partial<SyncPullResponse> & { nextCursor?: unknown };

describe('sync protocol', () => {
  let masterKey: CryptoKey;
  /** Every pull request the server received, in order */
  let pulls: SyncPullRequest[];
  let pushes: SyncPushRequest[];

  beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
  afterAll(() => server.close());

  beforeEach(async () => {
    masterKey = (await initTestDB()).masterKey;
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
    settings.set({ ...DEFAULT_SETTINGS, syncEnabled: true });
    server.resetHandlers();
    pulls = [];
    pushes = [];
    inboxItems.set([]);

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

  async function remoteNote(n: number, overrides: Record<string, unknown> = {}): Promise<SyncNote> {
    return {
      id: ID(n), createdAt: NOW, modifiedAt: LATER, content: await encrypted(`remote ${n}`), tags: [], attachments: [],
      pinned: false, archived: false, deleted: false, version: 2,
      contentHash: HASH('b'), parentHash: HASH('c'), hashChain: [HASH('b'), HASH('c')], ...overrides,
    } as SyncNote;
  }

  /** The server: `pull` sees each request and says what comes back */
  function serve(
    pull: (request: SyncPullRequest, index: number) => PullResponse,
    push?: (request: SyncPushRequest) => Partial<SyncPushResponse> & Record<string, unknown>,
  ) {
    server.use(
      http.get(`${ENDPOINT}/api/v1/sync/status`, () => HttpResponse.json({ serverTime: NOW, version: '1', pendingNotes: 0, totalNotes: 0 })),
      http.post(`${ENDPOINT}/api/v1/sync/push`, async ({ request }) => {
        const body = await request.json() as SyncPushRequest;
        pushes.push(body);
        return HttpResponse.json({ accepted: [], rejected: [], ...push?.(body) });
      }),
      http.post(`${ENDPOINT}/api/v1/sync/pull`, async ({ request }) => {
        const body = await request.json() as SyncPullRequest;
        pulls.push(body);
        return HttpResponse.json({ notes: [], attachments: [], versions: [], syncedAt: NOW, ...pull(body, pulls.length - 1) });
      }),
    );
  }

  const metadata = () => syncRepository.getMetadata();

  /** A device that has pulled before and already moved to the cursor */
  const onCursor = (cursor: string) => syncRepository.updateMetadata({ lastSyncAt: NOW, pullCursor: cursor, cursorFullPullDone: true });

  describe('the pull cursor', () => {
    it('starts with an empty cursor, and always sends the fields older servers require', async () => {
      serve(() => ({ nextCursor: 'c1' }));

      await syncService.syncNow();

      expect(pulls[0].cursor).toBe('');
      expect(pulls[0].knownNoteIds).toEqual([]);
      expect(pulls[0].knownAttachmentIds).toEqual([]);
      expect(pulls[0].offset).toBe(0);
    });

    it('sends the stored cursor, and stores the next one', async () => {
      await onCursor('c5');
      serve(() => ({ nextCursor: 'c6' }));

      await syncService.syncNow();

      expect(pulls[0].cursor).toBe('c5');
      expect((await metadata())?.pullCursor).toBe('c6');
    });

    it('follows hasMore with each cursor, not with an offset', async () => {
      await onCursor('c0');
      serve((_, index) => (index === 0 ? { hasMore: true, nextCursor: 'c1' } : { nextCursor: 'c2' }));

      await syncService.syncNow();

      expect(pulls.map(p => p.cursor)).toEqual(['c0', 'c1']);
      expect((await metadata())?.pullCursor).toBe('c2');
    });

    it('keeps going past a page with no notes, which can be all versions', async () => {
      await onCursor('c0');
      const note = await remoteNote(1);
      serve((_, index) => (index === 0 ? { hasMore: true, nextCursor: 'c1' } : { notes: [note], nextCursor: 'c2' }));

      await syncService.syncNow();

      expect(await noteRepository.getById(ID(1))).toBeTruthy();
    });

    it('does not save a cursor for a page it failed to apply, so the page comes again', async () => {
      await onCursor('c0');
      const original = noteRepository.create.bind(noteRepository);
      noteRepository.create = async () => { throw new Error('storage failed'); };
      const note = await remoteNote(1);
      serve(() => ({ notes: [note], nextCursor: 'c1' }));

      const result = await syncService.syncNow();
      noteRepository.create = original;

      expect(result.success).toBe(false);
      expect((await metadata())?.pullCursor).toBe('c0');
    });

    it('keeps the pages already applied when a later one fails', async () => {
      await onCursor('c0');
      serve((_, index) => {
        if (index === 0) return { hasMore: true, nextCursor: 'c1' };
        return { notes: 'not a list' as never, nextCursor: 'c2' };
      });

      const result = await syncService.syncNow();

      expect(result.success).toBe(false);
      expect((await metadata())?.pullCursor).toBe('c1');
    });

    it('stops rather than spinning if the server never moves', async () => {
      await onCursor('c0');
      serve(() => ({ hasMore: true, nextCursor: 'c0' }));

      await syncService.syncNow();

      expect(pulls).toHaveLength(1);
    });

    it('treats an unusable cursor as no cursor at all', async () => {
      await onCursor('c0');
      serve(() => ({ nextCursor: 'x'.repeat(5000) }));

      await syncService.syncNow();

      expect((await metadata())?.pullCursor).toBeUndefined();
    });

    it('accepts an empty cursor, which a server with no changes yet can send', async () => {
      serve(() => ({ nextCursor: '' }));

      await syncService.syncNow();

      expect((await metadata())?.pullCursor).toBe('');
    });

    it('stores the time the server reports, for falling back on', async () => {
      await onCursor('c0');
      serve(() => ({ nextCursor: 'c1', syncedAt: LATER }));

      await syncService.syncNow();

      expect((await metadata())?.lastSyncAt).toBe(LATER);
    });

    it('is forgotten when the account changes', async () => {
      await onCursor('c9');
      await syncRepository.updateMetadata({ userId: 'first' });
      await syncRepository.updateMetadata({ userId: 'second' });

      expect((await metadata())?.pullCursor).toBeUndefined();
    });

    it('is forgotten when the server changes', async () => {
      await onCursor('c9');
      await syncRepository.updateMetadata({ syncEndpoint: 'https://other.example.com' });

      expect((await metadata())?.pullCursor).toBeUndefined();
    });
  });

  describe('moving to the cursor', () => {
    it('does one full pull the first time a server turns out to support it', async () => {
      await syncRepository.updateMetadata({ lastSyncAt: NOW });
      serve((_, index) => ({ nextCursor: index === 0 ? 'c1' : 'c2' }));

      await syncService.syncNow();

      expect(pulls).toHaveLength(2);
      expect(pulls[0].lastSyncAt).toBeDefined();
      expect(pulls[1].cursor).toBe('');
      expect(pulls[1].lastSyncAt).toBeUndefined();
      expect((await metadata())?.pullCursor).toBe('c2');
    });

    it('never repeats it', async () => {
      await syncRepository.updateMetadata({ lastSyncAt: NOW });
      serve(() => ({ nextCursor: 'c1' }));

      await syncService.syncNow();
      pulls = [];
      await syncService.syncNow();

      expect(pulls).toHaveLength(1);
      expect(pulls[0].cursor).toBe('c1');
    });

    it('is not needed on a device that has never synced', async () => {
      serve(() => ({ nextCursor: 'c1' }));

      await syncService.syncNow();

      expect(pulls).toHaveLength(1);
    });

    it('resumes where it left off if interrupted', async () => {
      await syncRepository.updateMetadata({ lastSyncAt: NOW });
      serve((_, index) => {
        if (index === 0) return { nextCursor: 'c9' };
        if (index === 1) return { hasMore: true, nextCursor: 'c1' };
        return { notes: 'not a list' as never };
      });

      await syncService.syncNow();
      pulls = [];
      serve(() => ({ nextCursor: 'c2' }));
      await syncService.syncNow();

      expect(pulls[0].cursor).toBe('c1');
      expect(pulls).toHaveLength(1);
    });
  });

  describe('an older server', () => {
    it('pages by offset, counting notes', async () => {
      const first = await remoteNote(1);
      const second = await remoteNote(2);
      serve((_, index) => (index === 0 ? { notes: [first, second], hasMore: true } : {}));

      await syncService.syncNow();

      expect(pulls.map(p => p.offset)).toEqual([0, 2]);
    });

    it('asks for the same changes on every page', async () => {
      await syncRepository.updateMetadata({ lastSyncAt: NOW });
      const note = await remoteNote(1);
      serve((_, index) => (index === 0 ? { notes: [note], hasMore: true, syncedAt: LATER } : {}));

      await syncService.syncNow();

      expect(pulls[0].lastSyncAt).toBe(pulls[1].lastSyncAt);
    });

    it('reaches back over the time a slow push can take to commit', async () => {
      await syncRepository.updateMetadata({ lastSyncAt: NOW });
      serve(() => ({}));

      await syncService.syncNow();

      expect(Date.parse(pulls[0].lastSyncAt!)).toBe(Date.parse(NOW) - 2 * 60 * 1000);
    });

    it('stores when the first page began, not when the device finished', async () => {
      // Changes pushed during a long pull are stamped after the first page, and
      // would be skipped by a later time. The device clock isn't the server's at all.
      const note = await remoteNote(1);
      serve((_, index) => (index === 0
        ? { notes: [note], hasMore: true, syncedAt: NOW }
        : { syncedAt: LATER }));

      await syncService.syncNow();

      expect((await metadata())?.lastSyncAt).toBe(NOW);
    });

    it('stores nothing if the pull fails partway, so no page is skipped', async () => {
      await syncRepository.updateMetadata({ lastSyncAt: NOW });
      const note = await remoteNote(1);
      serve((_, index) => (index === 0 ? { notes: [note], hasMore: true, syncedAt: LATER } : { notes: 'bad' as never }));

      await syncService.syncNow();

      expect((await metadata())?.lastSyncAt).toBe(NOW);
    });

    it('stops rather than spinning if pages stop carrying notes', async () => {
      serve(() => ({ hasMore: true }));

      await syncService.syncNow();

      expect(pulls).toHaveLength(1);
    });

    it('falls back cleanly when a server loses the cursor', async () => {
      await onCursor('c5');
      serve(() => ({}));

      await syncService.syncNow();

      expect((await metadata())?.pullCursor).toBeUndefined();
      expect(pulls[0].lastSyncAt).toBeDefined();
    });
  });

  describe('attachmentsAuthoritative', () => {
    async function dirtyNote(n: number, overrides: Partial<Note> = {}): Promise<void> {
      await noteRepository.create(createTestNote({
        id: ID(n), content: await encrypted('x'), needsSync: true,
        contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')], ...overrides,
      }));
    }

    it('is sent on every batch, so a removal reaches the server', async () => {
      for (let n = 1; n <= 101; n++) await dirtyNote(n);
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes.length).toBeGreaterThan(1);
      expect(pushes.every(p => p.attachmentsAuthoritative === true)).toBe(true);
    });

    it('is left off for a push holding a note with no hash, which can be accepted on timestamps alone', async () => {
      // Its list can be stale, and the flag would then remove what another device added
      await dirtyNote(1);
      await dirtyNote(2, { contentHash: undefined, hashChain: undefined });
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes[0].attachmentsAuthoritative).toBeUndefined();
    });

    it('is not sent on a repair push, which carries no notes', async () => {
      await dirtyNote(1);
      serve(() => ({}), body => (body.notes.length > 0
        ? { accepted: [{ id: ID(1) }], attachmentWarnings: [{ noteId: ID(1), attachmentIds: [ID(90)] }] }
        : {}));
      const { attachmentRepository } = await import('./attachmentRepository');
      await attachmentRepository.storeBlob(ID(90), new Uint8Array([1, 2, 3]).buffer);

      await syncService.syncNow();

      expect(pushes).toHaveLength(2);
      expect(pushes[1].notes).toEqual([]);
      expect(pushes[1].attachmentsAuthoritative).toBeUndefined();
    });
  });

  describe('deletions', () => {
    const pending = () => noteRepository.getPendingDeletions();

    async function deleteHere(n: number): Promise<void> {
      await noteRepository.create(createTestNote({ id: ID(n), content: await encrypted('x'), needsSync: false }));
      await noteRepository.permanentDelete(ID(n));
    }

    it('stops sending one the server refused, and the note comes back through the pull', async () => {
      await deleteHere(1);
      const note = await remoteNote(1);
      serve(() => ({ notes: [note] }), () => ({ rejectedDeletions: [ID(1)] }));

      await syncService.syncNow();
      expect(await noteRepository.getById(ID(1))).toBeTruthy();
      expect(await pending()).toEqual([]);

      pushes = [];
      await syncService.syncNow();
      expect(pushes).toEqual([]);
    });

    it('marks the rest as synced', async () => {
      await deleteHere(1);
      await deleteHere(2);
      serve(() => ({}), () => ({ rejectedDeletions: [ID(1)] }));

      await syncService.syncNow();

      expect(await pending()).toEqual([]);
      const { getDB, STORES } = await import('./db');
      const rows = await getDB().getAll(STORES.DELETIONS);
      expect(rows.map(row => row.id)).toEqual([ID(2)]);
    });

    it('treats a server that sends no list as refusing nothing', async () => {
      await deleteHere(1);
      serve(() => ({}));

      await syncService.syncNow();

      expect(await pending()).toEqual([]);
    });

    it('sends a deletion when no note has changed', async () => {
      await deleteHere(1);
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes).toHaveLength(1);
      expect(pushes[0].notes).toEqual([]);
      expect(pushes[0].deletions?.map(d => d.id)).toEqual([ID(1)]);
    });

    it('ignores an id it never sent', async () => {
      await deleteHere(1);
      serve(() => ({}), () => ({ rejectedDeletions: [ID(99), 'not an id' as never] }));

      await syncService.syncNow();

      const { getDB, STORES } = await import('./db');
      expect((await getDB().getAll(STORES.DELETIONS)).map(row => row.id)).toEqual([ID(1)]);
    });
  });

  describe('saved searches', () => {
    it('are sent when no note has changed', async () => {
      const { create } = await import('./savedSearchRepository');
      await create('a', 'b');
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes).toHaveLength(1);
      expect(pushes[0].savedSearches).toHaveLength(1);
    });
  });

  describe('nothing to push', () => {
    it('sends nothing at all', async () => {
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes).toEqual([]);
    });
  });

  describe('locks', () => {
    it('are pushed, so a push here cannot clear one set elsewhere', async () => {
      await noteRepository.create(createTestNote({
        id: ID(1), content: await encrypted('x'), needsSync: true,
        locked: true, lockedAt: NOW, contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')],
      }));
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes[0].notes[0].locked).toBe(true);
      expect(pushes[0].notes[0].lockedAt).toBe(NOW);
    });
  });

  describe('pushes the server refuses', () => {
    const MB = 1024 * 1024;

    async function noteWithBlob(n: number, bytes: number, overrides: Partial<Note> = {}): Promise<void> {
      const { attachmentRepository } = await import('./attachmentRepository');
      await attachmentRepository.storeBlob(ID(100 + n), new Uint8Array(bytes).buffer);
      await noteRepository.create(createTestNote({
        id: ID(n), content: await encrypted('x'), needsSync: true,
        contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')],
        attachments: [{ id: ID(100 + n), filename: 'f', mimeType: 'image/png', size: bytes, data: ID(100 + n) }],
        ...overrides,
      }));
    }

    const accept = (body: SyncPushRequest) => ({ accepted: body.notes.map(n => ({ id: n.id })) });
    const payloadBytes = (body: SyncPushRequest) => body.attachments.reduce((sum, a) => sum + a.data.length, 0);

    /** The server's rule: over the limit fails the whole push, and writes nothing */
    function serveWithLimit(limit: number) {
      server.use(
        http.get(`${ENDPOINT}/api/v1/sync/status`, () => HttpResponse.json({ serverTime: NOW })),
        http.post(`${ENDPOINT}/api/v1/sync/push`, async ({ request }) => {
          const body = await request.json() as SyncPushRequest;
          pushes.push(body);
          if (payloadBytes(body) > limit) return new HttpResponse('too large', { status: 413 });
          return HttpResponse.json({ rejected: [], ...accept(body) });
        }),
        http.post(`${ENDPOINT}/api/v1/sync/pull`, async ({ request }) => {
          pulls.push(await request.json() as SyncPullRequest);
          return HttpResponse.json({ notes: [], attachments: [], versions: [], syncedAt: NOW });
        }),
      );
    }

    it('sizes batches by bytes, not just by note count', async () => {
      await noteWithBlob(1, 2 * MB);
      await noteWithBlob(2, 2 * MB);
      await noteWithBlob(3, 2 * MB);
      serve(() => ({}), accept);

      await syncService.syncNow();

      expect(pushes.length).toBeGreaterThanOrEqual(2);
      for (const body of pushes) {
        expect(payloadBytes(body)).toBeLessThanOrEqual(5 * MB);
      }
      expect(pushes.flatMap(p => p.notes.map(n => n.id)).sort()).toEqual([ID(1), ID(2), ID(3)]);
    });

    it('still pulls when a push fails', async () => {
      await noteWithBlob(1, 16);
      server.use(
        http.get(`${ENDPOINT}/api/v1/sync/status`, () => HttpResponse.json({ serverTime: NOW })),
        http.post(`${ENDPOINT}/api/v1/sync/push`, () => new HttpResponse('nope', { status: 500 })),
        http.post(`${ENDPOINT}/api/v1/sync/pull`, async ({ request }) => {
          pulls.push(await request.json() as SyncPullRequest);
          return HttpResponse.json({ notes: [], attachments: [], versions: [], syncedAt: NOW });
        }),
      );

      const result = await syncService.syncNow();

      expect(pulls).toHaveLength(1);
      expect(result.success).toBe(false);
    });

    it('splits a batch the account limit refuses, so the rest get through', async () => {
      await noteWithBlob(1, 600 * 1024);
      await noteWithBlob(2, 600 * 1024);
      await noteWithBlob(3, 600 * 1024);
      serveWithLimit(MB);

      await syncService.syncNow();

      for (const n of [1, 2, 3]) {
        expect((await noteRepository.getById(ID(n)))?.needsSync).toBe(false);
      }
    });

    it('isolates the one note that can never fit, and syncs the others', async () => {
      await noteWithBlob(1, 16);
      await noteWithBlob(2, 2 * MB);
      await noteWithBlob(3, 16);
      serveWithLimit(MB);

      const result = await syncService.syncNow();

      expect((await noteRepository.getById(ID(1)))?.needsSync).toBe(false);
      expect((await noteRepository.getById(ID(3)))?.needsSync).toBe(false);
      expect((await noteRepository.getById(ID(2)))?.needsSync).toBe(true);
      expect((await syncRepository.getNoteSyncMetadata(ID(2)))?.lastSyncStatus).toBe('error');
      expect(pulls).toHaveLength(1);
      expect(result.success).toBe(true);
    });

    it('sends a blob once, not with every later edit of its note', async () => {
      await noteWithBlob(1, 16);
      serve(() => ({}), accept);
      await syncService.syncNow();
      expect(pushes[0].attachments).toHaveLength(1);

      pushes = [];
      const note = await noteRepository.getById(ID(1)) as Note;
      await noteRepository.update({ ...note, needsSync: true }, false, true);
      await syncService.syncNow();

      expect(pushes[0].attachments).toEqual([]);
    });

    it('does not send a blob back to the server it came from', async () => {
      const attachment = { id: ID(190), filename: 'f', mimeType: 'image/png', size: 3, data: ID(190) };
      const note = await remoteNote(1, { attachments: [attachment] });
      serve(() => ({ notes: [note], attachments: [{ id: ID(190), data: 'AQID' }] }), accept);
      await syncService.syncNow();

      const stored = await noteRepository.getById(ID(1)) as Note;
      await noteRepository.update({ ...stored, needsSync: true }, false, true);
      await syncService.syncNow();

      expect(pushes[0].notes).toHaveLength(1);
      expect(pushes[0].attachments).toEqual([]);
    });

    it('sends a blob again if the push it went with was not accepted', async () => {
      await noteWithBlob(1, 16);
      serve(() => ({}), () => ({ accepted: [] }));
      await syncService.syncNow();

      pushes = [];
      await syncService.syncNow();

      expect(pushes[0].attachments).toHaveLength(1);
    });

    it('re-uploads a blob the server says it lost, even after sending it once', async () => {
      await noteWithBlob(1, 16);
      serve(() => ({}), accept);
      await syncService.syncNow();

      pushes = [];
      const note = await noteRepository.getById(ID(1)) as Note;
      await noteRepository.update({ ...note, needsSync: true }, false, true);
      serve(() => ({}), body => (body.notes.length > 0
        ? { ...accept(body), attachmentWarnings: [{ noteId: ID(1), attachmentIds: [ID(101)] }] }
        : {}));
      await syncService.syncNow();

      expect(pushes[1].attachments.map(a => a.id)).toEqual([ID(101)]);
    });

    it('re-uploads lost blobs in pieces the server can take', async () => {
      await noteWithBlob(1, 3 * MB);
      await noteWithBlob(2, 3 * MB);
      serve(() => ({}), accept);
      await syncService.syncNow();

      pushes = [];
      for (const n of [1, 2]) {
        const note = await noteRepository.getById(ID(n)) as Note;
        await noteRepository.update({ ...note, needsSync: true }, false, true);
      }
      serve(() => ({}), body => (body.notes.length > 0
        ? { ...accept(body), attachmentWarnings: [
            { noteId: ID(1), attachmentIds: [ID(101)] }, { noteId: ID(2), attachmentIds: [ID(102)] },
          ] }
        : {}));
      await syncService.syncNow();

      const repairs = pushes.filter(p => p.notes.length === 0);
      expect(repairs).toHaveLength(2);
    });
  });

  describe('a long sync', () => {
    it('is not mistaken for a stuck one while pages keep arriving', async () => {
      // Two minutes with nothing happening means stuck. A big vault passes that
      // while working normally, and a second sync would start alongside it.
      await onCursor('c0');
      let now = Date.now();
      vi.spyOn(Date, 'now').mockImplementation(() => now);
      let overlapping: { success: boolean; error?: string } | undefined;

      server.use(http.get(`${ENDPOINT}/api/v1/sync/status`, () => HttpResponse.json({ serverTime: NOW })));
      serve((_, index) => {
        now += 90_000;
        return index < 2 ? { hasMore: true, nextCursor: `c${index + 1}` } : { nextCursor: 'done' };
      });
      const original = syncRepository.updateMetadata.bind(syncRepository);
      vi.spyOn(syncRepository, 'updateMetadata').mockImplementation(async update => {
        if (update.pullCursor === 'c2') {
          overlapping = await syncService.syncNow();
        }
        return original(update);
      });

      await syncService.syncNow();

      expect(overlapping?.error).toBe('Sync already in progress');
    });
  });

  describe('the inbox', () => {
    const item = { id: ID(50), content: 'x', tags: [], createdAt: NOW, sizeBytes: 1 };

    it('is empty when a page says nothing about it, on any page', async () => {
      inboxItems.set([item]);
      const note = await remoteNote(1);
      serve(() => ({ notes: [note] }));

      await syncService.syncNow();

      expect(get(inboxItems)).toEqual([]);
    });
  });
});
