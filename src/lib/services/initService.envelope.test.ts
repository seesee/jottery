/**
 * Tests for the server-wrapped master key flow between devices.
 *
 * The first device uploads a copy of the master key wrapped with a key derived
 * from a password and the user ID. A second device downloads it and unwraps it.
 * The server receives the account (server) password in plaintext at login, so
 * the blob must be wrapped with the notes password, which never leaves the client.
 * Older clients wrapped it with the server password. That password is never tried
 * when adopting a key, because the server could use it to plant one it chose.
 * Those accounts are repaired from a device that already holds the real key.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// The message key is what matters here, not its translation
vi.mock('svelte-i18n', () => ({
  _: { subscribe: (run: (format: (key: string) => string) => void) => { run((key: string) => key); return () => undefined; } },
}));

import {
  deleteAllLocalData,
  initialize,
  onboardFromServer,
  tryMigrateToEnvelope,
  uploadWrappedKeyToServer,
  verifyUnlockPassword,
  ensureServerWrappedKey,
  changePassword,
  isLocked,
  lock,
  unlock,
} from './initService';
import { AuthApiError, authService } from './authService';
import { cryptoService } from './crypto';
import { keyManager } from './keyManager';
import { encryptionRepository } from './encryptionRepository';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { savedSearchRepository } from './savedSearchRepository';
import { settingsRepository } from './settingsRepository';
import { passwordStorageService } from './passwordStorageService';
import { sessionStorageService } from './sessionStorageService';
import { toast } from '../utils/toast.svelte';
import { closeDB, deleteDB, getDB, initDB, STORES } from './db';
import { createTestNote } from '../../test/db-utils';
import { AccountKeyExistsError, AccountKeyUnreadableError, LocalNotesOrphanedError, ValidationError } from '../errors';
import { arrayBufferToBase64, base64ToUint8Array } from '../utils/base64';
import { CRYPTO_PBKDF2_ITERATIONS } from '../constants';
import type { PutWrappedKeyRequest, WrappedKeyResponse, WrappedKeyWrite } from './authService';

const ENDPOINT = 'https://sync.example.test';
const USER_ID = 'b3c1f5a2-8f0e-4c55-9d52-2f6a1c0e7a11';
const API_KEY = 'plaintext-device-api-key';
const NOTES_PASSWORD = 'notes-password-correct-horse';
const SERVER_PASSWORD = 'server-password-battery-staple';

/** What the fake server currently holds for the user */
let serverBlob: WrappedKeyResponse | null;
let putSpy: ReturnType<typeof vi.spyOn>;
/** How each write asked to be made */
let writes: Array<WrappedKeyWrite | undefined>;
/** Whether the fake server is an older one, with no updatedAt and no conditions */
let olderServer: boolean;
/** Runs between a client's GET and its PUT, for racing another device */
let beforeNextWrite: (() => Promise<void>) | null;
let writeCount: number;

function stamp(): string {
  writeCount += 1;
  return `2026-11-05T03:00:0${writeCount}.000Z`;
}

function bytesOf(): Uint8Array {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) throw new Error('test setup: no master key in memory');
  return masterKey.keyBytes;
}

/** Create a fresh envelope vault on "device A" and enable sync metadata on it */
async function setUpFirstDevice(): Promise<Uint8Array> {
  await initialize(NOTES_PASSWORD);
  const masterKey = keyManager.getMasterKey()!;
  const encryptedApiKey = await cryptoService.encryptText(API_KEY, masterKey.key);
  await syncRepository.updateMetadata({
    syncEnabled: true,
    syncEndpoint: ENDPOINT,
    apiKey: JSON.stringify(encryptedApiKey),
    clientId: 'client-a',
    userId: USER_ID,
    userEmail: 'user@example.test',
  });
  return new Uint8Array(masterKey.keyBytes);
}

/** Put a blob on the fake server, wrapped however the test needs */
async function wrapOnServer(password: string, keyBytes: Uint8Array): Promise<void> {
  const wrappingKey = await cryptoService.deriveWrappingKey(password, USER_ID);
  const wrapped = await cryptoService.wrapMasterKey(wrappingKey, keyBytes);
  serverBlob = {
    blob: JSON.stringify(wrapped),
    kdfVersion: 1,
    kdfIterations: 1_000_000,
    updatedAt: olderServer ? undefined : stamp(),
  };
}

/** Record that this device has completed a sync, so it must hold the account's key */
async function markSynced(): Promise<void> {
  await syncRepository.updateMetadata({ lastSyncAt: '2026-11-05T01:02:03.000Z' });
}

/** Store a note on this device, encrypted with its current key */
async function addLocalNote(text: string): Promise<string> {
  const key = keyManager.getMasterKey()!.key;
  const note = createTestNote({ content: JSON.stringify(await cryptoService.encryptText(text, key)) });
  await noteRepository.create(note);
  return note.id;
}

/** Start from an empty database, as on a brand new device */
async function freshDatabase(): Promise<void> {
  keyManager.clearMasterKey();
  closeDB();
  await deleteDB();
  await initDB();
}

/** Wipe local state so the next call behaves like a brand new device */
async function becomeSecondDevice(): Promise<void> {
  await freshDatabase();
}

