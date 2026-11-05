/**
 * The server is untrusted. These tests stand in for a hostile sync server: it
 * holds no key, so it can't touch the ciphertext, but it controls every plaintext
 * field around it. Each one runs the real syncNow path and checks that a bad
 * record is dropped, reported, and doesn't stop the rest of the sync.
 */
import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll } from 'vitest';
import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import { get } from 'svelte/store';
import { syncService } from './syncService';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { savedSearchRepository } from './savedSearchRepository';
import { versionRepository } from './versionRepository';
import { rejectedRemoteRecords, resetVaultHealth } from './vaultHealthService';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';
import { inboxItems, settings } from '../stores/appStore';
import { DEFAULT_SETTINGS } from '../types';
import type { SyncNote, SyncPullResponse, SyncPushResponse } from '../types';

const ENDPOINT = 'https://sync.example.com';
const GOOD_ID = 'a0000000-0000-4000-8000-000000000010';
const BAD_ID = 'a0000000-0000-4000-8000-000000000011';
const ATTACHMENT_ID = 'b0000000-0000-4000-8000-000000000010';
const NOW = '2026-11-05T01:02:03.000Z';
const HOSTILE = '"><img src=x onerror=alert(1)>';

const server = setupServer();

describe('sync boundary', () => {
  let masterKey: CryptoKey;

  beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
  afterAll(() => server.close());

  beforeEach(async () => {
    masterKey = (await initTestDB()).masterKey;
    keyManager.setMasterKey({ key: masterKey, keyBytes: new Uint8Array(32), derivedAt: Date.now() });
    settings.set({ ...DEFAULT_SETTINGS, syncEnabled: true });
    inboxItems.set([]);
    server.resetHandlers();
    resetVaultHealth();

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

  async function remoteNote(id: string, overrides: Record<string, unknown> = {}): Promise<SyncNote> {
    return {
      id,
      createdAt: NOW,
      modifiedAt: NOW,
      content: JSON.stringify(await cryptoService.encryptText('text', masterKey)),
      tags: [],
      attachments: [],
      pinned: false,
      archived: false,
      deleted: false,
      version: 1,
      ...overrides,
    } as SyncNote;
  }

  function serve(pull: Partial<SyncPullResponse>, push: Partial<SyncPushResponse> = {}) {
    server.use(
      http.get(`${ENDPOINT}/api/v1/sync/status`, () => HttpResponse.json({ serverTime: NOW, version: '1', pendingNotes: 0, totalNotes: 0 })),
      http.post(`${ENDPOINT}/api/v1/sync/push`, () => HttpResponse.json({ accepted: [], rejected: [], ...push })),
      http.post(`${ENDPOINT}/api/v1/sync/pull`, () => HttpResponse.json({ notes: [], attachments: [], versions: [], syncedAt: NOW, ...pull })),
    );
  }

  const rejected = () => get(rejectedRemoteRecords);

  describe('notes', () => {
    const hostileFields: Array<[string, Record<string, unknown>]> = [
      ['attachment data', { attachments: [{ id: ATTACHMENT_ID, filename: 'f', mimeType: 'image/png', size: 1, data: HOSTILE }] }],
      ['attachment id', { attachments: [{ id: HOSTILE, filename: 'f', mimeType: 'image/png', size: 1, data: ATTACHMENT_ID }] }],
      ['numeric modifiedAt', { modifiedAt: 1 }],
      ['numeric createdAt', { createdAt: 1 }],
      ['non-string tag', { tags: [1] }],
    ];

    for (const [name, overrides] of hostileFields) {
      it(`drops a note with a bad ${name} and keeps syncing`, async () => {
        serve({ notes: [await remoteNote(BAD_ID, overrides), await remoteNote(GOOD_ID)] });

        const result = await syncService.syncNow();

        expect(result.success).toBe(true);
        expect(await noteRepository.getById(BAD_ID)).toBeNull();
        expect(await noteRepository.getById(GOOD_ID)).not.toBeNull();
        expect(rejected()).toEqual([expect.objectContaining({ kind: 'note', id: BAD_ID })]);
      });
    }

    it('drops a note with a hostile id without keeping the id', async () => {
      serve({ notes: [await remoteNote(HOSTILE), await remoteNote(GOOD_ID)] });

      await syncService.syncNow();

      expect(await noteRepository.getById(GOOD_ID)).not.toBeNull();
      expect(rejected()).toEqual([expect.objectContaining({ kind: 'note', id: null })]);
    });

    it('leaves the local copy alone when an update is bad', async () => {
      const local = createTestNote({ tags: [], id: BAD_ID, content: 'local', contentHash: 'aaaaaaaaaaaaaaaaaaaaaaaa' });
      await noteRepository.create(local);

      serve({
        notes: [await remoteNote(BAD_ID, {
          contentHash: 'bbbbbbbbbbbbbbbbbbbbbbbb',
          parentHash: local.contentHash,
          attachments: [{ id: HOSTILE, filename: 'f', mimeType: 'x/y', size: 1, data: HOSTILE }],
        })],
      });

      await syncService.syncNow();

      expect((await noteRepository.getById(BAD_ID))?.content).toBe('local');
    });

    it('stores a clean copy of attachment references', async () => {
      const attachment = { id: ATTACHMENT_ID, filename: 'f', mimeType: HOSTILE, size: 1, data: ATTACHMENT_ID, extra: HOSTILE };
      serve({ notes: [await remoteNote(GOOD_ID, { attachments: [attachment] })] });

      await syncService.syncNow();

      const stored = await noteRepository.getById(GOOD_ID);
      expect(stored?.attachments).toEqual([
        { id: ATTACHMENT_ID, filename: 'f', mimeType: 'application/octet-stream', size: 1, data: ATTACHMENT_ID },
      ]);
    });
  });

  describe('showPreview stays with the device', () => {
    it('ignores the flag on a new note', async () => {
      serve({ notes: [await remoteNote(GOOD_ID, { showPreview: true })] });
      await syncService.syncNow();
      expect((await noteRepository.getById(GOOD_ID))?.showPreview).toBe(false);
    });

    it('does not let a pull turn preview on', async () => {
      const local = createTestNote({ tags: [], id: GOOD_ID, showPreview: false, contentHash: 'aaaaaaaaaaaaaaaaaaaaaaaa' });
      await noteRepository.create(local);

      serve({ notes: [await remoteNote(GOOD_ID, { showPreview: true, contentHash: 'bbbbbbbbbbbbbbbbbbbbbbbb', parentHash: local.contentHash })] });
      await syncService.syncNow();

      const stored = await noteRepository.getById(GOOD_ID);
      expect(stored?.contentHash).toBe('bbbbbbbbbbbbbbbbbbbbbbbb');
      expect(stored?.showPreview).toBe(false);
    });

    it('does not let a pull turn preview off', async () => {
      const local = createTestNote({ tags: [], id: GOOD_ID, showPreview: true, contentHash: 'aaaaaaaaaaaaaaaaaaaaaaaa' });
      await noteRepository.create(local);

      serve({ notes: [await remoteNote(GOOD_ID, { showPreview: false, contentHash: 'bbbbbbbbbbbbbbbbbbbbbbbb', parentHash: local.contentHash })] });
      await syncService.syncNow();

      expect((await noteRepository.getById(GOOD_ID))?.showPreview).toBe(true);
    });
  });

  describe('other records', () => {
    it('drops a version with hostile attachment metadata', async () => {
      const version = {
        versionKey: `${GOOD_ID}:1`, noteId: GOOD_ID, version: 1, createdAt: NOW, syncedAt: NOW,
        content: 'x', tags: [], reason: 'sync',
        attachments: [{ id: ATTACHMENT_ID, filename: 'f', mimeType: 'image/png', size: 1, data: HOSTILE }],
      };
      serve({ notes: [await remoteNote(GOOD_ID)], versions: [version] as SyncPullResponse['versions'] });

      await syncService.syncNow();

      expect(await versionRepository.getVersionsForNote(GOOD_ID)).toEqual([]);
      expect(rejected()).toEqual([expect.objectContaining({ kind: 'version' })]);
    });

    it('drops a saved search with a bad date', async () => {
      const search = { id: GOOD_ID, name: 'n', query: 'q', order: 0, createdAt: NOW, modifiedAt: 1, deleted: false, version: 1 };
      serve({ savedSearches: [search] as unknown as SyncPullResponse['savedSearches'] });

      await syncService.syncNow();

      expect(await savedSearchRepository.getById(GOOD_ID)).toBeUndefined();
      expect(rejected()).toEqual([expect.objectContaining({ kind: 'savedSearch' })]);
    });

    async function searchRow(overrides: Record<string, unknown> = {}) {
      const enc = async (text: string) => JSON.stringify(await cryptoService.encryptText(text, masterKey));
      return {
        id: GOOD_ID, name: await enc('Name'), query: await enc('query'), order: 0,
        createdAt: NOW, modifiedAt: NOW, deleted: false, version: 1, ...overrides,
      } as unknown as NonNullable<SyncPullResponse['savedSearches']>[number];
    }

    it('accepts a saved search this vault can read', async () => {
      serve({ savedSearches: [await searchRow()] });
      await syncService.syncNow();
      expect((await savedSearchRepository.getById(GOOD_ID))?.name).toBe('Name');
    });

    it('refuses a saved search the server wrote in plaintext', async () => {
      serve({ savedSearches: [await searchRow({ name: 'planted', query: 'planted' })] });

      const result = await syncService.syncNow();

      expect(result.success).toBe(true);
      expect(await savedSearchRepository.getById(GOOD_ID)).toBeUndefined();
      expect(rejected()).toEqual([expect.objectContaining({ kind: 'savedSearch', id: GOOD_ID })]);
    });

    it('still applies a deletion, whatever the row holds', async () => {
      await savedSearchRepository.create('Keep', 'keep');
      const existing = (await savedSearchRepository.getAll())[0];
      serve({ savedSearches: [await searchRow({ id: existing.id, name: 'x', query: 'x', deleted: true, modifiedAt: '2099-01-01T00:00:00.000Z' })] });

      await syncService.syncNow();

      expect(await savedSearchRepository.getAll()).toEqual([]);
    });

    it('does not stop the pull on a local row that will not decrypt', async () => {
      const { getDB, STORES } = await import('./db');
      await getDB().put(STORES.SAVED_SEARCHES, { id: GOOD_ID, name: 'x', query: 'x', order: 0, createdAt: NOW, modifiedAt: NOW, deleted: false, version: 1 });
      serve({ savedSearches: [await searchRow({ modifiedAt: '2099-01-01T00:00:00.000Z' })], notes: [await remoteNote(BAD_ID)] });

      const result = await syncService.syncNow();

      expect(result.success).toBe(true);
      expect((await savedSearchRepository.getById(GOOD_ID))?.name).toBe('Name');
    });

    it('does not store a blob under a hostile id', async () => {
      serve({ attachments: [{ id: HOSTILE, data: 'AAAA' }, { id: ATTACHMENT_ID, data: 'AAAA' }] });

      await syncService.syncNow();

      expect(await attachmentRepository.listAllIds()).toEqual([ATTACHMENT_ID]);
    });

    it('ignores a deletion with a hostile id', async () => {
      await noteRepository.create(createTestNote({ tags: [], id: GOOD_ID }));
      serve({ deletions: [{ id: HOSTILE, deletedAt: NOW }] });

      const result = await syncService.syncNow();

      expect(result.success).toBe(true);
      expect(await noteRepository.getById(GOOD_ID)).not.toBeNull();
      expect(rejected()).toEqual([expect.objectContaining({ kind: 'deletion', id: null })]);
    });

    it('filters inbox items', async () => {
      const good = { id: GOOD_ID, content: 'c', tags: [], createdAt: NOW, sizeBytes: 1 };
      const bad = { id: HOSTILE, content: 'c', tags: [], createdAt: NOW, sizeBytes: 1 };
      serve({ inboxItems: [bad, good] as SyncPullResponse['inboxItems'] });

      await syncService.syncNow();

      expect(get(inboxItems).map(item => item.id)).toEqual([GOOD_ID]);
      expect(rejected()).toEqual([expect.objectContaining({ kind: 'inboxItem' })]);
    });

    it('does not store a conflict with hostile attachment metadata', async () => {
      await noteRepository.create(createTestNote({ tags: [], id: GOOD_ID, needsSync: true }));
      serve({}, {
        rejected: [{
          id: GOOD_ID, reason: 'conflict', serverModifiedAt: NOW, serverContent: 'x', serverTags: [], serverVersion: 2,
          serverPinned: false, serverAttachments: [{ id: HOSTILE, filename: 'f', mimeType: 'x/y', size: 1, data: HOSTILE }],
        }],
      });

      await syncService.syncNow();

      expect((await syncRepository.getNoteSyncMetadata(GOOD_ID))?.conflictData).toBeUndefined();
      expect((await noteRepository.getById(GOOD_ID))?.needsSync).toBe(true);
      expect(rejected()).toEqual([expect.objectContaining({ kind: 'conflict', id: GOOD_ID })]);
    });
  });

  it('keeps at most a bounded number of reports', async () => {
    const notes = await Promise.all(Array.from({ length: 150 }, () => remoteNote(HOSTILE)));
    serve({ notes });

    await syncService.syncNow();

    expect(rejected().length).toBe(100);
  });
});
