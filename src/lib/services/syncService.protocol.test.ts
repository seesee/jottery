/**
 * docs/sync-protocol.md, as a client must follow it: the pull cursor, authoritative
 * attachment lists and refused deletions, against both a server that has them and
 * one that doesn't. Released servers stay in service, so neither is a special case.
 */
import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll, vi } from 'vitest';

// The message key is what matters here, not its translation
vi.mock('svelte-i18n', async importOriginal => ({
  ...await importOriginal<object>(),
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
import { decryptTags } from './tagConversionService';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';
import { inboxItems, notes as appNotes, settings } from '../stores/appStore';
import { toast } from '../utils/toast.svelte';
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
      tags: [],
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
      await noteRepository.create(createTestNote({ tags: [], id: ID(n), content: await encrypted('x'), needsSync: false }));
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
      tags: [],
        id: ID(1), content: await encrypted('x'), needsSync: true,
        locked: true, lockedAt: NOW, contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')],
      }));
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes[0].notes[0].locked).toBe(true);
      expect(pushes[0].notes[0].lockedAt).toBe(NOW);
    });
  });

  // These move megabytes through real base64, which takes a while on a busy machine
  describe('pushes the server refuses', { timeout: 30_000 }, () => {
    const MB = 1024 * 1024;

    async function noteWithBlob(n: number, bytes: number, overrides: Partial<Note> = {}): Promise<void> {
      const { attachmentRepository } = await import('./attachmentRepository');
      await attachmentRepository.storeBlob(ID(100 + n), new Uint8Array(bytes).buffer);
      await noteRepository.create(createTestNote({
      tags: [],
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

  describe('being refused by the server', () => {
    function refuseEverything(status: number, headers: Record<string, string> = {}) {
      server.use(
        http.get(`${ENDPOINT}/api/v1/sync/status`, () => HttpResponse.json({ serverTime: NOW })),
        http.post(`${ENDPOINT}/api/v1/sync/pull`, () => new HttpResponse('refused', { status, headers })),
      );
    }

    afterEach(() => {
      (syncService as unknown as { retryNotBefore: number }).retryNotBefore = 0;
      syncService.cancelPendingBackgroundSync();
    });

    it('keeps the credentials on a 403, says why, and stops background sync', async () => {
      const error = vi.spyOn(toast, 'error');
      const stop = vi.spyOn(syncService, 'disableAutoSync');
      refuseEverything(403);
      const before = await metadata();

      await syncService.syncNow();

      expect((await metadata())?.apiKey).toBe(before?.apiKey);
      expect((await metadata())?.syncEnabled).toBe(true);
      expect(stop).toHaveBeenCalled();
      expect(error).toHaveBeenCalledWith('sync.accountDisabled');
    });

    it('waits as long as a 429 asks before trying again', async () => {
      refuseEverything(429, { 'Retry-After': '30' });
      await syncService.syncNow();

      expect((await syncService.syncNow()).error).toBe('Rate limited');

      vi.spyOn(Date, 'now').mockReturnValue(Date.now() + 31_000);
      expect((await syncService.syncNow()).error).not.toBe('Rate limited');
    });

    it('does not let a server stop it syncing for a year', async () => {
      refuseEverything(429, { 'Retry-After': String(365 * 24 * 3600) });
      await syncService.syncNow();

      vi.spyOn(Date, 'now').mockReturnValue(Date.now() + 3601_000);
      expect((await syncService.syncNow()).error).not.toBe('Rate limited');
    });
  });

  describe('a busy, slow or full server', () => {
    afterEach(() => {
      (syncService as unknown as { retryNotBefore: number }).retryNotBefore = 0;
      (syncService as unknown as { quotaNoticeAt: number }).quotaNoticeAt = 0;
      syncService.cancelPendingBackgroundSync();
    });

    async function dirtyNote(n: number): Promise<void> {
      await noteRepository.create(createTestNote({
      tags: [],
        id: ID(n), content: await encrypted('x'), needsSync: true,
        contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')],
      }));
    }

    /** `push` decides the status of each push: 200 accepts its notes */
    function serveStatuses(push: (body: SyncPushRequest) => number, headers: Record<string, string> = {}) {
      server.use(
        http.get(`${ENDPOINT}/api/v1/sync/status`, () => HttpResponse.json({ serverTime: NOW })),
        http.post(`${ENDPOINT}/api/v1/sync/push`, async ({ request }) => {
          const body = await request.json() as SyncPushRequest;
          pushes.push(body);
          const status = push(body);
          return status === 200
            ? HttpResponse.json({ accepted: body.notes.map(n => ({ id: n.id })), rejected: [] })
            : new HttpResponse('{"error":"no"}', { status, headers });
        }),
        http.post(`${ENDPOINT}/api/v1/sync/pull`, async ({ request }) => {
          pulls.push(await request.json() as SyncPullRequest);
          return HttpResponse.json({ notes: [], attachments: [], versions: [], syncedAt: NOW });
        }),
      );
    }

    describe('408', () => {
      it('tries a batch again in smaller pieces, which are likelier to finish', async () => {
        await dirtyNote(1);
        await dirtyNote(2);
        serveStatuses(body => (body.notes.length > 1 ? 408 : 200));

        const result = await syncService.syncNow();

        expect(result.success).toBe(true);
        expect((await noteRepository.getById(ID(1)))?.needsSync).toBe(false);
        expect((await noteRepository.getById(ID(2)))?.needsSync).toBe(false);
      });

      it('leaves a single note marked, without treating it as refused', async () => {
        await dirtyNote(1);
        serveStatuses(() => 408);

        const result = await syncService.syncNow();

        expect(result.success).toBe(false);
        expect((await noteRepository.getById(ID(1)))?.needsSync).toBe(true);
        expect((await syncRepository.getNoteSyncMetadata(ID(1)))?.lastSyncStatus).not.toBe('error');
        expect(pulls).toHaveLength(1);
      });
    });

    describe('503', () => {
      it('waits as long as it is told, then tries again by itself', async () => {
        // With live updates on, periodic sync is suspended, so nothing else would
        await dirtyNote(1);
        const schedule = vi.spyOn(window, 'setTimeout');
        serveStatuses(() => 503, { 'Retry-After': '30' });

        await syncService.syncNow();
        expect((await syncService.syncNow()).error).toBe('Rate limited');

        // Not the 30,000 of a request timeout: the wait, plus a small margin
        const retry = schedule.mock.calls.find(([, delay]) => (delay ?? 0) > 30_000 && (delay ?? 0) < 31_000);
        expect(retry).toBeDefined();

        vi.spyOn(Date, 'now').mockReturnValue(Date.now() + 31_000);
        serveStatuses(() => 200);
        (retry?.[0] as () => void)();

        await vi.waitFor(async () => expect((await noteRepository.getById(ID(1)))?.needsSync).toBe(false));
      });

      it('does not split a batch, which would only add to the load', async () => {
        await dirtyNote(1);
        await dirtyNote(2);
        serveStatuses(() => 503, { 'Retry-After': '30' });

        await syncService.syncNow();

        expect(pushes).toHaveLength(1);
      });
    });

    describe('507', () => {
      const quotaNotices = (spy: { mock: { calls: unknown[][] } }) =>
        spy.mock.calls.filter(([message]) => message === 'sync.storageQuotaExceeded');

      it('tells the user, keeps the note marked, and still pulls', async () => {
        const error = vi.spyOn(toast, 'error');
        await dirtyNote(1);
        serveStatuses(() => 507);

        const result = await syncService.syncNow();

        expect(result.success).toBe(false);
        expect(quotaNotices(error)).toHaveLength(1);
        expect((await noteRepository.getById(ID(1)))?.needsSync).toBe(true);
        expect(pulls).toHaveLength(1);
      });

      it('does not repeat itself on every background sync', async () => {
        const error = vi.spyOn(toast, 'error');
        await dirtyNote(1);
        serveStatuses(() => 507);

        await syncService.syncNow();
        await syncService.syncNow();

        expect(quotaNotices(error)).toHaveLength(1);
      });

      it('does not split a batch, because no piece would fit', async () => {
        await dirtyNote(1);
        await dirtyNote(2);
        serveStatuses(() => 507);

        await syncService.syncNow();

        expect(pushes).toHaveLength(1);
      });

      it('still sends deletions, so the user can make room', async () => {
        await noteRepository.create(createTestNote({ tags: [], id: ID(1), content: await encrypted('x'), needsSync: false }));
        await noteRepository.permanentDelete(ID(1));
        const { create } = await import('./savedSearchRepository');
        await create('a', 'b');
        serveStatuses(body => (body.savedSearches ? 507 : 200));

        await syncService.syncNow();

        expect(await noteRepository.getPendingDeletions()).toEqual([]);
      });
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

  describe('version history', () => {
    const NOTE = ID(70);

    async function noteWithVersion(tags: string[]): Promise<void> {
      await noteRepository.create(createTestNote({
      tags: [],
        id: NOTE, content: await encrypted('x'), needsSync: true,
        contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')],
      }));
      const { versionRepository } = await import('./versionRepository');
      await versionRepository.createVersion(
        { ...(await noteRepository.getById(NOTE) as Note), tags, version: 1 },
        { syncedAt: NOW, reason: 'manual-sync' },
      );
    }

    it('pushes tags in the sync format, as every reader understands', async () => {
      const { encryptJSON } = await import('./crypto');
      await noteWithVersion([JSON.stringify(await encryptJSON(['a', 'b'], masterKey))]);
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes[0].versions[0].tags).toHaveLength(2);
      expect(await decryptTags(pushes[0].versions[0].tags, masterKey)).toEqual(['a', 'b']);
    });

    it('still pushes the note if one version can\'t be read', async () => {
      await noteWithVersion(['not json']);
      serve(() => ({}));

      await syncService.syncNow();

      expect(pushes[0].notes).toHaveLength(1);
      expect(pushes[0].versions).toEqual([]);
    });

    it.each([
      ['one ciphertext per tag, from iOS or the TUI', async () => {
        const one = async (t: string) => JSON.stringify(await cryptoService.encryptText(JSON.stringify(t), masterKey));
        return [await one('a'), await one('b')];
      }],
      ['one blob, from web or Android', async () => {
        const { encryptJSON } = await import('./crypto');
        return [JSON.stringify(await encryptJSON(['a', 'b'], masterKey))];
      }],
    ])('reads tags stored as %s', async (_name, tags) => {
      const version = {
        versionKey: `${NOTE}:1`, noteId: NOTE, version: 1, createdAt: NOW, syncedAt: NOW,
        content: await encrypted('old'), tags: await tags(), attachments: [], reason: 'sync',
      };
      serve(() => ({ versions: [version] as never }));

      await syncService.syncNow();

      const { versionRepository } = await import('./versionRepository');
      const stored = await versionRepository.getVersion(NOTE, 1);
      expect(stored?.tags).toHaveLength(1);
      expect(await decryptTags(stored?.tags, masterKey)).toEqual(['a', 'b']);
    });
  });

  describe('a note whose tags cannot be read', () => {
    it('is set aside, and does not stop the rest of its batch', async () => {
      await noteRepository.create(createTestNote({
        id: ID(80), content: await encrypted('x'), tags: ['not json'], needsSync: true,
        contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')],
      }));
      await noteRepository.create(createTestNote({
      tags: [],
        id: ID(81), content: await encrypted('y'), needsSync: true,
        contentHash: HASH('b'), parentHash: null, hashChain: [HASH('b')],
      }));
      serve(() => ({}), body => ({ accepted: body.notes.map(n => ({ id: n.id })) }));

      await syncService.syncNow();

      expect(pushes[0].notes.map(n => n.id)).toEqual([ID(81)]);
      expect((await noteRepository.getById(ID(80)))?.needsSync).toBe(true);
      expect((await syncRepository.getNoteSyncMetadata(ID(80)))?.lastSyncStatus).toBe('error');
    });
  });

  describe('disconnecting', () => {
    function serveRevoke(status: number): { calls: string[] } {
      const seen = { calls: [] as string[] };
      server.use(http.delete(`${ENDPOINT}/api/v1/sync/device`, ({ request }) => {
        seen.calls.push(request.headers.get('Authorization') ?? '');
        return new HttpResponse(null, { status });
      }));
      return seen;
    }

    it('revokes the device while it still has the key to', async () => {
      const seen = serveRevoke(204);

      expect(await syncService.disconnect()).toEqual({ revoked: true });

      expect(seen.calls).toEqual(['Bearer key']);
      expect(await metadata()).toBeNull();
    });

    it('still disconnects if the server refuses, and says so', async () => {
      serveRevoke(500);

      expect(await syncService.disconnect()).toEqual({ revoked: false });

      expect(await metadata()).toBeNull();
    });

    it('still disconnects if the server cannot be reached', async () => {
      server.use(http.delete(`${ENDPOINT}/api/v1/sync/device`, () => HttpResponse.error()));

      expect(await syncService.disconnect()).toEqual({ revoked: false });

      expect(await metadata()).toBeNull();
    });
  });

  describe('a note permanently deleted on another device', () => {
    it('leaves nothing behind here: versions, sync state or blobs', async () => {
      const { versionRepository } = await import('./versionRepository');
      const { attachmentRepository } = await import('./attachmentRepository');
      const attachment = { id: ID(91), filename: 'f', mimeType: 'image/png', size: 1, data: ID(91) };
      await attachmentRepository.storeBlob(ID(91), new Uint8Array([1]).buffer);
      const note = createTestNote({ id: ID(90), tags: [], content: await encrypted('x'), needsSync: false, attachments: [attachment] });
      await noteRepository.create(note);
      await versionRepository.createVersion({ ...note, version: 1 }, { syncedAt: NOW, reason: 'manual-sync' });
      await syncRepository.updateNoteSyncMetadata(ID(90), { noteId: ID(90), lastSyncStatus: 'synced' });
      serve(() => ({ deletions: [{ id: ID(90), deletedAt: NOW }] }));

      await syncService.syncNow();

      expect(await noteRepository.getById(ID(90))).toBeFalsy();
      expect(await versionRepository.getVersionsForNote(ID(90))).toEqual([]);
      expect(await syncRepository.getNoteSyncMetadata(ID(90))).toBeNull();
      expect(await attachmentRepository.getBlob(ID(91))).toBeNull();
    });
  });

  describe('a pull that deletes a large part of the vault', () => {
    /** Nothing authenticates the deletions list, so this is all that bounds the damage */
    async function vaultOf(count: number, overrides: Partial<Note> = {}): Promise<void> {
      for (let n = 1; n <= count; n++) {
        await noteRepository.create(createTestNote({ id: ID(n), tags: [], content: await encrypted('x'), needsSync: false, ...overrides }));
      }
    }

    const deletionsOf = (count: number) => Array.from({ length: count }, (_, i) => ({ id: ID(i + 1), deletedAt: NOW }));

    it('moves them to the recycle bin, where they can be restored, and says so', async () => {
      const error = vi.spyOn(toast, 'error');
      await vaultOf(30);
      serve(() => ({ deletions: deletionsOf(30) }));

      await syncService.syncNow();

      const notes = await noteRepository.getAll();
      expect(notes).toHaveLength(30);
      expect(notes.every(note => note.deleted)).toBe(true);
      expect(error).toHaveBeenCalledWith('sync.massDeletionHeld');
    });

    it('marks them with this device\'s clock, so the server can\'t have them purged at once', async () => {
      await vaultOf(30);
      serve(() => ({ deletions: deletionsOf(30) }));

      await syncService.syncNow();

      const [note] = await noteRepository.getAll();
      expect(Date.now() - Date.parse(note.deletedAt as string)).toBeLessThan(60_000);
    });

    it('cannot be got round by asking again, which would delete them from the bin', async () => {
      await vaultOf(30);
      serve(() => ({ deletions: deletionsOf(30) }));

      await syncService.syncNow();
      await syncService.syncNow();

      expect(await noteRepository.getAll()).toHaveLength(30);
    });

    it('lets a note restored from the bin be deleted again in the ordinary way', async () => {
      await vaultOf(30);
      serve(() => ({ deletions: deletionsOf(30) }));
      await syncService.syncNow();
      await noteRepository.restore(ID(1));
      serve(() => ({ deletions: deletionsOf(1) }), () => ({ skippedDeleted: [] }));

      await syncService.syncNow();

      expect(await noteRepository.getById(ID(1))).toBeFalsy();
    });

    it('applies an ordinary deletion straight away', async () => {
      await vaultOf(30);
      serve(() => ({ deletions: deletionsOf(3) }));

      await syncService.syncNow();

      expect(await noteRepository.getAll()).toHaveLength(27);
    });

    it('does not mistake emptying a bin for one, because those notes are in the bin here too', async () => {
      await vaultOf(30, { deleted: true, deletedAt: NOW });
      serve(() => ({ deletions: deletionsOf(30) }));

      await syncService.syncNow();

      expect(await noteRepository.getAll()).toHaveLength(0);
    });

    it('counts across pages', async () => {
      await vaultOf(40);
      await onCursor('c0');
      serve((_, index) => (index < 3
        ? { hasMore: true, nextCursor: `c${index + 1}`, deletions: deletionsOf(40).slice(index * 8, index * 8 + 8) }
        : { nextCursor: 'done', deletions: deletionsOf(40).slice(24) }));

      await syncService.syncNow();

      expect((await noteRepository.getAll()).length).toBeGreaterThanOrEqual(16);
    });
  });

  describe('skippedDeleted', () => {
    async function dirtyNote(n: number, overrides: Partial<Note> = {}): Promise<void> {
      await noteRepository.create(createTestNote({
        tags: [], id: ID(n), content: await encrypted('x'), needsSync: true,
        contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')], ...overrides,
      }));
    }

    const live = async () => (await noteRepository.getAllNonDeleted()).map(note => note.id);

    it('drops a note deleted after this edit, even if no pull will say so', async () => {
      await dirtyNote(1);
      await onCursor('c1');
      serve(() => ({ nextCursor: 'c1' }), () => ({ skippedDeleted: [ID(1)] }));

      await syncService.syncNow();

      expect(await noteRepository.getById(ID(1))).toBeFalsy();
    });

    it('stops sending it', async () => {
      await dirtyNote(1);
      await onCursor('c1');
      serve(() => ({ nextCursor: 'c1' }), () => ({ skippedDeleted: [ID(1)] }));

      await syncService.syncNow();
      pushes = [];
      await syncService.syncNow();

      expect(pushes).toEqual([]);
    });

    it('keeps a note edited since it was sent, which is later than the deletion', async () => {
      await dirtyNote(1);
      await onCursor('c1');
      serve(() => ({ nextCursor: 'c1' }), () => ({ skippedDeleted: [ID(1)] }));
      const push = pushToServerThenEdit(ID(1));

      await syncService.syncNow();
      await push;

      const note = await noteRepository.getById(ID(1));
      expect(note?.needsSync).toBe(true);
      expect(note?.deleted).toBe(false);
    });

    /** Edits a note the moment the server has its copy, as a person typing during a sync would */
    function pushToServerThenEdit(id: string): Promise<void> {
      return new Promise(resolve => {
        const pull = async ({ request }: { request: Request }) => {
          const body = await request.json() as SyncPushRequest;
          pushes.push(body);
          const note = await noteRepository.getById(id);
          await noteRepository.update({ ...note!, modifiedAt: LATER, contentHash: HASH('z') }, false, true);
          resolve();
          return HttpResponse.json({ accepted: [], rejected: [], skippedDeleted: [id] });
        };
        server.use(http.post(`${ENDPOINT}/api/v1/sync/push`, pull));
      });
    }

    it('means nothing from an older server, which does not send it', async () => {
      await dirtyNote(1);
      serve(() => ({}), body => ({ accepted: body.notes.map(n => ({ id: n.id, serverVersion: 1, syncedAt: NOW })) }));

      await syncService.syncNow();

      expect(await live()).toEqual([ID(1)]);
    });

    it('ignores a note it did not send, and anything that is not an id', async () => {
      await dirtyNote(1);
      await dirtyNote(2, { needsSync: false, syncedAt: NOW });
      serve(() => ({}), () => ({ skippedDeleted: [ID(2), 'not an id' as never] }));

      await syncService.syncNow();

      expect((await live()).sort()).toEqual([ID(1), ID(2)]);
    });

    it('holds them in the bin past the limit, as a pull would, so a server cannot wipe a device this way', async () => {
      const ids = Array.from({ length: 12 }, (_, i) => i + 1);
      for (const n of ids) await dirtyNote(n);
      await onCursor('c1');
      const error = vi.spyOn(toast, 'error');
      serve(() => ({ nextCursor: 'c1' }), body => ({ skippedDeleted: body.notes.map(n => n.id) }));

      await syncService.syncNow();

      const notes = await noteRepository.getAll();
      expect(notes).toHaveLength(12);
      expect(notes.every(note => note.deleted && !note.needsSync)).toBe(true);
      expect(error).toHaveBeenCalledWith('sync.massDeletionHeld');
    });

    it('does not send a held note again, even one that never synced', async () => {
      const ids = Array.from({ length: 12 }, (_, i) => i + 1);
      for (const n of ids) await dirtyNote(n);
      await onCursor('c1');
      serve(() => ({ nextCursor: 'c1' }), body => ({ skippedDeleted: body.notes.map(n => n.id) }));

      await syncService.syncNow();
      pushes = [];
      await syncService.syncNow();

      expect(pushes).toEqual([]);
      expect(await noteRepository.getAll()).toHaveLength(12);
    });

    it('takes the note out of the list', async () => {
      await dirtyNote(1);
      await onCursor('c1');
      appNotes.set([{ id: ID(1) } as never]);
      serve(() => ({ nextCursor: 'c1' }), () => ({ skippedDeleted: [ID(1)] }));

      await syncService.syncNow();

      expect(get(appNotes)).toEqual([]);
    });
  });

  describe('a pull that only deletes', () => {
    it('takes the note out of the list', async () => {
      await noteRepository.create(createTestNote({ tags: [], id: ID(1), content: await encrypted('x'), needsSync: false }));
      await onCursor('c1');
      appNotes.set([{ id: ID(1) } as never]);
      serve(() => ({ nextCursor: 'c2', deletions: [{ id: ID(1), deletedAt: LATER }] }));

      await syncService.syncNow();

      expect(get(appNotes)).toEqual([]);
    });
  });

  describe('a clock over a day ahead of the server', () => {
    const DAY = 24 * 60 * 60 * 1000;
    const ahead = (ms: number) => new Date(Date.parse(NOW) + ms).toISOString();
    const clockNotices = (spy: { mock: { calls: unknown[][] } }) =>
      spy.mock.calls.filter(([message]) => message === 'sync.clockAhead');

    // The stub server's time is fixed, so the device's has to be too. A test note is
    // stamped with the current time, which a day after NOW looks a day ahead of the server.
    // Tests elsewhere can raise the notice too, and its rate limit would swallow the
    // next one, so it starts clear as well as ending that way.
    const clearClockNotice = () => {
      (syncService as unknown as { clockNoticeAt: number }).clockNoticeAt = 0;
    };

    beforeEach(() => {
      vi.useFakeTimers({ toFake: ['Date'], now: Date.parse(NOW) });
      clearClockNotice();
    });

    afterEach(() => {
      vi.useRealTimers();
      clearClockNotice();
    });

    async function noteDated(modifiedAt: string, overrides: Partial<Note> = {}): Promise<void> {
      await noteRepository.create(createTestNote({
        tags: [], id: ID(1), content: await encrypted('x'), needsSync: true, modifiedAt,
        contentHash: HASH('a'), parentHash: null, hashChain: [HASH('a')], ...overrides,
      }));
    }

    const skippedForItsTime = () => ({ errors: ['Note skipped: modifiedAt is more than 24 hours ahead'] });

    it('is reported when the server skips a note for it, and the note stays to send again', async () => {
      const error = vi.spyOn(toast, 'error');
      await noteDated(ahead(DAY + 60_000));
      serve(() => ({}), skippedForItsTime);

      const result = await syncService.syncNow();

      expect(result.success).toBe(true);
      expect(clockNotices(error)).toHaveLength(1);
      expect((await noteRepository.getById(ID(1)))?.needsSync).toBe(true);
    });

    it('is judged against the server, because the device is the one in doubt', async () => {
      const error = vi.spyOn(toast, 'error');
      // Long past by this machine's clock, and still a day ahead of this server's
      const serverTime = '2001-01-01T00:00:00.000Z';
      await noteDated('2001-01-03T00:00:00.000Z');
      serve(() => ({ syncedAt: serverTime }), skippedForItsTime);

      await syncService.syncNow();

      expect(clockNotices(error)).toHaveLength(1);
    });

    it('looks at every time the server checks', async () => {
      const error = vi.spyOn(toast, 'error');
      await noteDated(NOW, { createdAt: ahead(2 * DAY) });
      serve(() => ({}), skippedForItsTime);

      await syncService.syncNow();

      expect(clockNotices(error)).toHaveLength(1);
    });

    it('says nothing for a note within the day', async () => {
      const error = vi.spyOn(toast, 'error');
      await noteDated(ahead(DAY - 60_000));
      serve(() => ({}));

      await syncService.syncNow();

      expect(clockNotices(error)).toHaveLength(0);
    });

    it('says nothing if the server took the note, as an older one does', async () => {
      const error = vi.spyOn(toast, 'error');
      await noteDated(ahead(2 * DAY));
      serve(() => ({}), body => ({ accepted: body.notes.map(n => ({ id: n.id, serverVersion: 1, syncedAt: NOW })) }));

      await syncService.syncNow();

      expect(clockNotices(error)).toHaveLength(0);
    });

    it('says nothing for a note it was told is deleted', async () => {
      const error = vi.spyOn(toast, 'error');
      await noteDated(ahead(2 * DAY));
      serve(() => ({}), () => ({ skippedDeleted: [ID(1)] }));

      await syncService.syncNow();

      expect(clockNotices(error)).toHaveLength(0);
    });

    it('does not repeat itself on every sync', async () => {
      const error = vi.spyOn(toast, 'error');
      await noteDated(ahead(2 * DAY));
      serve(() => ({}), skippedForItsTime);

      await syncService.syncNow();
      await syncService.syncNow();

      expect(clockNotices(error)).toHaveLength(1);
    });

    describe('for a saved search', () => {
      async function searchDated(modifiedAt: string) {
        const { getDB, STORES } = await import('./db');
        await getDB().put(STORES.SAVED_SEARCHES, {
          id: ID(9), name: await encrypted('n'), query: await encrypted('q'), order: 0,
          createdAt: NOW, modifiedAt, deleted: false, version: 1, needsSync: true,
        });
      }

      it('is reported, and the search stays to send again', async () => {
        const { savedSearchRepository } = await import('./savedSearchRepository');
        const error = vi.spyOn(toast, 'error');
        await searchDated(ahead(2 * DAY));
        serve(() => ({}), () => ({ errors: ['Saved search skipped: modifiedAt is more than 24 hours ahead'] }));

        await syncService.syncNow();

        expect(clockNotices(error)).toHaveLength(1);
        expect(await savedSearchRepository.getAllNeedingSync()).toHaveLength(1);
      });

      it('is marked synced when the server skipped nothing', async () => {
        const { savedSearchRepository } = await import('./savedSearchRepository');
        await searchDated(NOW);
        serve(() => ({}));

        await syncService.syncNow();

        expect(await savedSearchRepository.getAllNeedingSync()).toHaveLength(0);
      });
    });
  });

  describe('live updates', () => {
    class FakeEventSource {
      static instances: FakeEventSource[] = [];
      static CONNECTING = 0;
      static OPEN = 1;
      static CLOSED = 2;

      readyState = FakeEventSource.CONNECTING;
      onopen: (() => void) | null = null;
      onerror: ((event: unknown) => void) | null = null;

      constructor(public url: string) {
        FakeEventSource.instances.push(this);
      }

      addEventListener() {}

      close() {
        this.readyState = FakeEventSource.CLOSED;
      }

      /** What a browser does when connecting gets anything but a 200 */
      refuse() {
        this.readyState = FakeEventSource.CLOSED;
        this.onerror?.({});
      }
    }

    let tokenRequests: number;
    let tokenStatus: number;

    beforeEach(() => {
      FakeEventSource.instances = [];
      tokenRequests = 0;
      tokenStatus = 200;
      vi.stubGlobal('EventSource', FakeEventSource);
      serve(() => ({}));
      server.use(http.get(`${ENDPOINT}/api/v1/sync/events/token`, () => {
        tokenRequests++;
        return tokenStatus === 200
          ? HttpResponse.json({ token: `token-${tokenRequests}`, expires_in: 300 })
          : HttpResponse.json({ error: 'no' }, { status: tokenStatus });
      }));
    });

    afterEach(() => {
      syncService.disconnectFromSyncEvents();
      syncService.disableAutoSync();
      vi.useRealTimers();
      vi.unstubAllGlobals();
    });

    const stream = () => FakeEventSource.instances.at(-1)!;
    // On real time: the token request can outlast any fixed number of event-loop turns
    const waitFor = (check: () => boolean) =>
      vi.waitFor(() => expect(check()).toBe(true), { timeout: 15_000, interval: 5 });

    it('asks for a new token when the server refuses the stream', async () => {
      await syncService.connectToSyncEvents();
      expect(stream().url).toContain('token=token-1');

      vi.useFakeTimers({ toFake: ['setTimeout'] });
      stream().refuse();
      vi.runOnlyPendingTimers();
      vi.useRealTimers();

      await waitFor(() => FakeEventSource.instances.length === 2);
      expect(stream().url).toContain('token=token-2');
    });

    it('stops, and says the device was removed, if the new token is refused with a 401', async () => {
      const error = vi.spyOn(toast, 'error');
      syncService.enableAutoSync(5);
      await syncService.connectToSyncEvents();
      tokenStatus = 401;

      vi.useFakeTimers({ toFake: ['setTimeout'] });
      stream().refuse();
      vi.runOnlyPendingTimers();
      vi.useRealTimers();

      await waitFor(() => tokenRequests === 2 && error.mock.calls.length > 0);
      expect(error).toHaveBeenCalledWith('sync.deviceRevoked');
      expect(syncService.isAutoSyncEnabled()).toBe(false);
      expect(FakeEventSource.instances).toHaveLength(1);
    });

    it('stops, and says the account is disabled, if it is refused with a 403', async () => {
      const error = vi.spyOn(toast, 'error');
      syncService.enableAutoSync(5);
      tokenStatus = 403;

      await syncService.connectToSyncEvents();

      expect(error).toHaveBeenCalledWith('sync.accountDisabled');
      expect(syncService.isAutoSyncEnabled()).toBe(false);
      expect(FakeEventSource.instances).toHaveLength(0);
    });

    it('falls back to periodic sync if the token cannot be had for any other reason', async () => {
      const error = vi.spyOn(toast, 'error');
      tokenStatus = 503;

      await syncService.connectToSyncEvents();

      expect(error).not.toHaveBeenCalled();
      expect(FakeEventSource.instances).toHaveLength(0);
      expect((syncService as unknown as { sseFailedOver: boolean }).sseFailedOver).toBe(true);
    });
  });

  describe('a version from the server', () => {
    async function pulledVersion(overrides: Record<string, unknown> = {}) {
      const { versionRepository } = await import('./versionRepository');
      const version = {
        versionKey: `${ID(1)}:3`, noteId: ID(1), version: 3, createdAt: NOW, syncedAt: NOW,
        content: await encrypted('old text'), tags: [], attachments: [], reason: 'sync',
        contentHash: HASH('d'), parentHash: HASH('e'), showPreview: true, color: 'red', ...overrides,
      };
      serve(() => ({ versions: [version as never] }));

      await syncService.syncNow();

      return versionRepository.getVersion(ID(1), 3);
    }

    it('keeps its hashes, or a later push would send it without them', async () => {
      const stored = await pulledVersion();

      expect(stored?.contentHash).toBe(HASH('d'));
      expect(stored?.parentHash).toBe(HASH('e'));
    });

    it('keeps its own date, not the time it arrived', async () => {
      expect((await pulledVersion())?.createdAt).toBe(NOW);
    });

    it('keeps its colour', async () => {
      expect((await pulledVersion())?.color).toBe('red');
    });

    it('takes no preview setting, which stays per device so that a server cannot turn one on', async () => {
      expect((await pulledVersion())?.showPreview).toBeUndefined();
    });
  });

  describe('an attachment that cannot be stored', () => {
    it('is reported, as the next pull will not send it again', async () => {
      const { attachmentRepository } = await import('./attachmentRepository');
      const { rejectedRemoteRecords } = await import('./vaultHealthService');
      rejectedRemoteRecords.set([]);
      vi.spyOn(attachmentRepository, 'storeBlob').mockRejectedValue(new Error('quota'));
      serve(() => ({ attachments: [{ id: ID(70), data: 'AAAA' }] }));

      await syncService.syncNow();

      expect(get(rejectedRemoteRecords).map(record => record.kind)).toContain('attachment');
    });
  });

  describe('the inbox', () => {
    const item = { id: ID(50), content: 'x', tags: [], createdAt: NOW, sizeBytes: 1 };

    describe('after a delete that failed', () => {
      let deletes: number;
      let deleteStatus: number;

      beforeEach(() => {
        deletes = 0;
        deleteStatus = 500;
      });

      const serveInbox = () => {
        serve(() => ({ inboxItems: [item] }));
        server.use(http.delete(`${ENDPOINT}/api/v1/inbox/${item.id}`, () => {
          deletes++;
          return new HttpResponse(null, { status: deleteStatus });
        }));
      };

      async function acceptWhileDeleteFails() {
        const { inboxService } = await import('./inboxService');
        inboxItems.set([item]);
        await inboxService.acceptItem(item);
      }

      it('does not offer the item again', async () => {
        serveInbox();
        await acceptWhileDeleteFails();

        await syncService.syncNow();

        expect(get(inboxItems)).toEqual([]);
      });

      it('tries the delete again on each sync, and stops once it works', async () => {
        serveInbox();
        await acceptWhileDeleteFails();
        expect(deletes).toBe(1);

        await syncService.syncNow();
        expect(deletes).toBe(2);

        deleteStatus = 204;
        await syncService.syncNow();
        expect(deletes).toBe(3);
        expect((await metadata())?.acceptedInboxIds ?? []).toEqual([]);

        await syncService.syncNow();
        expect(deletes).toBe(3);
      });

      it('counts an item already gone from the server as done', async () => {
        serveInbox();
        await acceptWhileDeleteFails();
        deleteStatus = 404;

        await syncService.syncNow();

        expect((await metadata())?.acceptedInboxIds ?? []).toEqual([]);
      });

      it('does not stop a sync', async () => {
        serveInbox();
        await acceptWhileDeleteFails();

        expect((await syncService.syncNow()).success).toBe(true);
      });
    });

    it('goes when the account is disconnected, as it is plaintext from that account', async () => {
      inboxItems.set([item]);

      await syncService.disconnect();

      expect(get(inboxItems)).toEqual([]);
    });

    it('is empty when a page says nothing about it, on any page', async () => {
      inboxItems.set([item]);
      const note = await remoteNote(1);
      serve(() => ({ notes: [note] }));

      await syncService.syncNow();

      expect(get(inboxItems)).toEqual([]);
    });
  });
});