describe('server-wrapped master key', () => {
  beforeEach(async () => {
    await freshDatabase();
    serverBlob = null;
    writes = [];
    olderServer = false;
    beforeNextWrite = null;
    writeCount = 0;

    // The contract in docs/sync-protocol.md, so that races are real here
    putSpy = vi
      .spyOn(authService, 'putWrappedKey')
      .mockImplementation(async (_endpoint: string, _token: string, request: PutWrappedKeyRequest, write?: WrappedKeyWrite) => {
        writes.push(write);
        const racing = beforeNextWrite;
        beforeNextWrite = null;
        await racing?.();

        if (!olderServer && write) {
          if ('ifMatch' in write && serverBlob?.updatedAt !== write.ifMatch) {
            throw new AuthApiError('changed', 409);
          }
          if ('replace' in write && !write.replace && serverBlob) {
            throw new AuthApiError('exists', 409);
          }
        }

        serverBlob = {
          blob: request.blob,
          kdfVersion: request.kdfVersion,
          kdfIterations: request.kdfIterations,
          updatedAt: olderServer ? undefined : stamp(),
        };
      });
    vi.spyOn(authService, 'getWrappedKey').mockImplementation(async () => serverBlob);
  });

  afterEach(async () => {
    vi.restoreAllMocks();
    keyManager.clearMasterKey();
    closeDB();
    await deleteDB();
  });

  describe('first device upload', () => {
    it('lets a second device unwrap the key with the notes password', async () => {
      const firstDeviceKey = await setUpFirstDevice();

      await uploadWrappedKeyToServer(NOTES_PASSWORD);
      await becomeSecondDevice();

      // The second device has its own random key until onboarding replaces it
      await initialize(NOTES_PASSWORD);
      await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);

      expect(Array.from(bytesOf())).toEqual(Array.from(firstDeviceKey));
    }, 60_000);

    it('does not let the server password unwrap the uploaded key', async () => {
      await setUpFirstDevice();
      await uploadWrappedKeyToServer(NOTES_PASSWORD);
      await becomeSecondDevice();
      await initialize(NOTES_PASSWORD);

      await expect(
        onboardFromServer(SERVER_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD)
      ).rejects.toThrow(/unwrap/i);
    }, 60_000);

    it('leaves the local unlock password untouched', async () => {
      await setUpFirstDevice();
      const before = await encryptionRepository.getMetadata();

      await uploadWrappedKeyToServer(NOTES_PASSWORD);

      expect(await encryptionRepository.getMetadata()).toEqual(before);
    }, 60_000);

    it('rejects a password that does not unlock this vault, and uploads nothing', async () => {
      await setUpFirstDevice();

      await expect(uploadWrappedKeyToServer(SERVER_PASSWORD)).rejects.toThrow(/incorrect password/i);

      expect(putSpy).not.toHaveBeenCalled();
      expect(serverBlob).toBeNull();
    }, 60_000);

    it('refuses to replace a key the account already has', async () => {
      await setUpFirstDevice();
      await wrapOnServer(NOTES_PASSWORD, crypto.getRandomValues(new Uint8Array(32)));
      const before = serverBlob;

      await expect(uploadWrappedKeyToServer(NOTES_PASSWORD)).rejects.toBeInstanceOf(AccountKeyExistsError);

      expect(putSpy).not.toHaveBeenCalled();
      expect(serverBlob).toBe(before);
    }, 60_000);

    it('reports upload failures instead of swallowing them', async () => {
      await setUpFirstDevice();
      putSpy.mockRejectedValueOnce(new Error('Failed to store wrapped key: 500'));

      await expect(uploadWrappedKeyToServer(NOTES_PASSWORD)).rejects.toThrow(/500/);
    }, 60_000);
  });

  describe('second device download', () => {
    it('refuses a key wrapped with the account password', async () => {
      const firstDeviceKey = await setUpFirstDevice();
      await wrapOnServer(SERVER_PASSWORD, firstDeviceKey);
      await becomeSecondDevice();
      await initialize(NOTES_PASSWORD);
      const before = await encryptionRepository.getMetadata();
      const keyBefore = Array.from(bytesOf());

      await expect(
        onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD)
      ).rejects.toBeInstanceOf(AccountKeyUnreadableError);

      expect(await encryptionRepository.getMetadata()).toEqual(before);
      expect(Array.from(bytesOf())).toEqual(keyBefore);
    }, 60_000);

    it('cannot be made to adopt a key the server planted', async () => {
      // The server sees the account password at register-device, so it can wrap a
      // key of its own under it. Nothing may try that password when adopting a key,
      // however a caller passes it.
      const plantedKey = crypto.getRandomValues(new Uint8Array(32));
      await wrapOnServer(SERVER_PASSWORD, plantedKey);
      await initialize(NOTES_PASSWORD);
      const keyBefore = Array.from(bytesOf());
      const adopt = onboardFromServer as unknown as (...args: unknown[]) => Promise<void>;

      await expect(
        adopt(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD, SERVER_PASSWORD)
      ).rejects.toBeInstanceOf(AccountKeyUnreadableError);

      expect(Array.from(bytesOf())).toEqual(keyBefore);
      expect(Array.from(bytesOf())).not.toEqual(Array.from(plantedKey));
    }, 60_000);

    it('keeps the notes password as the local unlock password', async () => {
      const firstDeviceKey = await setUpFirstDevice();
      await uploadWrappedKeyToServer(NOTES_PASSWORD);
      await becomeSecondDevice();
      await initialize(NOTES_PASSWORD);

      await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);

      // The local blob must unwrap with the notes password, not the server password
      const metadata = await encryptionRepository.getMetadata();
      expect(metadata?.envelopeVersion).toBe(1);
      const deviceKey = await cryptoService.deriveKey({
        password: NOTES_PASSWORD,
        salt: base64ToUint8Array(metadata!.deviceSalt!),
        iterations: CRYPTO_PBKDF2_ITERATIONS,
        extractable: false,
      });
      const local = JSON.parse(metadata!.localWrappedMaster!);
      const unwrapped = await cryptoService.unwrapMasterKey(deviceKey, local);
      expect(Array.from(unwrapped)).toEqual(Array.from(firstDeviceKey));
    }, 60_000);

    it('fails with a clear error and changes nothing when the password is wrong', async () => {
      await setUpFirstDevice();
      await uploadWrappedKeyToServer(NOTES_PASSWORD);
      await becomeSecondDevice();
      await initialize(NOTES_PASSWORD);
      const before = await encryptionRepository.getMetadata();
      const keyBefore = Array.from(bytesOf());

      await expect(
        onboardFromServer('wrong-notes-password', ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD)
      ).rejects.toBeInstanceOf(AccountKeyUnreadableError);

      expect(await encryptionRepository.getMetadata()).toEqual(before);
      expect(Array.from(bytesOf())).toEqual(keyBefore);
    }, 60_000);

    it('reports a missing server key distinctly from a wrong password', async () => {
      await initialize(NOTES_PASSWORD);

      await expect(
        onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD)
      ).rejects.toThrow(/no wrapped key found/i);
    }, 60_000);
  });

  describe('connecting a device that already has notes', () => {
    it('refuses when the account key would orphan local notes, and changes nothing', async () => {
      await wrapOnServer(NOTES_PASSWORD, crypto.getRandomValues(new Uint8Array(32)));
      await initialize(NOTES_PASSWORD);
      const noteId = await addLocalNote('only copy');
      const before = await encryptionRepository.getMetadata();
      const keyBefore = Array.from(bytesOf());

      await expect(
        onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD)
      ).rejects.toBeInstanceOf(LocalNotesOrphanedError);

      expect(await encryptionRepository.getMetadata()).toEqual(before);
      expect(Array.from(bytesOf())).toEqual(keyBefore);

      const stored = await noteRepository.getById(noteId);
      const key = keyManager.getMasterKey()!.key;
      expect(await cryptoService.decryptText(JSON.parse(stored!.content), key)).toBe('only copy');
    }, 60_000);

    it('counts notes in the recycle bin too', async () => {
      await wrapOnServer(NOTES_PASSWORD, crypto.getRandomValues(new Uint8Array(32)));
      await initialize(NOTES_PASSWORD);
      const noteId = await addLocalNote('binned');
      const stored = await noteRepository.getById(noteId);
      await noteRepository.update({ ...stored!, deleted: true }, false, true);

      await expect(
        onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD)
      ).rejects.toBeInstanceOf(LocalNotesOrphanedError);
    }, 60_000);

    it('connects when local notes already use the account key', async () => {
      const key = await setUpFirstDevice();
      await addLocalNote('shared');
      await wrapOnServer(NOTES_PASSWORD, key);

      await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);

      expect(Array.from(bytesOf())).toEqual(Array.from(key));
    }, 60_000);

    it('connects when the local vault is empty', async () => {
      const accountKey = crypto.getRandomValues(new Uint8Array(32));
      await wrapOnServer(NOTES_PASSWORD, accountKey);
      await initialize(NOTES_PASSWORD);

      await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);

      expect(Array.from(bytesOf())).toEqual(Array.from(accountKey));
    }, 60_000);

    it('is not blocked by a note that no key can open', async () => {
      const accountKey = crypto.getRandomValues(new Uint8Array(32));
      await wrapOnServer(NOTES_PASSWORD, accountKey);
      await initialize(NOTES_PASSWORD);
      await noteRepository.create(createTestNote({ content: 'already unreadable' }));

      await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);

      expect(Array.from(bytesOf())).toEqual(Array.from(accountKey));
    }, 60_000);
  });

  describe('legacy migration', () => {
    it('does not replace a key the account already has', async () => {
      const key = await setUpFirstDevice();
      await wrapOnServer(NOTES_PASSWORD, crypto.getRandomValues(new Uint8Array(32)));
      const before = serverBlob;

      await tryMigrateToEnvelope(NOTES_PASSWORD, key);

      expect(putSpy).not.toHaveBeenCalled();
      expect(serverBlob).toBe(before);
    }, 60_000);

    it('uploads when the account has no key yet', async () => {
      const key = await setUpFirstDevice();

      await tryMigrateToEnvelope(NOTES_PASSWORD, key);

      expect(putSpy).toHaveBeenCalledTimes(1);
    }, 60_000);
  });

  describe('changePassword', () => {
    it('refuses a wrong current password while unlocked, and changes nothing', async () => {
      await setUpFirstDevice();
      const before = await encryptionRepository.getMetadata();

      await expect(changePassword('not-the-password', 'a-brand-new-password')).rejects.toThrow('Incorrect password');

      expect(await encryptionRepository.getMetadata()).toEqual(before);
      expect(putSpy).not.toHaveBeenCalled();
    }, 60_000);

    it('refuses a new password that is too short', async () => {
      await initialize(NOTES_PASSWORD);
      const before = await encryptionRepository.getMetadata();

      await expect(changePassword(NOTES_PASSWORD, 'short')).rejects.toBeInstanceOf(ValidationError);

      expect(await encryptionRepository.getMetadata()).toEqual(before);
    }, 60_000);

    it('accepts a new password of the minimum length', async () => {
      await initialize(NOTES_PASSWORD);
      await changePassword(NOTES_PASSWORD, '12345678');
      lock();
      await unlock('12345678');
      expect(isLocked()).toBe(false);
    }, 60_000);

    it('never applies the minimum at unlock, so existing short passwords keep working', async () => {
      await initialize('short');
      lock();
      await unlock('short');
      expect(isLocked()).toBe(false);
    }, 60_000);
  });

  describe('saved searches stored as plaintext', () => {
    const ROW = {
      id: 'c0000000-0000-4000-8000-000000000001', name: 'Old name', query: 'old query', order: 1,
      createdAt: '2026-01-01T00:00:00.000Z', modifiedAt: '2026-01-01T00:00:00.000Z', deleted: false, version: 1,
    };

    it('are encrypted at unlock, once, and queued for sync', async () => {
      await initialize(NOTES_PASSWORD);
      await getDB().put(STORES.SAVED_SEARCHES, ROW);
      lock();

      await unlock(NOTES_PASSWORD);

      const stored = await getDB().get(STORES.SAVED_SEARCHES, ROW.id);
      expect(JSON.parse(stored.name).ciphertext).toBeDefined();
      expect(stored.needsSync).toBe(true);
      expect((await settingsRepository.get()).savedSearchesEncrypted).toBe(true);
    }, 60_000);

    it('are left unreadable once the migration has run', async () => {
      await initialize(NOTES_PASSWORD);
      await settingsRepository.update({ savedSearchesEncrypted: true });
      await getDB().put(STORES.SAVED_SEARCHES, ROW);
      lock();

      await unlock(NOTES_PASSWORD);

      expect((await getDB().get(STORES.SAVED_SEARCHES, ROW.id)).name).toBe('Old name');
      const [read] = await savedSearchRepository.getAll();
      expect(read.name).not.toBe('Old name');
    }, 60_000);
  });

  describe('credentials restored from a backup', () => {
    const restoredMetadata = async () => {
      const key = keyManager.getMasterKey()!.key;
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: ENDPOINT,
        apiKey: 'RESTORE:' + JSON.stringify(await cryptoService.encryptText(API_KEY, key)),
      });
      const marker = (await syncRepository.getMetadata())?.apiKey;
      lock();
      return marker;
    };

    beforeEach(() => {
      vi.spyOn(authService, 'getWrappedKey').mockResolvedValue(null);
    });

    it('stay in place when the device is offline, and work at the next unlock', async () => {
      await initialize(NOTES_PASSWORD);
      const marker = await restoredMetadata();
      const clone = vi.spyOn(authService, 'cloneDevice').mockRejectedValueOnce(new TypeError('Failed to fetch'));

      await unlock(NOTES_PASSWORD);

      expect((await syncRepository.getMetadata())?.apiKey).toBe(marker);
      expect((await syncRepository.getMetadata())?.syncEnabled).toBe(true);

      lock();
      clone.mockResolvedValueOnce({ clientId: 'new', apiKey: 'new-key', userId: USER_ID, deviceName: 'd' });
      await unlock(NOTES_PASSWORD);

      expect((await syncRepository.getMetadata())?.apiKey?.startsWith('RESTORE:')).toBe(false);
      expect((await syncRepository.getMetadata())?.clientId).toBe('new');
    }, 60_000);

    it('stay in place when the server is failing', async () => {
      await initialize(NOTES_PASSWORD);
      const marker = await restoredMetadata();
      vi.spyOn(authService, 'cloneDevice').mockRejectedValueOnce(new AuthApiError('down', 503));

      await unlock(NOTES_PASSWORD);

      expect((await syncRepository.getMetadata())?.apiKey).toBe(marker);
    }, 60_000);

    it('stay in place when the account is disabled, because the key still works', async () => {
      await initialize(NOTES_PASSWORD);
      const marker = await restoredMetadata();
      vi.spyOn(authService, 'cloneDevice').mockRejectedValueOnce(new AuthApiError('disabled', 403));

      await unlock(NOTES_PASSWORD);

      expect((await syncRepository.getMetadata())?.apiKey).toBe(marker);
    }, 60_000);

    it('are cleared when the server says the key is no good', async () => {
      await initialize(NOTES_PASSWORD);
      await restoredMetadata();
      vi.spyOn(authService, 'cloneDevice').mockRejectedValueOnce(new AuthApiError('bad key', 401));

      await unlock(NOTES_PASSWORD);

      expect((await syncRepository.getMetadata())?.apiKey).toBe('');
      expect((await syncRepository.getMetadata())?.syncEnabled).toBe(false);
    }, 60_000);
  });

  describe('the parameters a blob claims', () => {
    async function connect(): Promise<Promise<void>> {
      return onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);
    }

    async function serverBlobWrapped(iterations: number, kdfVersion = 1): Promise<Uint8Array> {
      const key = crypto.getRandomValues(new Uint8Array(32));
      const wrappingKey = await cryptoService.deriveWrappingKey(NOTES_PASSWORD, USER_ID, iterations);
      serverBlob = { blob: JSON.stringify(await cryptoService.wrapMasterKey(wrappingKey, key)), kdfVersion, kdfIterations: iterations };
      return key;
    }

    it('are honoured for a real increase, so a later client can raise them', async () => {
      await initialize(NOTES_PASSWORD);
      const key = await serverBlobWrapped(1_500_000);

      await connect();

      expect(Array.from(bytesOf())).toEqual(Array.from(key));
    }, 60_000);

    it('refuse anything weaker than clients use, whatever the server claims', async () => {
      await initialize(NOTES_PASSWORD);
      await serverBlobWrapped(1000);

      await expect(connect()).rejects.toBeInstanceOf(AccountKeyUnreadableError);
    }, 60_000);

    it('refuse a scheme this client does not know', async () => {
      await initialize(NOTES_PASSWORD);
      await serverBlobWrapped(1_000_000, 99);

      await expect(connect()).rejects.toBeInstanceOf(AccountKeyUnreadableError);
    }, 60_000);

    it('refuse a count high enough to hang the app', async () => {
      await initialize(NOTES_PASSWORD);
      serverBlob = { blob: '{}', kdfVersion: 1, kdfIterations: 2_000_000_000 };
      const derive = vi.spyOn(cryptoService, 'deriveWrappingKey');

      await expect(connect()).rejects.toBeInstanceOf(AccountKeyUnreadableError);

      expect(derive).not.toHaveBeenCalled();
    }, 60_000);

    it('are never used to wrap a key', async () => {
      await setUpFirstDevice();
      const derive = vi.spyOn(cryptoService, 'deriveWrappingKey');
      serverBlob = null;

      await uploadWrappedKeyToServer(NOTES_PASSWORD);

      expect(derive).toHaveBeenCalledWith(NOTES_PASSWORD, USER_ID);
      expect(JSON.parse(JSON.stringify(serverBlob)).kdfIterations).toBe(1_000_000);
    }, 60_000);
  });

  describe('unlocking', () => {
    async function legacyVault(password: string): Promise<CryptoKey> {
      const salt = cryptoService.generateSalt();
      await encryptionRepository.setMetadata({
        salt: arrayBufferToBase64(salt), iterations: 1000, createdAt: new Date().toISOString(), algorithm: 'AES-256-GCM',
      } as never);
      return cryptoService.deriveKey({ password, salt, iterations: 1000, algorithm: 'PBKDF2', extractable: true });
    }

    const noteHolding = async (key: CryptoKey, text: string) => createTestNote({
      content: JSON.stringify(await cryptoService.encryptText(text, key)),
    });

    it('is not stopped by a corrupt note in an envelope vault', async () => {
      await initialize(NOTES_PASSWORD);
      await noteRepository.create(createTestNote({ content: 'not ciphertext' }));
      lock();

      await expect(unlock(NOTES_PASSWORD)).resolves.toBeUndefined();
    }, 60_000);

    it('still refuses a wrong password in an envelope vault', async () => {
      await initialize(NOTES_PASSWORD);
      lock();

      await expect(unlock('wrong password')).rejects.toThrow('Incorrect password');
    }, 60_000);

    it('reads no notes in an envelope vault, which slowed every unlock', async () => {
      await initialize(NOTES_PASSWORD);
      lock();
      const active = vi.spyOn(noteRepository, 'getAllActive');
      const all = vi.spyOn(noteRepository, 'getAll');

      await unlock(NOTES_PASSWORD);

      expect(active).not.toHaveBeenCalled();
      expect(all).not.toHaveBeenCalled();
    }, 60_000);

    it('accepts a legacy vault whose first note is corrupt, if another one opens', async () => {
      const key = await legacyVault(NOTES_PASSWORD);
      await noteRepository.create(createTestNote({ content: 'not ciphertext' }));
      await noteRepository.create(await noteHolding(key, 'readable'));

      await expect(unlock(NOTES_PASSWORD)).resolves.toBeUndefined();
    }, 60_000);

    it('refuses a legacy vault when no note opens', async () => {
      const key = await legacyVault(NOTES_PASSWORD);
      await noteRepository.create(await noteHolding(key, 'readable'));

      await expect(unlock('wrong password')).rejects.toThrow('Incorrect password');
    }, 60_000);

    it('does not migrate a legacy vault it could not verify', async () => {
      // With no notes any password appears to work, and migrating would keep it. The
      // API key opens with what was typed, so nothing but the check can stop it.
      await legacyVault(NOTES_PASSWORD);
      const metadata = await encryptionRepository.getMetadata();
      const typed = await cryptoService.deriveKey({
        password: 'any password at all', salt: base64ToUint8Array(metadata!.salt!), iterations: 1000, algorithm: 'PBKDF2', extractable: true,
      });
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: ENDPOINT,
        userId: USER_ID,
        apiKey: JSON.stringify(await cryptoService.encryptText(API_KEY, typed)),
      });

      await unlock('any password at all');

      expect((await encryptionRepository.getMetadata())?.envelopeVersion).toBeFalsy();
    }, 60_000);
  });

  describe('deleteAllLocalData', () => {
    beforeEach(() => {
      // Node's own global is undefined without a storage file, and shadows happy-dom's
      const entries = new Map<string, string>();
      vi.stubGlobal('localStorage', {
        getItem: (key: string) => entries.get(key) ?? null,
        setItem: (key: string, value: string) => void entries.set(key, value),
        removeItem: (key: string) => void entries.delete(key),
      });
    });

    afterEach(() => vi.unstubAllGlobals());

    it('leaves nothing behind, including a remembered password', async () => {
      await initialize(NOTES_PASSWORD);
      await passwordStorageService.store(NOTES_PASSWORD);
      localStorage.setItem('jottery_stored_password', 'left by an older version');
      localStorage.setItem('jottery-theme', 'dark');
      localStorage.setItem('jottery-locale', 'fr');
      sessionStorageService.store(NOTES_PASSWORD);

      await deleteAllLocalData();

      expect(await passwordStorageService.hasStored()).toBe(false);
      expect(localStorage.getItem('jottery_stored_password')).toBeNull();
      expect(localStorage.getItem('jottery-theme')).toBeNull();
      expect(localStorage.getItem('jottery-locale')).toBeNull();
      expect(sessionStorageService.hasStored()).toBe(false);

      await initDB();
      expect(await encryptionRepository.getMetadata()).toBeFalsy();
    }, 60_000);
  });

  describe('verifyUnlockPassword', () => {
    it('accepts the vault password and rejects any other', async () => {
      await setUpFirstDevice();

      expect(await verifyUnlockPassword(NOTES_PASSWORD)).toBe(true);
      expect(await verifyUnlockPassword(SERVER_PASSWORD)).toBe(false);
    }, 60_000);

    it('rejects everything while the vault is locked', async () => {
      await setUpFirstDevice();
      keyManager.clearMasterKey();

      expect(await verifyUnlockPassword(NOTES_PASSWORD)).toBe(false);
    }, 60_000);
  });

  describe('ensureServerWrappedKey', () => {
    it('uploads a key when the server has none, so other devices can onboard', async () => {
      const firstDeviceKey = await setUpFirstDevice();
      expect(serverBlob).toBeNull();

      await ensureServerWrappedKey(NOTES_PASSWORD);
      expect(putSpy).toHaveBeenCalledTimes(1);

      await becomeSecondDevice();
      await initialize(NOTES_PASSWORD);
      await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);
      expect(Array.from(bytesOf())).toEqual(Array.from(firstDeviceKey));
    }, 60_000);

    it('never overwrites a key the server already holds', async () => {
      await setUpFirstDevice();
      serverBlob = { blob: '{"ciphertext":"x","iv":"y"}', kdfVersion: 1, kdfIterations: 1_000_000 };

      await ensureServerWrappedKey(NOTES_PASSWORD);

      expect(putSpy).not.toHaveBeenCalled();
    }, 60_000);

    describe('repairing a key an older client wrapped with the account password', () => {
      it('re-wraps once this device has synced, so a new device can onboard', async () => {
        const key = await setUpFirstDevice();
        await wrapOnServer(SERVER_PASSWORD, key);
        await markSynced();

        await ensureServerWrappedKey(NOTES_PASSWORD);
        expect(putSpy).toHaveBeenCalledTimes(1);

        await becomeSecondDevice();
        await initialize(NOTES_PASSWORD);
        await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);
        expect(Array.from(bytesOf())).toEqual(Array.from(key));
      }, 60_000);

      it('leaves it alone if this device has never synced', async () => {
        const key = await setUpFirstDevice();
        await wrapOnServer(SERVER_PASSWORD, key);

        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).not.toHaveBeenCalled();
      }, 60_000);

      it('replaces a blob that cannot be parsed, once synced', async () => {
        await setUpFirstDevice();
        await markSynced();
        serverBlob = { blob: 'not json', kdfVersion: 1, kdfIterations: 1 };

        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).toHaveBeenCalledTimes(1);
      }, 60_000);

      it('never overwrites a blob that unwraps, even to a different key', async () => {
        // That means this device has diverged from the account. The account's key
        // is the one to keep: replacing it would damage every other device.
        await setUpFirstDevice();
        await markSynced();
        await wrapOnServer(NOTES_PASSWORD, crypto.getRandomValues(new Uint8Array(32)));

        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).not.toHaveBeenCalled();
      }, 60_000);

      it('never overwrites a blob that already unwraps to this key', async () => {
        const key = await setUpFirstDevice();
        await markSynced();
        await wrapOnServer(NOTES_PASSWORD, key);

        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).not.toHaveBeenCalled();
      }, 60_000);
    });

    describe('after the password changes on another device', () => {
      const NEW_PASSWORD = 'changed-on-another-device';
      const CHANGED_ELSEWHERE = 'unlock.passwordChangedElsewhere';

      /** The other device's password change, as this one sees it: a new blob on the server. */
      const otherDeviceChangesPassword = (key: Uint8Array) => wrapOnServer(NEW_PASSWORD, key);

      async function serverOpensWith(password: string, key: Uint8Array): Promise<boolean> {
        const wrappingKey = await cryptoService.deriveWrappingKey(password, USER_ID);
        try {
          const opened = await cryptoService.unwrapMasterKey(wrappingKey, JSON.parse(serverBlob!.blob));
          return opened.every((byte, i) => byte === key[i]);
        } catch {
          return false;
        }
      }

      it('leaves the blob alone on a device that uploaded it, and says why', async () => {
        const info = vi.spyOn(toast, 'info');
        const key = await setUpFirstDevice();
        await ensureServerWrappedKey(NOTES_PASSWORD);
        await markSynced();
        expect(putSpy).toHaveBeenCalledTimes(1);

        await otherDeviceChangesPassword(key);
        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).toHaveBeenCalledTimes(1);
        expect(await serverOpensWith(NEW_PASSWORD, key)).toBe(true);
        expect(info).toHaveBeenCalledWith(CHANGED_ELSEWHERE);
      }, 60_000);

      it('leaves the blob alone on a device that has only ever opened it', async () => {
        const key = await setUpFirstDevice();
        await wrapOnServer(NOTES_PASSWORD, key);
        await markSynced();
        await ensureServerWrappedKey(NOTES_PASSWORD);

        await otherDeviceChangesPassword(key);
        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).not.toHaveBeenCalled();
      }, 60_000);

      it('leaves the blob alone on a device that joined by opening it', async () => {
        const key = await setUpFirstDevice();
        await wrapOnServer(NOTES_PASSWORD, key);
        await becomeSecondDevice();
        await initialize(NOTES_PASSWORD);
        await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD);
        await syncRepository.updateMetadata({
          syncEnabled: true,
          syncEndpoint: ENDPOINT,
          userId: USER_ID,
          apiKey: JSON.stringify(await cryptoService.encryptText(API_KEY, keyManager.getMasterKey()!.key)),
        });
        await markSynced();

        await otherDeviceChangesPassword(key);
        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).not.toHaveBeenCalled();
      }, 60_000);

      it('stops two devices taking turns to overwrite each other', async () => {
        // Both devices start on the first password. Each round is the second
        // device's next unlock, with the first device's change already on the server.
        const key = await setUpFirstDevice();
        await wrapOnServer(NOTES_PASSWORD, key);
        await markSynced();
        await ensureServerWrappedKey(NOTES_PASSWORD);

        await otherDeviceChangesPassword(key);
        for (let unlocks = 0; unlocks < 3; unlocks++) {
          await ensureServerWrappedKey(NOTES_PASSWORD);
        }

        expect(putSpy).not.toHaveBeenCalled();
        expect(await serverOpensWith(NEW_PASSWORD, key)).toBe(true);
      }, 60_000);

      it('heals an old-format blob only once', async () => {
        const key = await setUpFirstDevice();
        await wrapOnServer(SERVER_PASSWORD, key);
        await markSynced();

        await ensureServerWrappedKey(NOTES_PASSWORD);
        expect(putSpy).toHaveBeenCalledTimes(1);
        expect(await serverOpensWith(NOTES_PASSWORD, key)).toBe(true);

        await otherDeviceChangesPassword(key);
        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).toHaveBeenCalledTimes(1);
      }, 60_000);

      it('still uploads when the server has lost the blob', async () => {
        await setUpFirstDevice();
        await ensureServerWrappedKey(NOTES_PASSWORD);
        expect(putSpy).toHaveBeenCalledTimes(1);

        serverBlob = null;
        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(putSpy).toHaveBeenCalledTimes(2);
      }, 60_000);

      it('forgets the proof when the device moves to another account', async () => {
        await setUpFirstDevice();
        await ensureServerWrappedKey(NOTES_PASSWORD);
        await markSynced();

        await syncRepository.updateMetadata({ userId: 'c9d2e6b3-9a1f-4d66-8e63-3a7b2d1f8b22' });

        const metadata = await syncRepository.getMetadata();
        expect(metadata?.serverKeyVerified).toBeFalsy();
        expect(metadata?.lastSyncAt).toBeUndefined();
      }, 60_000);
    });

    describe('saying what each write is for', () => {
      const CHANGED_ELSEWHERE = 'unlock.passwordChangedElsewhere';
      const NEW_PASSWORD = 'changed-at-the-same-moment';

      it('asks for a first upload to fail if the account already has a key', async () => {
        await setUpFirstDevice();

        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(writes).toEqual([{ replace: false }]);
      }, 60_000);

      it('leaves a key that another device uploaded first', async () => {
        const key = await setUpFirstDevice();
        beforeNextWrite = () => wrapOnServer(NEW_PASSWORD, key);

        await ensureServerWrappedKey(NOTES_PASSWORD);

        const wrappingKey = await cryptoService.deriveWrappingKey(NEW_PASSWORD, USER_ID);
        await expect(cryptoService.unwrapMasterKey(wrappingKey, JSON.parse(serverBlob!.blob))).resolves.toBeTruthy();
      }, 60_000);

      it('reports the same race on first device setup, rather than replacing the key', async () => {
        const key = await setUpFirstDevice();
        beforeNextWrite = () => wrapOnServer(NEW_PASSWORD, key);

        await expect(uploadWrappedKeyToServer(NOTES_PASSWORD)).rejects.toBeInstanceOf(AccountKeyExistsError);
      }, 60_000);

      it('still finishes moving a legacy vault if another device uploaded first', async () => {
        const key = await setUpFirstDevice();
        beforeNextWrite = () => wrapOnServer(NEW_PASSWORD, key);
        const convert = vi.spyOn(encryptionRepository, 'setMetadata');

        await tryMigrateToEnvelope(NOTES_PASSWORD, key);

        expect(convert).toHaveBeenCalled();
      }, 60_000);

      it('makes a repair conditional on the blob it read', async () => {
        const key = await setUpFirstDevice();
        await wrapOnServer(SERVER_PASSWORD, key);
        const read = serverBlob!.updatedAt;
        await markSynced();

        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(writes).toEqual([{ ifMatch: read }]);
      }, 60_000);

      it('backs off a repair if the blob changes first, and never tries again', async () => {
        // This is the race the per-device flag alone can't close: the device has no
        // proof yet, and another device changes the password between its read and write
        const info = vi.spyOn(toast, 'info');
        const key = await setUpFirstDevice();
        await wrapOnServer(SERVER_PASSWORD, key);
        await markSynced();
        beforeNextWrite = () => wrapOnServer(NEW_PASSWORD, key);

        await ensureServerWrappedKey(NOTES_PASSWORD);
        expect(info).toHaveBeenCalledWith(CHANGED_ELSEWHERE);

        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(writes).toHaveLength(1);
        const wrappingKey = await cryptoService.deriveWrappingKey(NEW_PASSWORD, USER_ID);
        await expect(cryptoService.unwrapMasterKey(wrappingKey, JSON.parse(serverBlob!.blob))).resolves.toBeTruthy();
      }, 60_000);

      it('sends no If-Match to a server that gave no updatedAt, whose preflight could refuse it', async () => {
        olderServer = true;
        const key = await setUpFirstDevice();
        await wrapOnServer(SERVER_PASSWORD, key);
        await markSynced();

        await ensureServerWrappedKey(NOTES_PASSWORD);

        expect(writes).toEqual([{ replace: true }]);
        expect(putSpy).toHaveBeenCalledTimes(1);
      }, 60_000);

      it('replaces the key on a password change, whatever is there', async () => {
        const key = await setUpFirstDevice();
        await wrapOnServer(NOTES_PASSWORD, key);

        await changePassword(NOTES_PASSWORD, 'a-brand-new-password');

        expect(writes).toEqual([{ replace: true }]);
      }, 60_000);

      it('keeps retrying its own password change after a conflict', async () => {
        const key = await setUpFirstDevice();
        await wrapOnServer(NOTES_PASSWORD, key);
        putSpy.mockRejectedValueOnce(new TypeError('Failed to fetch'));
        await changePassword(NOTES_PASSWORD, 'a-brand-new-password');

        beforeNextWrite = () => wrapOnServer(NEW_PASSWORD, key);
        await ensureServerWrappedKey('a-brand-new-password');

        expect((await syncRepository.getMetadata())?.serverKeyStale).toBe(true);
      }, 60_000);
    });

    describe('when a password change here fails to reach the server', () => {
      const NEW_PASSWORD = 'changed-on-this-device';

      it('retries at the next unlock, and does not blame another device', async () => {
        const info = vi.spyOn(toast, 'info');
        const key = await setUpFirstDevice();
        await ensureServerWrappedKey(NOTES_PASSWORD);
        await markSynced();
        expect(putSpy).toHaveBeenCalledTimes(1);

        putSpy.mockRejectedValueOnce(new TypeError('Failed to fetch'));
        const result = await changePassword(NOTES_PASSWORD, NEW_PASSWORD);
        expect(result.serverKeyUpdated).toBe(false);
        expect((await syncRepository.getMetadata())?.serverKeyStale).toBe(true);

        await ensureServerWrappedKey(NEW_PASSWORD);

        expect(putSpy).toHaveBeenCalledTimes(3);
        expect(info).not.toHaveBeenCalled();
        expect((await syncRepository.getMetadata())?.serverKeyStale).toBeFalsy();
        const wrappingKey = await cryptoService.deriveWrappingKey(NEW_PASSWORD, USER_ID);
        expect(Array.from(await cryptoService.unwrapMasterKey(wrappingKey, JSON.parse(serverBlob!.blob)))).toEqual(Array.from(key));
      }, 60_000);

      it('keeps retrying until the upload works', async () => {
        await setUpFirstDevice();
        await ensureServerWrappedKey(NOTES_PASSWORD);

        putSpy.mockRejectedValueOnce(new TypeError('Failed to fetch'));
        await changePassword(NOTES_PASSWORD, NEW_PASSWORD);

        putSpy.mockRejectedValueOnce(new TypeError('Failed to fetch'));
        await ensureServerWrappedKey(NEW_PASSWORD);
        expect((await syncRepository.getMetadata())?.serverKeyStale).toBe(true);

        await ensureServerWrappedKey(NEW_PASSWORD);
        expect((await syncRepository.getMetadata())?.serverKeyStale).toBeFalsy();
      }, 60_000);

      it('reports success when the upload works first time', async () => {
        await setUpFirstDevice();
        await ensureServerWrappedKey(NOTES_PASSWORD);

        const result = await changePassword(NOTES_PASSWORD, NEW_PASSWORD);

        expect(result.serverKeyUpdated).toBe(true);
        expect((await syncRepository.getMetadata())?.serverKeyStale).toBeFalsy();
      }, 60_000);
    });

    it('does nothing when sync is not configured', async () => {
      await initialize(NOTES_PASSWORD);

      await ensureServerWrappedKey(NOTES_PASSWORD);

      expect(putSpy).not.toHaveBeenCalled();
    }, 60_000);

    it('swallows network failures so unlock is never blocked', async () => {
      await setUpFirstDevice();
      vi.spyOn(authService, 'getWrappedKey').mockRejectedValueOnce(new TypeError('Failed to fetch'));

      await expect(ensureServerWrappedKey(NOTES_PASSWORD)).resolves.toBeUndefined();
      expect(putSpy).not.toHaveBeenCalled();
    }, 60_000);
  });
});
