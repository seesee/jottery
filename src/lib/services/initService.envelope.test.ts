/**
 * Tests for the server-wrapped master key flow between devices.
 *
 * The first device uploads a copy of the master key wrapped with a key derived
 * from a password and the user ID. A second device downloads it and unwraps it.
 * The server receives the account (server) password in plaintext at login, so
 * the blob must be wrapped with the notes password, which never leaves the client.
 * Older clients wrapped it with the server password, so downloads must still cope.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  initialize,
  onboardFromServer,
  tryMigrateToEnvelope,
  uploadWrappedKeyToServer,
  verifyUnlockPassword,
  ensureServerWrappedKey,
} from './initService';
import { authService } from './authService';
import { cryptoService } from './crypto';
import { keyManager } from './keyManager';
import { encryptionRepository } from './encryptionRepository';
import { syncRepository } from './syncRepository';
import { closeDB, deleteDB, initDB } from './db';
import { base64ToUint8Array } from '../utils/base64';
import { CRYPTO_PBKDF2_ITERATIONS } from '../constants';
import type { PutWrappedKeyRequest, WrappedKeyResponse } from './authService';

const ENDPOINT = 'https://sync.example.test';
const USER_ID = 'b3c1f5a2-8f0e-4c55-9d52-2f6a1c0e7a11';
const API_KEY = 'plaintext-device-api-key';
const NOTES_PASSWORD = 'notes-password-correct-horse';
const SERVER_PASSWORD = 'server-password-battery-staple';

/** What the fake server currently holds for the user */
let serverBlob: WrappedKeyResponse | null;
let putSpy: ReturnType<typeof vi.spyOn>;

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

    putSpy = vi
      .spyOn(authService, 'putWrappedKey')
      .mockImplementation(async (_endpoint: string, _token: string, request: PutWrappedKeyRequest) => {
        serverBlob = { blob: request.blob, kdfVersion: request.kdfVersion, kdfIterations: request.kdfIterations };
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

    it('reports upload failures instead of swallowing them', async () => {
      await setUpFirstDevice();
      putSpy.mockRejectedValueOnce(new Error('Failed to store wrapped key: 500'));

      await expect(uploadWrappedKeyToServer(NOTES_PASSWORD)).rejects.toThrow(/500/);
    }, 60_000);
  });

  describe('second device download', () => {
    it('falls back to the server password for keys uploaded by older clients', async () => {
      const firstDeviceKey = await setUpFirstDevice();

      // Older clients wrapped the blob with the server password
      await tryMigrateToEnvelope(SERVER_PASSWORD, firstDeviceKey);
      expect(serverBlob).not.toBeNull();
      await becomeSecondDevice();

      await initialize(NOTES_PASSWORD);
      await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD, SERVER_PASSWORD);

      expect(Array.from(bytesOf())).toEqual(Array.from(firstDeviceKey));
    }, 60_000);

    it('keeps the notes password as the local unlock password after a fallback', async () => {
      const firstDeviceKey = await setUpFirstDevice();
      await tryMigrateToEnvelope(SERVER_PASSWORD, firstDeviceKey);
      await becomeSecondDevice();
      await initialize(NOTES_PASSWORD);

      await onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD, SERVER_PASSWORD);

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

    it('fails with a clear error and changes nothing when neither password matches', async () => {
      await setUpFirstDevice();
      await uploadWrappedKeyToServer(NOTES_PASSWORD);
      await becomeSecondDevice();
      await initialize(NOTES_PASSWORD);
      const before = await encryptionRepository.getMetadata();
      const keyBefore = Array.from(bytesOf());

      await expect(
        onboardFromServer('wrong-notes-password', ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD, 'wrong-server-password')
      ).rejects.toThrow(/unwrap/i);

      expect(await encryptionRepository.getMetadata()).toEqual(before);
      expect(Array.from(bytesOf())).toEqual(keyBefore);
    }, 60_000);

    it('reports a missing server key distinctly from a wrong password', async () => {
      await initialize(NOTES_PASSWORD);

      await expect(
        onboardFromServer(NOTES_PASSWORD, ENDPOINT, USER_ID, API_KEY, NOTES_PASSWORD, SERVER_PASSWORD)
      ).rejects.toThrow(/no wrapped key found/i);
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
