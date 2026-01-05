/**
 * Tests for syncService
 */

import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll } from 'vitest';
import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import { syncService } from './syncService';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { settingsRepository } from './settingsRepository';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { initTestDB, cleanupTestDB, createTestNote } from '../../test/db-utils';
import type { SyncPushResponse, SyncPullResponse, AuthRegisterResponse } from '../types';

const TEST_ENDPOINT = 'https://sync.example.com';
const TEST_API_KEY = 'test-api-key-123';
const TEST_CLIENT_ID = 'test-client-id';

// Setup MSW server
const server = setupServer();

describe('syncService', () => {
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
      derivedAt: Date.now(),
    });

    // Reset MSW handlers
    server.resetHandlers();
  });

  afterEach(async () => {
    keyManager.clearMasterKey();
    await cleanupTestDB();
  });

  describe('Registration', () => {
    it('should register a new client', async () => {
      const mockResponse: AuthRegisterResponse = {
        clientId: TEST_CLIENT_ID,
        apiKey: TEST_API_KEY,
      };

      server.use(
        http.post(`${TEST_ENDPOINT}/api/v1/auth/register`, () => {
          return HttpResponse.json(mockResponse);
        })
      );

      const result = await syncService.register(TEST_ENDPOINT, 'Test Device');

      expect(result.clientId).toBe(TEST_CLIENT_ID);
      expect(result.apiKey).toBe(TEST_API_KEY);

      // Verify sync metadata was saved
      const metadata = await syncRepository.getMetadata();
      expect(metadata?.clientId).toBe(TEST_CLIENT_ID);
      expect(metadata?.syncEnabled).toBe(true);
      expect(metadata?.syncEndpoint).toBe(TEST_ENDPOINT);
      expect(metadata?.apiKey).toBeDefined(); // Encrypted

      // Verify settings were updated
      const settings = await settingsRepository.get();
      expect(settings.syncEnabled).toBe(true);
      expect(settings.syncEndpoint).toBe(TEST_ENDPOINT);
    });

    it('should normalize endpoint URL by removing trailing slash', async () => {
      const mockResponse: AuthRegisterResponse = {
        clientId: TEST_CLIENT_ID,
        apiKey: TEST_API_KEY,
      };

      let capturedUrl = '';
      server.use(
        http.post('*', ({ request }) => {
          capturedUrl = request.url;
          return HttpResponse.json(mockResponse);
        })
      );

      await syncService.register(TEST_ENDPOINT + '/', 'Test Device');

      expect(capturedUrl).toBe(`${TEST_ENDPOINT}/api/v1/auth/register`);
    });

    it('should encrypt API key before storing', async () => {
      const mockResponse: AuthRegisterResponse = {
        clientId: TEST_CLIENT_ID,
        apiKey: TEST_API_KEY,
      };

      server.use(
        http.post(`${TEST_ENDPOINT}/api/v1/auth/register`, () => {
          return HttpResponse.json(mockResponse);
        })
      );

      await syncService.register(TEST_ENDPOINT, 'Test Device');

      const metadata = await syncRepository.getMetadata();
      expect(metadata?.apiKey).toBeDefined();

      // API key should be encrypted JSON
      const encryptedApiKey = JSON.parse(metadata!.apiKey!);
      expect(encryptedApiKey.ciphertext).toBeDefined();
      expect(encryptedApiKey.iv).toBeDefined();

      // Decrypt and verify
      const decrypted = await cryptoService.decryptText(encryptedApiKey, masterKey);
      expect(decrypted).toBe(TEST_API_KEY);
    });

    it('should throw error when registration fails', async () => {
      server.use(
        http.post(`${TEST_ENDPOINT}/api/v1/auth/register`, () => {
          return HttpResponse.json(
            { error: 'Device limit reached' },
            { status: 403 }
          );
        })
      );

      await expect(syncService.register(TEST_ENDPOINT, 'Test Device')).rejects.toThrow();
    });

    it('should throw error when app is locked', async () => {
      keyManager.clearMasterKey();

      server.use(
        http.post(`${TEST_ENDPOINT}/api/v1/auth/register`, () => {
          return HttpResponse.json({
            clientId: TEST_CLIENT_ID,
            apiKey: TEST_API_KEY,
          });
        })
      );

      await expect(syncService.register(TEST_ENDPOINT, 'Test Device')).rejects.toThrow('Application is locked');
    });
  });

  describe('Manual Configuration', () => {
    it('should configure sync with manual credentials', async () => {
      await syncService.configureCredentials(TEST_ENDPOINT, TEST_CLIENT_ID, TEST_API_KEY);

      const metadata = await syncRepository.getMetadata();
      expect(metadata?.clientId).toBe(TEST_CLIENT_ID);
      expect(metadata?.syncEnabled).toBe(true);
      expect(metadata?.apiKey).toBeDefined();

      // Decrypt and verify API key
      const encryptedApiKey = JSON.parse(metadata!.apiKey!);
      const decrypted = await cryptoService.decryptText(encryptedApiKey, masterKey);
      expect(decrypted).toBe(TEST_API_KEY);
    });

    it('should normalize endpoint URL in manual configuration', async () => {
      await syncService.configureCredentials(TEST_ENDPOINT + '/', TEST_CLIENT_ID, TEST_API_KEY);

      const metadata = await syncRepository.getMetadata();
      expect(metadata?.syncEndpoint).toBe(TEST_ENDPOINT);
    });

    it('should throw error when app is locked during manual config', async () => {
      keyManager.clearMasterKey();

      await expect(
        syncService.configureCredentials(TEST_ENDPOINT, TEST_CLIENT_ID, TEST_API_KEY)
      ).rejects.toThrow('Application is locked');
    });
  });

  // Sync Status is tested indirectly through syncNow() since getServerStatus is private

  describe('Push Operation', () => {
    beforeEach(async () => {
      // Set up sync configuration
      const encryptedApiKey = await cryptoService.encryptText(TEST_API_KEY, masterKey);
      await syncRepository.updateMetadata({
        apiKey: JSON.stringify(encryptedApiKey),
        clientId: TEST_CLIENT_ID,
        syncEnabled: true,
        syncEndpoint: TEST_ENDPOINT,
      });
    });

    it('should push modified notes to server', async () => {
      // Create and save test note
      const note = createTestNote({ content: 'Test content', tags: ['test'] });
      await noteRepository.create(note);

      let capturedRequest: any;
      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 0,
            totalNotes: 0,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, async ({ request }) => {
          capturedRequest = await request.json();
          return HttpResponse.json({
            accepted: [{ id: note.id }],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, () => {
          return HttpResponse.json({
            notes: [],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      await syncService.syncNow(true); // Force full sync

      expect(capturedRequest).toBeDefined();
      expect(capturedRequest.notes).toHaveLength(1);
      expect(capturedRequest.notes[0].id).toBe(note.id);
    });

    it.skip('should encrypt note content before push', async () => {
      // TODO: Complex test requiring store setup
      const note = createTestNote({ content: 'Secret content', tags: ['confidential'] });
      await noteRepository.create(note);

      let capturedRequest: any;
      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 0,
            totalNotes: 0,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, async ({ request }) => {
          capturedRequest = await request.json();
          return HttpResponse.json({
            accepted: [{ id: note.id }],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, () => {
          return HttpResponse.json({
            notes: [],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      await syncService.syncNow(true);

      // Content should be encrypted (not contain plain text)
      const pushedNote = capturedRequest.notes[0];
      expect(pushedNote.content).not.toContain('Secret');
      expect(pushedNote.content).toMatch(/^\{.*\}$/); // Should be JSON encrypted object
    });

    it('should include authorization header in push request', async () => {
      const note = createTestNote({ content: 'Test', tags: [] });
      await noteRepository.create(note);

      let capturedHeaders: Headers | undefined;
      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 0,
            totalNotes: 0,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, ({ request }) => {
          capturedHeaders = request.headers;
          return HttpResponse.json({
            accepted: [],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, () => {
          return HttpResponse.json({
            notes: [],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      await syncService.syncNow(true);

      expect(capturedHeaders?.get('Authorization')).toBe(`Bearer ${TEST_API_KEY}`);
    });

    it.skip('should handle push when no notes need syncing', async () => {
      // TODO: Complex test requiring store setup
      let pushCalled = false;

      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 0,
            totalNotes: 0,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, () => {
          pushCalled = true;
          return HttpResponse.json({
            accepted: [],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, () => {
          return HttpResponse.json({
            notes: [],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      await syncService.syncNow();

      // Push should still be called even with no notes (to check for deletions)
      expect(pushCalled).toBe(true);
    });
  });

  describe('Pull Operation', () => {
    beforeEach(async () => {
      const encryptedApiKey = await cryptoService.encryptText(TEST_API_KEY, masterKey);
      await syncRepository.updateMetadata({
        apiKey: JSON.stringify(encryptedApiKey),
        clientId: TEST_CLIENT_ID,
        syncEnabled: true,
        syncEndpoint: TEST_ENDPOINT,
      });
    });

    it.skip('should pull and decrypt remote notes', async () => {
      // TODO: Complex test requiring store setup
      // Create encrypted note data as server would send it
      const remoteContent = await cryptoService.encryptText('Remote content', masterKey);
      const remoteTag = await cryptoService.encryptText('remote', masterKey);

      const remoteNote = {
        id: 'remote-note-id',
        createdAt: new Date().toISOString(),
        modifiedAt: new Date().toISOString(),
        content: JSON.stringify(remoteContent),
        tags: [JSON.stringify(remoteTag)],
        attachments: [],
        pinned: false,
        deleted: false,
        version: 1,
      };

      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 0,
            totalNotes: 1,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, () => {
          return HttpResponse.json({
            accepted: [],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, () => {
          return HttpResponse.json({
            notes: [remoteNote],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      await syncService.syncNow();

      // Verify note was decrypted and stored
      const note = await noteRepository.getById('remote-note-id');
      expect(note).toBeDefined();
      expect(note!.content).toBe('Remote content');
      expect(note!.tags).toContain('remote');
    });

    it('should include authorization header in pull request', async () => {
      let capturedHeaders: Headers | undefined;

      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 0,
            totalNotes: 0,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, () => {
          return HttpResponse.json({
            accepted: [],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, ({ request }) => {
          capturedHeaders = request.headers;
          return HttpResponse.json({
            notes: [],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      await syncService.syncNow();

      expect(capturedHeaders?.get('Authorization')).toBe(`Bearer ${TEST_API_KEY}`);
    });
  });

  describe('Error Handling', () => {
    beforeEach(async () => {
      const encryptedApiKey = await cryptoService.encryptText(TEST_API_KEY, masterKey);
      await syncRepository.updateMetadata({
        apiKey: JSON.stringify(encryptedApiKey),
        clientId: TEST_CLIENT_ID,
        syncEnabled: true,
        syncEndpoint: TEST_ENDPOINT,
      });
    });

    it('should handle network errors gracefully', async () => {
      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.error();
        })
      );

      const result = await syncService.syncNow();

      expect(result.success).toBe(false);
      expect(result.error).toBeDefined();
    });

    it('should handle server errors gracefully', async () => {
      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json(
            { error: 'Internal server error' },
            { status: 500 }
          );
        })
      );

      const result = await syncService.syncNow();

      expect(result.success).toBe(false);
      expect(result.error).toBeDefined();
    });

    it('should handle unauthorized errors', async () => {
      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json(
            { error: 'Unauthorized' },
            { status: 401 }
          );
        })
      );

      const result = await syncService.syncNow();

      expect(result.success).toBe(false);
      expect(result.error).toBeDefined();
    });

    it.skip('should prevent concurrent sync operations', async () => {
      // TODO: Complex async test requiring careful timing
      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, async () => {
          // Delay to simulate slow network
          await new Promise(resolve => setTimeout(resolve, 100));
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 0,
            totalNotes: 0,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, () => {
          return HttpResponse.json({
            accepted: [],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, () => {
          return HttpResponse.json({
            notes: [],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      // Start first sync
      const sync1 = syncService.syncNow();
      // Try to start second sync immediately
      const sync2 = syncService.syncNow();

      const [result1, result2] = await Promise.all([sync1, sync2]);

      // One should succeed, one should fail with "already in progress"
      const results = [result1, result2];
      const successCount = results.filter(r => r.success).length;
      const failureCount = results.filter(r => !r.success && r.error === 'Sync already in progress').length;

      expect(successCount).toBe(1);
      expect(failureCount).toBe(1);
    });

    it('should fail when sync not configured', async () => {
      // Clear sync metadata
      await syncRepository.clearAll();

      const result = await syncService.syncNow();

      expect(result.success).toBe(false);
      expect(result.error).toContain('Sync not configured');
    });

    it('should fail when app is locked', async () => {
      keyManager.clearMasterKey();

      const result = await syncService.syncNow();

      expect(result.success).toBe(false);
      expect(result.error).toContain('Application is locked');
    });
  });

  describe('Full Sync Workflow', () => {
    beforeEach(async () => {
      const encryptedApiKey = await cryptoService.encryptText(TEST_API_KEY, masterKey);
      await syncRepository.updateMetadata({
        apiKey: JSON.stringify(encryptedApiKey),
        clientId: TEST_CLIENT_ID,
        syncEnabled: true,
        syncEndpoint: TEST_ENDPOINT,
      });
    });

    it.skip('should complete full bidirectional sync', async () => {
      // TODO: Complex test requiring store and search service setup
      // Create and save local note
      const localNote = createTestNote({ content: 'Local note', tags: ['local'] });
      await noteRepository.create(localNote, masterKey);

      // Create remote note
      const remoteContent = await cryptoService.encryptText('Remote note', masterKey);
      const remoteTag = await cryptoService.encryptText('remote', masterKey);
      const remoteNote = {
        id: 'remote-note-id',
        createdAt: new Date().toISOString(),
        modifiedAt: new Date().toISOString(),
        content: JSON.stringify(remoteContent),
        tags: [JSON.stringify(remoteTag)],
        attachments: [],
        pinned: false,
        deleted: false,
        version: 1,
      };

      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 1,
            totalNotes: 2,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, () => {
          return HttpResponse.json({
            accepted: [{ id: localNote.id }],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, () => {
          return HttpResponse.json({
            notes: [remoteNote],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      const result = await syncService.syncNow(true);

      expect(result.success).toBe(true);

      // Verify both notes exist locally
      const local = await noteRepository.getById(localNote.id);
      const remote = await noteRepository.getById('remote-note-id');

      expect(local).toBeDefined();
      expect(remote).toBeDefined();
      expect(remote!.content).toBe('Remote note');

      // Verify last sync timestamp was updated
      const metadata = await syncRepository.getMetadata();
      expect(metadata?.lastSyncAt).toBeDefined();
    });

    it.skip('should update sync timestamp after successful sync', async () => {
      // TODO: Complex test requiring store setup
      server.use(
        http.get(`${TEST_ENDPOINT}/api/v1/sync/status`, () => {
          return HttpResponse.json({
            serverTime: new Date().toISOString(),
            version: '1.0.0',
            pendingNotes: 0,
            totalNotes: 0,
          });
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/push`, () => {
          return HttpResponse.json({
            accepted: [],
            rejected: [],
          } as SyncPushResponse);
        }),
        http.post(`${TEST_ENDPOINT}/api/v1/sync/pull`, () => {
          return HttpResponse.json({
            notes: [],
            attachments: [],
            versions: [],
          } as SyncPullResponse);
        })
      );

      const beforeSync = Date.now();
      await syncService.syncNow();
      const afterSync = Date.now();

      const metadata = await syncRepository.getMetadata();
      expect(metadata?.lastSyncAt).toBeDefined();

      const lastSyncTime = new Date(metadata!.lastSyncAt!).getTime();
      expect(lastSyncTime).toBeGreaterThanOrEqual(beforeSync);
      expect(lastSyncTime).toBeLessThanOrEqual(afterSync);
    });
  });
});
