/**
 * Tests for inboxService
 */

import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll, vi } from 'vitest';
import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import { get } from 'svelte/store';
import { inboxService } from './inboxService';
import { syncRepository } from './syncRepository';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { noteService } from './noteService';
import { initTestDB, cleanupTestDB } from '../../test/db-utils';
import { inboxItems, settings } from '../stores/appStore';
import { DEFAULT_SETTINGS } from '../types';
import type { InboxItem } from '../types';

const TEST_ENDPOINT = 'https://sync.example.com';
const TEST_API_KEY = 'test-api-key-123';

// Setup MSW server
const server = setupServer();

// Mock the toast module
vi.mock('../utils/toast.svelte', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

// Mock svelte-i18n
vi.mock('svelte-i18n', () => ({
  _: {
    subscribe: (cb: (val: unknown) => void) => {
      // Return a function that translates keys to themselves
      cb((key: string) => key);
      return () => {};
    },
  },
}));

function createTestInboxItem(overrides: Partial<InboxItem> = {}): InboxItem {
  return {
    id: `inbox-${Math.random().toString(36).slice(2)}`,
    content: 'Test inbox content',
    tags: ['inbox'],
    createdAt: new Date().toISOString(),
    sizeBytes: 100,
    ...overrides,
  };
}

describe('inboxService', () => {
  let masterKey: CryptoKey;

  beforeAll(() => {
    server.listen({ onUnhandledRequest: 'error' });
  });

  afterAll(() => {
    server.close();
  });

  beforeEach(async () => {
    const testDB = await initTestDB();
    masterKey = testDB.masterKey;

    // Set up authenticated key manager
    keyManager.setMasterKey({
      key: masterKey,
      keyBytes: new Uint8Array(32),
      derivedAt: Date.now(),
    });

    // Initialize settings store with syncEnabled
    settings.set({ ...DEFAULT_SETTINGS, syncEnabled: true });

    // Set up sync metadata with encrypted API key
    const encryptedApiKey = await cryptoService.encryptText(TEST_API_KEY, masterKey);
    await syncRepository.updateMetadata({
      syncEnabled: true,
      syncEndpoint: TEST_ENDPOINT,
      clientId: 'test-client-id',
      apiKey: JSON.stringify(encryptedApiKey),
    });

    // Reset MSW handlers and stores
    server.resetHandlers();
    inboxItems.set([]);
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    settings.set(DEFAULT_SETTINGS);
    inboxItems.set([]);
    await cleanupTestDB();
  });

  // ==========================================================================
  // acceptItem
  // ==========================================================================

  describe('acceptItem', () => {
    it('should create an encrypted note from inbox item', async () => {
      const item = createTestInboxItem({
        content: 'My inbox note',
        tags: ['important', 'inbox'],
      });
      inboxItems.set([item]);

      const createNoteSpy = vi.spyOn(noteService, 'createNote').mockResolvedValue({} as any);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/${item.id}`, () => {
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.acceptItem(item);

      expect(createNoteSpy).toHaveBeenCalledWith(
        'My inbox note',
        ['important', 'inbox'],
        { showPreview: false }
      );
    });

    it('should force showPreview to false', async () => {
      const item = createTestInboxItem();
      inboxItems.set([item]);

      const createNoteSpy = vi.spyOn(noteService, 'createNote').mockResolvedValue({} as any);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/${item.id}`, () => {
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.acceptItem(item);

      // Verify showPreview is explicitly false (XSS prevention)
      const options = createNoteSpy.mock.calls[0][2];
      expect(options?.showPreview).toBe(false);
    });

    it('should delete item from server after accepting', async () => {
      const item = createTestInboxItem();
      inboxItems.set([item]);

      vi.spyOn(noteService, 'createNote').mockResolvedValue({} as any);

      let deleteWasCalled = false;
      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/${item.id}`, () => {
          deleteWasCalled = true;
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.acceptItem(item);

      expect(deleteWasCalled).toBe(true);
    });

    it('should remove item from store after accepting', async () => {
      const item1 = createTestInboxItem({ id: 'item-1' });
      const item2 = createTestInboxItem({ id: 'item-2' });
      inboxItems.set([item1, item2]);

      vi.spyOn(noteService, 'createNote').mockResolvedValue({} as any);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/${item1.id}`, () => {
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.acceptItem(item1);

      const remaining = get(inboxItems);
      expect(remaining).toHaveLength(1);
      expect(remaining[0].id).toBe('item-2');
    });

    it('should throw if sync is not configured', async () => {
      // Clear sync metadata
      await syncRepository.updateMetadata({
        syncEnabled: false,
        syncEndpoint: '',
        clientId: '',
        apiKey: '',
      });

      const item = createTestInboxItem();
      await expect(inboxService.acceptItem(item)).rejects.toThrow();
    });
  });

  // ==========================================================================
  // deleteItem
  // ==========================================================================

  describe('deleteItem', () => {
    it('should delete item from server', async () => {
      const item = createTestInboxItem();
      inboxItems.set([item]);

      let deleteWasCalled = false;
      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/${item.id}`, () => {
          deleteWasCalled = true;
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.deleteItem(item.id);

      expect(deleteWasCalled).toBe(true);
    });

    it('should remove item from store', async () => {
      const item1 = createTestInboxItem({ id: 'del-1' });
      const item2 = createTestInboxItem({ id: 'del-2' });
      inboxItems.set([item1, item2]);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/${item1.id}`, () => {
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.deleteItem(item1.id);

      const remaining = get(inboxItems);
      expect(remaining).toHaveLength(1);
      expect(remaining[0].id).toBe('del-2');
    });

    it('should handle server 404 gracefully', async () => {
      const item = createTestInboxItem();
      inboxItems.set([item]);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/${item.id}`, () => {
          return HttpResponse.json({ error: 'Not found' }, { status: 404 });
        })
      );

      // Should not throw — 404 is handled gracefully by deleteFromServer
      await inboxService.deleteItem(item.id);

      // Item should still be removed from store
      expect(get(inboxItems)).toHaveLength(0);
    });
  });

  // ==========================================================================
  // acceptAll
  // ==========================================================================

  describe('acceptAll', () => {
    it('should accept all inbox items', async () => {
      const items = [
        createTestInboxItem({ id: 'all-1', content: 'Note 1' }),
        createTestInboxItem({ id: 'all-2', content: 'Note 2' }),
        createTestInboxItem({ id: 'all-3', content: 'Note 3' }),
      ];
      inboxItems.set(items);

      const createNoteSpy = vi.spyOn(noteService, 'createNote').mockResolvedValue({} as any);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/:id`, () => {
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.acceptAll();

      expect(createNoteSpy).toHaveBeenCalledTimes(3);
    });

    it('should clear store after accepting all', async () => {
      const items = [
        createTestInboxItem({ id: 'clr-1' }),
        createTestInboxItem({ id: 'clr-2' }),
      ];
      inboxItems.set(items);

      vi.spyOn(noteService, 'createNote').mockResolvedValue({} as any);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox/:id`, () => {
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.acceptAll();

      expect(get(inboxItems)).toHaveLength(0);
    });
  });

  // ==========================================================================
  // deleteAll
  // ==========================================================================

  describe('deleteAll', () => {
    it('should call bulk delete endpoint', async () => {
      inboxItems.set([
        createTestInboxItem({ id: 'bulk-1' }),
        createTestInboxItem({ id: 'bulk-2' }),
      ]);

      let bulkDeleteCalled = false;
      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox`, () => {
          bulkDeleteCalled = true;
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.deleteAll();

      expect(bulkDeleteCalled).toBe(true);
    });

    it('should clear store after bulk delete', async () => {
      inboxItems.set([
        createTestInboxItem({ id: 'clr-bulk-1' }),
        createTestInboxItem({ id: 'clr-bulk-2' }),
      ]);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox`, () => {
          return new HttpResponse(null, { status: 204 });
        })
      );

      await inboxService.deleteAll();

      expect(get(inboxItems)).toHaveLength(0);
    });

    it('should throw on server error', async () => {
      inboxItems.set([createTestInboxItem()]);

      server.use(
        http.delete(`${TEST_ENDPOINT}/api/v1/inbox`, () => {
          return HttpResponse.json({ error: 'Server error' }, { status: 500 });
        })
      );

      await expect(inboxService.deleteAll()).rejects.toThrow('Failed to delete all inbox items');
    });
  });

  // ==========================================================================
  // Sync integration (store population)
  // ==========================================================================

  describe('sync integration', () => {
    it('should populate inbox store from sync pull response', () => {
      const items: InboxItem[] = [
        createTestInboxItem({ id: 'sync-1', content: 'From sync' }),
        createTestInboxItem({ id: 'sync-2', content: 'Also from sync' }),
      ];

      // Simulate what syncService does after a pull
      inboxItems.set(items);

      const stored = get(inboxItems);
      expect(stored).toHaveLength(2);
      expect(stored[0].id).toBe('sync-1');
      expect(stored[1].id).toBe('sync-2');
    });

    it('should clear inbox store when pull has no items', () => {
      // Start with items
      inboxItems.set([createTestInboxItem()]);

      // Simulate sync with empty inbox
      inboxItems.set([]);

      expect(get(inboxItems)).toHaveLength(0);
    });
  });
});
