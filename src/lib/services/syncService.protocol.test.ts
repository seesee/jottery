/**
 * docs/sync-protocol.md, as a client must follow it: the pull cursor, authoritative
 * attachment lists and refused deletions, against both a server that has them and
 * one that doesn't. Released servers stay in service, so neither is a special case.
 */
import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll } from 'vitest';
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
      serve((request, index) => {
        if (index === 0) return { nextCursor: 'c1' };
        return { nextCursor: 'c2' };
      });

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
