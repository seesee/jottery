/**
 * Application initialization service
 * Handles first-time setup and unlock/lock operations
 */

import type { EncryptionMetadata, MasterKey, Note } from '../types';
import type { BackupData } from './backupService';
import { encryptionRepository } from './encryptionRepository';
import { settingsRepository } from './settingsRepository';
import { noteRepository } from './noteRepository';
import { noteService } from './noteService';
import { syncRepository } from './syncRepository';
import { savedSearchRepository } from './savedSearchRepository';
import { cryptoService } from './crypto';
import { keyManager } from './keyManager';
import { sessionStorageService } from './sessionStorageService';
import { passwordStorageService } from './passwordStorageService';
import { AuthApiError, authService } from './authService';
import type { WrappedKeyResponse, WrappedKeyWrite } from './authService';
import { arrayBufferToBase64, base64ToUint8Array } from '../utils/base64';
import { CRYPTO_PBKDF2_ITERATIONS, CRYPTO_WRAPPING_ITERATIONS, CRYPTO_WRAPPING_KDF_VERSION, CRYPTO_WRAPPING_MAX_ITERATIONS } from '../constants';
import type { EncryptionResult } from '../types/crypto';
import { restoreFromBackup as restoreBackupData, verifyBackupContents, verifyBackupPassword } from './backupService';
import { backupSchedulerService } from './backupSchedulerService';
import { clearAllStores, deleteDB, initDB } from './db';
import { syncService } from './syncService';
import { restrictConnections } from './cspService';
import { searchService } from './searchService';
import { AccountKeyExistsError, AccountKeyUnreadableError, LocalNotesOrphanedError, ValidationError } from '../errors';
import { isNotesPasswordLongEnough, MIN_NOTES_PASSWORD_LENGTH } from '../utils/passwordRules';
import { toast } from '../utils/toast.svelte';
import { get } from 'svelte/store';
import { _ } from 'svelte-i18n';

/**
 * Check if the application has been initialized
 */
export async function isInitialized(): Promise<boolean> {
  return await encryptionRepository.isInitialized();
}

/**
 * Initialize the application with a password (first-time setup)
 */
export async function initialize(password: string): Promise<void> {
  // Check if encryption metadata already exists (from imported credentials)
  const existingMetadata = await encryptionRepository.getMetadata();

  if (existingMetadata) {
    // Metadata exists from credential import - use existing salt
    // Just need to unlock with the imported salt
    console.log('[Initialize] Using existing encryption metadata from credential import');
    await unlock(password);
  } else {
    // Fresh installation — envelope encryption
    console.log('[Initialize] Fresh installation - generating envelope encryption metadata');

    // Generate random master key and per-device salt
    const masterKeyBytes = cryptoService.generateMasterKey();
    const deviceSalt = cryptoService.generateSalt();

    // Derive device key for local storage
    const deviceKey = await cryptoService.deriveKey({
      password,
      salt: deviceSalt,
      iterations: CRYPTO_PBKDF2_ITERATIONS,
      extractable: false,
    });

    // Wrap master key for local storage
    const localWrapped = await cryptoService.wrapMasterKey(deviceKey, masterKeyBytes);

    const metadata: EncryptionMetadata = {
      envelopeVersion: 1,
      deviceSalt: arrayBufferToBase64(deviceSalt),
      localWrappedMaster: JSON.stringify(localWrapped),
      wrappingKdfVersion: CRYPTO_WRAPPING_KDF_VERSION,
      createdAt: new Date().toISOString(),
      algorithm: 'AES-256-GCM',
    };

    await encryptionRepository.setMetadata(metadata);

    // Import the raw master key bytes as a CryptoKey for encryption operations
    const key = await crypto.subtle.importKey(
      'raw',
      masterKeyBytes as Uint8Array<ArrayBuffer>,
      { name: 'AES-GCM', length: 256 },
      true,
      ['encrypt', 'decrypt']
    );

    // Store the master key
    const masterKey: MasterKey = {
      key,
      keyBytes: masterKeyBytes,
      derivedAt: Date.now(),
    };
    keyManager.setMasterKey(masterKey);
  }

  // Ensure default settings are saved
  const settingsExist = await settingsRepository.exists();
  if (!settingsExist) {
    await settingsRepository.reset();
  }
}

/**
 * Unlock the application with a password
 */
export async function unlock(password: string): Promise<void> {
  console.log('[Unlock] Starting unlock process...');

  const metadata = await encryptionRepository.getMetadata();
  if (!metadata) {
    console.error('[Unlock] No encryption metadata found!');
    throw new Error('Application not initialized. Please initialize first.');
  }

  let key: CryptoKey;
  let keyBytes: Uint8Array;

  if (metadata.envelopeVersion) {
    // === ENVELOPE PATH ===
    console.log('[Unlock] Envelope encryption detected (v' + metadata.envelopeVersion + ')');

    const deviceSalt = base64ToUint8Array(metadata.deviceSalt!);
    const deviceKey = await cryptoService.deriveKey({
      password,
      salt: deviceSalt,
      iterations: CRYPTO_PBKDF2_ITERATIONS,
      extractable: false,
    });

    // Unwrap the master key from local storage
    const localWrapped: EncryptionResult = JSON.parse(metadata.localWrappedMaster!);
    try {
      keyBytes = await cryptoService.unwrapMasterKey(deviceKey, localWrapped);
    } catch {
      throw new Error('Incorrect password');
    }

    // Import as CryptoKey
    key = await crypto.subtle.importKey(
      'raw',
      keyBytes as Uint8Array<ArrayBuffer>,
      { name: 'AES-GCM', length: 256 },
      true,
      ['encrypt', 'decrypt']
    );

    console.log('[Unlock] ✓ Master key unwrapped from envelope');
  } else if (metadata.salt) {
    // === LEGACY PATH ===
    console.log('[Unlock] Legacy encryption detected, deriving key...');
    console.log('[Unlock] Salt length:', metadata.salt.length, 'Iterations:', metadata.iterations);

    const salt = base64ToUint8Array(metadata.salt);
    key = await cryptoService.deriveKey({
      password,
      salt,
      iterations: metadata.iterations!,
      algorithm: 'PBKDF2',
      extractable: true,
    });

    const rawKey = await crypto.subtle.exportKey('raw', key);
    keyBytes = new Uint8Array(rawKey);

    console.log('[Unlock] ✓ Master key derived (legacy)');
  } else {
    throw new Error('Invalid encryption metadata: neither envelope nor legacy fields found');
  }

  // An envelope vault's key was authenticated by unwrapping it. A legacy vault has
  // nothing to check a derived key against but its own notes.
  const legacyKeyVerified = metadata.envelopeVersion ? true : await legacyKeyOpensNotes(key);

  // Store the master key with bytes for JWE operations
  const masterKey: MasterKey = {
    key,
    keyBytes,
    derivedAt: Date.now(),
  };

  keyManager.setMasterKey(masterKey);

  // Handle restored credentials (RESTORE: marker from backup restore)
  await handleImportedCredentials(key);

  await encryptLegacySavedSearches();

  // Moves anything an older version left in plaintext into the encrypted settings
  await settingsRepository.update({});

  // Decrypted notes exist from here on, so limit where they could be sent
  const connectionMetadata = await syncRepository.getMetadata();
  if (connectionMetadata?.syncEnabled && connectionMetadata.syncEndpoint) {
    restrictConnections(connectionMetadata.syncEndpoint);
  }

  // Attempt migration from legacy to envelope encryption. An unverified key could be
  // any password at all, and migrating would make it the vault's key for good.
  if (!metadata.envelopeVersion && legacyKeyVerified) {
    await tryMigrateToEnvelope(password, keyBytes);
  }

  // Make sure other devices can onboard: the server needs a copy of the key.
  // Runs in the background so a slow or offline server never delays unlock.
  void ensureServerWrappedKey(password);

  noteService.purgeOldDeletedNotes().catch(error => {
    console.warn('[Unlock] Could not empty old notes from the recycle bin:', error);
  });

  // Start backup scheduler
  backupSchedulerService.start();
  console.log('[Unlock] ✓ Backup scheduler started');

  console.log('[Unlock] ✓ Unlock complete!');
}

/**
 * Check a legacy vault's derived key against its notes, one at a time, so that one
 * corrupt note can't lock anyone out of a healthy vault.
 *
 * @returns false if there was nothing to check it against
 * @throws if no note opens with it
 */
async function legacyKeyOpensNotes(key: CryptoKey): Promise<boolean> {
  const notes = await noteRepository.getAll();
  if (notes.length === 0) {
    return false;
  }

  for (const note of notes) {
    if (await noteOpensWith(note, key)) {
      return true;
    }
  }
  throw new Error('Incorrect password');
}

/**
 * Only a 401 says the restored key is no good. A 403 means the account is disabled,
 * and reactivating it restores the key. A TypeError is fetch failing to connect.
 * Anything else means the marker itself is unusable.
 */
function canRetryRestoredDevice(error: unknown): boolean {
  if (error instanceof AuthApiError) {
    return error.status !== 401;
  }
  return error instanceof TypeError;
}

/**
 * Handle imported credentials after successful unlock
 * Detects RESTORE: marker from backup restore, calls clone-device to register as new device
 */
async function handleImportedCredentials(masterKey: CryptoKey): Promise<void> {
  try {
    const metadata = await syncRepository.getMetadata();

    if (!metadata?.apiKey) {
      return;
    }

    // Get device name (default to 'Restored Device' if not set)
    const deviceName = metadata.pendingDeviceName || 'Restored Device';

    // Check for RESTORE: marker (credentials restored from backup - need new device ID)
    if (metadata.apiKey.startsWith('RESTORE:')) {
      console.log('[ImportHandler] ✓ RESTORE marker detected! Re-registering device with new ID...');

      // Extract encrypted API key payload (JSON stringified EncryptedData)
      const encryptedPayload = metadata.apiKey.substring(8); // Remove "RESTORE:" prefix

      try {
        // Parse the encrypted data
        const encryptedData = JSON.parse(encryptedPayload);

        // Decrypt to get the plaintext API key
        const plaintextApiKey = await cryptoService.decryptText(encryptedData, masterKey);
        console.log('[ImportHandler] ✓ API key decrypted successfully');

        const endpoint = metadata.syncEndpoint;
        if (!endpoint) {
          throw new Error('No sync endpoint found for restored credentials');
        }

        // Call clone-device to register as a NEW device
        console.log('[ImportHandler] Calling clone-device to register new device...');
        const cloneResult = await authService.cloneDevice(endpoint, plaintextApiKey, deviceName);
        console.log('[ImportHandler] ✓ Clone-device successful, got new clientId:', cloneResult.clientId);

        // Encrypt the NEW API key
        const encryptedApiKey = await cryptoService.encryptText(cloneResult.apiKey, masterKey);
        console.log('[ImportHandler] ✓ New API key encrypted');

        // Update sync metadata with NEW credentials
        await syncRepository.updateMetadata({
          clientId: cloneResult.clientId,
          userId: cloneResult.userId,
          apiKey: JSON.stringify(encryptedApiKey),
          syncEnabled: true,
          pendingDeviceName: undefined,
        });
        console.log('[ImportHandler] ✓ Sync metadata updated with new device credentials');

        console.log('[ImportHandler] ✓✓✓ Restored credentials processed successfully! Registered as new device.');
      } catch (restoreError) {
        // Being offline, or the server being down, says nothing about the credentials.
        // Keep the marker and try again at the next unlock.
        if (canRetryRestoredDevice(restoreError)) {
          console.warn('[ImportHandler] Could not re-register restored device (will retry next unlock):', restoreError);
          return;
        }

        console.error('[ImportHandler] Failed to re-register restored device:', restoreError);
        // Clear the invalid data so sync is disabled but app still works
        await syncRepository.updateMetadata({
          apiKey: '',
          syncEnabled: false,
          pendingDeviceName: undefined,
        });

        const errorMessage = restoreError instanceof Error ? restoreError.message : String(restoreError);
        console.warn('[ImportHandler] Sync disabled due to re-registration failure:', errorMessage);
        // Don't throw - let the app work without sync
      }
    }
  } catch (error) {
    console.error('[ImportHandler] ERROR handling restored credentials:', error);
    // Don't throw - this shouldn't prevent unlock
  }
}

/**
 * One-off repair of saved searches stored as plaintext. It must finish before any
 * sync, or they would be pushed as they are. A failure is retried at the next unlock.
 */
async function encryptLegacySavedSearches(): Promise<void> {
  try {
    const current = await settingsRepository.get();
    if (current.savedSearchesEncrypted) {
      return;
    }

    const changed = await savedSearchRepository.encryptLegacyRows();
    await settingsRepository.update({ savedSearchesEncrypted: true });
    if (changed > 0) {
      console.log(`[Unlock] Encrypted ${changed} saved search(es) stored as plaintext`);
    }
  } catch (error) {
    console.warn('[Unlock] Could not encrypt plaintext saved searches (will retry next unlock):', error);
  }
}

/**
 * Lock the application
 */
export function lock(): void {
  keyManager.clearMasterKey();
  sessionStorageService.clear();
  backupSchedulerService.stop();
  searchService.indexNotes([]);
}

/** Display preferences kept outside the vault, for before it is unlocked */
const LOCAL_PREFERENCE_KEYS = ['jottery-theme', 'jottery-locale'];

/**
 * Delete everything this notebook keeps on this device. deleteDB alone leaves a
 * remembered password readable in localStorage, and someone clearing a machine
 * before handing it on has no way of knowing.
 */
export async function deleteAllLocalData(): Promise<void> {
  await passwordStorageService.clear();
  sessionStorageService.clear();
  try {
    LOCAL_PREFERENCE_KEYS.forEach(key => localStorage.removeItem(key));
  } catch (error) {
    console.warn('[Delete] Could not clear local preferences:', error);
  }
  await deleteDB();
}

/**
 * Check if the application is locked
 */
export function isLocked(): boolean {
  return keyManager.isLocked();
}

/**
 * Change the password (enabled by envelope encryption — no re-encryption needed)
 *
 * The local change always stands. `serverKeyUpdated` is false if the server's copy
 * couldn't be updated, in which case the upload is retried at the next unlock.
 */
export async function changePassword(
  currentPassword: string,
  newPassword: string
): Promise<{ serverKeyUpdated: boolean }> {
  if (!isNotesPasswordLongEnough(newPassword)) {
    throw new ValidationError(`Password must be at least ${MIN_NOTES_PASSWORD_LENGTH} characters`, 'newPassword');
  }

  // Ensure we're unlocked to get the master key
  if (isLocked()) {
    await unlock(currentPassword);
  }

  const currentMasterKey = keyManager.getMasterKey();
  if (!currentMasterKey) {
    throw new Error('Cannot change password: application is locked');
  }

  const metadata = await encryptionRepository.getMetadata();
  if (!metadata) {
    throw new Error('No encryption metadata found');
  }

  // Being unlocked proves nothing about whoever is at the keyboard
  if (!(await passwordUnlocksVault(currentPassword, metadata, currentMasterKey.keyBytes))) {
    throw new Error('Incorrect password');
  }

  // If still on legacy, migrate first
  if (!metadata.envelopeVersion) {
    await tryMigrateToEnvelope(currentPassword, currentMasterKey.keyBytes);
  }

  // Re-wrap master key with new password for local storage
  const newDeviceSalt = cryptoService.generateSalt();
  const newDeviceKey = await cryptoService.deriveKey({
    password: newPassword,
    salt: newDeviceSalt,
    iterations: CRYPTO_PBKDF2_ITERATIONS,
    extractable: false,
  });
  const newLocalWrapped = await cryptoService.wrapMasterKey(newDeviceKey, currentMasterKey.keyBytes);

  // Update local encryption metadata
  await encryptionRepository.setMetadata({
    ...metadata,
    envelopeVersion: 1,
    deviceSalt: arrayBufferToBase64(newDeviceSalt),
    localWrappedMaster: JSON.stringify(newLocalWrapped),
    wrappingKdfVersion: CRYPTO_WRAPPING_KDF_VERSION,
    // Clear legacy fields
    salt: undefined,
    iterations: undefined,
  });

  // Re-wrap master key for server (if sync configured)
  let serverKeyUpdated = true;
  const syncMetadata = await syncRepository.getMetadata();
  if (syncMetadata?.syncEnabled && syncMetadata.userId && syncMetadata.apiKey && syncMetadata.syncEndpoint) {
    try {
      await uploadWrappedKey(newPassword, currentMasterKey.keyBytes, { replace: true });
      console.log('[ChangePassword] ✓ Server wrapped key updated');
    } catch (error) {
      console.warn('[ChangePassword] Failed to update server wrapped key:', error);
      // The server still holds a copy under the old password, so a new device would
      // need that one. Without this, unlock would read the mismatch as a change
      // made somewhere else, and never repair it.
      serverKeyUpdated = false;
      await syncRepository.updateMetadata({ serverKeyStale: true });
    }
  }

  console.log('[ChangePassword] ✓ Password changed successfully (no re-encryption needed)');
  return { serverKeyUpdated };
}

/**
 * Check whether a password unlocks the vault held in memory.
 *
 * The password is checked against the local encryption metadata rather than
 * against whatever the caller happens to know, so a mistyped or wrong password
 * (for example the server password) is caught before it is used to wrap
 * anything.
 */
async function passwordUnlocksVault(
  password: string,
  metadata: EncryptionMetadata,
  masterKeyBytes: Uint8Array
): Promise<boolean> {
  try {
    let candidate: Uint8Array;

    if (metadata.envelopeVersion) {
      const deviceKey = await cryptoService.deriveKey({
        password,
        salt: base64ToUint8Array(metadata.deviceSalt!),
        iterations: CRYPTO_PBKDF2_ITERATIONS,
        extractable: false,
      });
      candidate = await cryptoService.unwrapMasterKey(deviceKey, JSON.parse(metadata.localWrappedMaster!));
    } else if (metadata.salt) {
      const legacyKey = await cryptoService.deriveKey({
        password,
        salt: base64ToUint8Array(metadata.salt),
        iterations: metadata.iterations!,
        algorithm: 'PBKDF2',
        extractable: true,
      });
      candidate = new Uint8Array(await crypto.subtle.exportKey('raw', legacyKey));
    } else {
      return false;
    }

    return candidate.length === masterKeyBytes.length && candidate.every((byte, i) => byte === masterKeyBytes[i]);
  } catch {
    return false;
  }
}

/**
 * Upload the master key to the server, wrapped with a key derived from
 * `password` and the user ID. Throws if sync is not configured or the upload fails.
 *
 * `password` must be the notes password. The server sees the account (server)
 * password in plaintext at registration and login, so a blob wrapped with it
 * could be opened by the server operator.
 */
async function uploadWrappedKey(password: string, masterKeyBytes: Uint8Array, write: WrappedKeyWrite): Promise<void> {
  const syncMetadata = await syncRepository.getMetadata();
  if (!syncMetadata?.syncEnabled || !syncMetadata.userId || !syncMetadata.apiKey || !syncMetadata.syncEndpoint) {
    throw new Error('Sync is not configured');
  }

  const currentMasterKey = keyManager.getMasterKey();
  if (!currentMasterKey) {
    throw new Error('Application is locked');
  }

  const wrappingKey = await cryptoService.deriveWrappingKey(password, syncMetadata.userId);
  const serverWrapped = await cryptoService.wrapMasterKey(wrappingKey, masterKeyBytes);

  // Get plaintext API key for auth
  const encryptedApiKey = JSON.parse(syncMetadata.apiKey);
  const plaintextApiKey = await cryptoService.decryptText(encryptedApiKey, currentMasterKey.key);

  await authService.putWrappedKey(syncMetadata.syncEndpoint, plaintextApiKey, {
    blob: JSON.stringify(serverWrapped),
    kdfVersion: CRYPTO_WRAPPING_KDF_VERSION,
    kdfIterations: CRYPTO_WRAPPING_ITERATIONS,
  }, write);

  await syncRepository.updateMetadata({ serverKeyVerified: true, serverKeyStale: false });
}

/** The key changed between reading it and writing, so the write was refused. */
function isWriteConflict(error: unknown): boolean {
  return error instanceof AuthApiError && error.status === 409;
}

/**
 * The condition for overwriting a key this device has read. An older server gives
 * no updatedAt, and sending If-Match there could fail its preflight.
 */
function overwriteOf(existing: WrappedKeyResponse): WrappedKeyWrite {
  return existing.updatedAt ? { ifMatch: existing.updatedAt } : { replace: true };
}

/**
 * Whether the account already holds a wrapped key. If it does, this device isn't
 * the first one, and the blob must not be replaced: PUT is an unconditional upsert,
 * so the account's real key would be lost and every device would onboard to the
 * wrong one.
 */
async function accountHasKey(): Promise<boolean> {
  const syncMetadata = await syncRepository.getMetadata();
  const masterKey = keyManager.getMasterKey();
  if (!syncMetadata?.apiKey || !syncMetadata.syncEndpoint || !masterKey) {
    throw new Error('Sync is not configured');
  }

  const plaintextApiKey = await cryptoService.decryptText(JSON.parse(syncMetadata.apiKey), masterKey.key);
  return (await authService.getWrappedKey(syncMetadata.syncEndpoint, plaintextApiKey)) !== null;
}

/**
 * Convert legacy local encryption metadata to the envelope format.
 * The master key bytes are unchanged; only the way they are stored differs.
 */
async function convertLocalToEnvelope(password: string, masterKeyBytes: Uint8Array): Promise<void> {
  const deviceSalt = cryptoService.generateSalt();
  const deviceKey = await cryptoService.deriveKey({
    password,
    salt: deviceSalt,
    iterations: CRYPTO_PBKDF2_ITERATIONS,
    extractable: false,
  });
  const localWrapped = await cryptoService.wrapMasterKey(deviceKey, masterKeyBytes);

  await encryptionRepository.setMetadata({
    envelopeVersion: 1,
    deviceSalt: arrayBufferToBase64(deviceSalt),
    localWrappedMaster: JSON.stringify(localWrapped),
    wrappingKdfVersion: CRYPTO_WRAPPING_KDF_VERSION,
    createdAt: new Date().toISOString(),
    algorithm: 'AES-256-GCM',
  });
}

/**
 * Publish this device's master key to the server so other devices can onboard.
 *
 * Call this when sync is first set up on a device that already holds the
 * vault. `notesPassword` must be the password that unlocks this vault; it is
 * checked first, so the server password cannot be used by mistake. The local
 * unlock password is never changed. A legacy vault is also converted to the
 * envelope format.
 *
 * Unlike the unlock-time migration, failures are thrown so the caller can show
 * them: a swallowed failure here leaves the server without a key and every
 * other device unable to connect.
 */
export async function uploadWrappedKeyToServer(notesPassword: string): Promise<void> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new Error('Application is locked');
  }

  const metadata = await encryptionRepository.getMetadata();
  if (!metadata) {
    throw new Error('No encryption metadata found');
  }

  if (!(await passwordUnlocksVault(notesPassword, metadata, masterKey.keyBytes))) {
    throw new Error('Incorrect password');
  }

  if (await accountHasKey()) {
    throw new AccountKeyExistsError();
  }

  try {
    await uploadWrappedKey(notesPassword, masterKey.keyBytes, { replace: false });
  } catch (error) {
    throw isWriteConflict(error) ? new AccountKeyExistsError() : error;
  }

  if (!metadata.envelopeVersion) {
    await convertLocalToEnvelope(notesPassword, masterKey.keyBytes);
  }
}

/**
 * Check whether a password is the one that unlocks the vault currently in memory.
 * Used to catch a mistyped notes password (or the server password) up front.
 */
export async function verifyUnlockPassword(password: string): Promise<boolean> {
  const masterKey = keyManager.getMasterKey();
  const metadata = await encryptionRepository.getMetadata();
  if (!masterKey || !metadata) {
    return false;
  }
  return passwordUnlocksVault(password, metadata, masterKey.keyBytes);
}

/**
 * Make sure the server holds a copy of the master key that the notes password
 * opens. Runs after unlock, when the notes password is to hand.
 *
 * This repairs two kinds of account:
 * - The original upload failed or was skipped, so the server has no copy.
 * - An older client wrapped the key with the account password. New devices refuse
 *   those, so without this repair they could never connect.
 *
 * A blob that opens with the notes password is never touched, even if it holds a
 * different key: that means this device has diverged, and overwriting would damage
 * the account.
 *
 * A blob that won't open has three possible causes, and only one is safe to repair:
 * - A password change here never reached the server. Finish it.
 * - This device has opened or uploaded the blob before, so it isn't old-format. The
 *   password must have changed on another device, and overwriting would undo that
 *   and start the two devices taking turns. Leave it, and tell the user.
 * - This device has never opened it. If it has synced it holds the account's real
 *   key, so it may repair the blob. Doing so sets the flag, so that happens only once.
 *
 * Every write here is conditional on what was read, so a change made by another
 * device in between can't be overwritten: that is the one race the flag can't close.
 *
 * All failures (including being offline) are logged and ignored.
 */
export async function ensureServerWrappedKey(notesPassword: string): Promise<void> {
  try {
    const syncMetadata = await syncRepository.getMetadata();
    if (!syncMetadata?.syncEnabled || !syncMetadata.userId || !syncMetadata.apiKey || !syncMetadata.syncEndpoint) {
      return;
    }

    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      return;
    }

    const plaintextApiKey = await cryptoService.decryptText(JSON.parse(syncMetadata.apiKey), masterKey.key);
    const existing = await authService.getWrappedKey(syncMetadata.syncEndpoint, plaintextApiKey);

    if (!existing) {
      console.log('[Unlock] Server has no wrapped key for this account, uploading one...');
      await uploadWrappedKey(notesPassword, masterKey.keyBytes, { replace: false });
      console.log('[Unlock] ✓ Wrapped key uploaded to server');
      return;
    }

    if (await unwrapServerBlob(existing, notesPassword, syncMetadata.userId)) {
      if (!syncMetadata.serverKeyVerified || syncMetadata.serverKeyStale) {
        await syncRepository.updateMetadata({ serverKeyVerified: true, serverKeyStale: false });
      }
      return;
    }

    const finishingPasswordChange = !!syncMetadata.serverKeyStale;
    if (!finishingPasswordChange) {
      if (syncMetadata.serverKeyVerified) {
        toast.info(get(_)('unlock.passwordChangedElsewhere'));
        return;
      }
      if (!syncMetadata.lastSyncAt) {
        return;
      }
    }

    console.log('[Unlock] Server key does not open with the notes password, re-wrapping it...');
    try {
      await uploadWrappedKey(notesPassword, masterKey.keyBytes, overwriteOf(existing));
      console.log('[Unlock] ✓ Wrapped key uploaded to server');
    } catch (error) {
      if (!isWriteConflict(error)) {
        throw error;
      }

      // Something changed the key while this ran. That was another device, so the key
      // isn't old-format, and this device mustn't repair it at a later unlock either.
      // Its own password change stays marked instead, and is retried.
      if (!finishingPasswordChange) {
        await syncRepository.updateMetadata({ serverKeyVerified: true });
        toast.info(get(_)('unlock.passwordChangedElsewhere'));
      }
    }
  } catch (error) {
    console.warn('[Unlock] Could not check or upload the server wrapped key (will retry next unlock):', error);
  }
}

/**
 * Migrate from legacy (direct PBKDF2) to envelope encryption.
 * The master key bytes are the same — just stored differently.
 *
 * `password` must be the password that unlocks this vault.
 */
export async function tryMigrateToEnvelope(password: string, masterKeyBytes: Uint8Array): Promise<void> {
  try {
    const syncMetadata = await syncRepository.getMetadata();

    // Only migrate if sync is configured and we have a userId
    if (!syncMetadata?.syncEnabled || !syncMetadata.userId || !syncMetadata.apiKey || !syncMetadata.syncEndpoint) {
      console.log('[Migration] Skipping envelope migration: sync not configured or no userId');
      return;
    }

    if (!keyManager.getMasterKey()) {
      console.log('[Migration] Skipping: no master key in memory');
      return;
    }

    console.log('[Migration] Starting legacy → envelope migration...');

    if (await accountHasKey()) {
      console.log('[Migration] Account already has a wrapped key, leaving it in place');
    } else {
      try {
        await uploadWrappedKey(password, masterKeyBytes, { replace: false });
        console.log('[Migration] ✓ Wrapped key uploaded to server');
      } catch (error) {
        if (!isWriteConflict(error)) {
          throw error;
        }
        console.log('[Migration] Another device uploaded a key first, leaving it in place');
      }
    }

    await convertLocalToEnvelope(password, masterKeyBytes);

    console.log('[Migration] ✓ Local metadata migrated to envelope format');
    console.log('[Migration] ✓✓✓ Migration complete!');
  } catch (error) {
    console.warn('[Migration] Envelope migration failed (non-fatal, will retry next unlock):', error);
    // Non-fatal — legacy path still works
  }
}

/**
 * Open a blob from the server with a password, or return null if it won't open.
 * This must only ever be given the notes password. The server receives the account
 * password in plaintext, so it could wrap a key of its own choosing under that one.
 */
async function unwrapServerBlob(wrapped: WrappedKeyResponse, password: string, userId: string): Promise<Uint8Array | null> {
  const iterations = serverWrappingIterations(wrapped);
  if (iterations === null) {
    return null;
  }

  try {
    const serverWrapped: EncryptionResult = JSON.parse(wrapped.blob);
    const wrappingKey = await cryptoService.deriveWrappingKey(password, userId, iterations);
    return await cryptoService.unwrapMasterKey(wrappingKey, serverWrapped);
  } catch {
    return null;
  }
}

/**
 * The iterations a blob says it was wrapped with, or null if they can't be accepted.
 * A later client may raise them, and this one must still open that blob. They are the
 * server's word, though, so an unknown scheme is refused, and so is anything below
 * what clients use or high enough to hang the app. Wrapping never uses this value.
 */
function serverWrappingIterations({ kdfVersion, kdfIterations }: WrappedKeyResponse): number | null {
  if (kdfVersion !== CRYPTO_WRAPPING_KDF_VERSION || !Number.isInteger(kdfIterations)) {
    return null;
  }
  return kdfIterations >= CRYPTO_WRAPPING_ITERATIONS && kdfIterations <= CRYPTO_WRAPPING_MAX_ITERATIONS
    ? kdfIterations
    : null;
}

async function noteOpensWith(note: Note, key: CryptoKey): Promise<boolean> {
  try {
    await cryptoService.decryptText(JSON.parse(note.content), key);
    return true;
  } catch {
    return false;
  }
}

/**
 * Refuse a key that would strand the notes on this device. The first sync after
 * connecting pushes everything, so they would reach the account as junk.
 * A note no key can open is already lost, and doesn't stop anyone connecting.
 */
async function assertLocalNotesSurvive(newKey: CryptoKey, newKeyBytes: Uint8Array): Promise<void> {
  const current = keyManager.getMasterKey();
  if (!current || current.keyBytes.every((byte, index) => byte === newKeyBytes[index])) {
    return;
  }

  for (const note of await noteRepository.getAll()) {
    if (await noteOpensWith(note, newKey)) continue;
    if (await noteOpensWith(note, current.key)) {
      throw new LocalNotesOrphanedError();
    }
  }
}

/**
 * Onboard a new device by downloading the wrapped master key from the server.
 * Called during sync setup when the user provides email + password.
 *
 * Only the notes password may unwrap the blob. Nothing is written locally unless
 * it works, and unless the notes already on this device would survive the change.
 */
export async function onboardFromServer(
  password: string,
  endpoint: string,
  userId: string,
  apiKey: string,
  localPassword?: string
): Promise<void> {
  console.log('[Onboard] Downloading wrapped key from server...');

  // Get the wrapped key from the server
  const wrappedKeyResponse = await authService.getWrappedKey(endpoint, apiKey);
  if (!wrappedKeyResponse) {
    throw new Error('No wrapped key found on server. The source device may not have migrated to envelope encryption yet.');
  }

  const masterKeyBytes = await unwrapServerBlob(wrappedKeyResponse, password, userId);
  if (!masterKeyBytes) {
    throw new AccountKeyUnreadableError();
  }
  console.log('[Onboard] ✓ Master key unwrapped from server blob');

  // Import as CryptoKey
  const key = await crypto.subtle.importKey(
    'raw',
    masterKeyBytes as Uint8Array<ArrayBuffer>,
    { name: 'AES-GCM', length: 256 },
    true,
    ['encrypt', 'decrypt']
  );

  await assertLocalNotesSurvive(key, masterKeyBytes);

  // Create local envelope storage
  const deviceSalt = cryptoService.generateSalt();
  const deviceKey = await cryptoService.deriveKey({
    password: localPassword ?? password,
    salt: deviceSalt,
    iterations: CRYPTO_PBKDF2_ITERATIONS,
    extractable: false,
  });
  const localWrapped = await cryptoService.wrapMasterKey(deviceKey, masterKeyBytes);

  await encryptionRepository.setMetadata({
    envelopeVersion: 1,
    deviceSalt: arrayBufferToBase64(deviceSalt),
    localWrappedMaster: JSON.stringify(localWrapped),
    wrappingKdfVersion: CRYPTO_WRAPPING_KDF_VERSION,
    createdAt: new Date().toISOString(),
    algorithm: 'AES-256-GCM',
  });

  // Store the master key
  const masterKey: MasterKey = {
    key,
    keyBytes: masterKeyBytes,
    derivedAt: Date.now(),
  };
  keyManager.setMasterKey(masterKey);

  // Written with the account it belongs to, because the proof is only about that one
  await syncRepository.updateMetadata({ syncEndpoint: endpoint, userId, serverKeyVerified: true, serverKeyStale: false });

  console.log('[Onboard] ✓ Device onboarded via envelope encryption');
}

/**
 * Restore from an encrypted backup
 *
 * This function:
 * 1. Verifies the password against the backup's encryption metadata
 * 2. Restores all data from the backup (decrypting each record)
 * 3. Unlocks the application with the verified key
 *
 * @param backup - The validated backup data
 * @param password - The password used when the backup was created
 * @param onProgress - Optional progress callback
 */
export async function restoreFromBackup(
  backup: BackupData,
  password: string,
  onProgress?: (progress: {
    phase: 'validating' | 'decrypting' | 'restoring' | 'complete';
    current?: number;
    total?: number;
  }) => void
): Promise<void> {
  console.log('[RestoreFromBackup] Starting restore process...');

  // Log backup summary for debugging
  if ('summary' in backup) {
    console.log('[RestoreFromBackup] Backup summary:', backup.summary);
  }

  // Step 1: Verify password against backup
  onProgress?.({ phase: 'validating' });
  console.log('[RestoreFromBackup] Verifying password...');

  const verification = await verifyBackupPassword(backup, password);
  if (!verification.valid || !verification.key || !verification.keyBytes) {
    console.error('[RestoreFromBackup] Password verification failed:', verification.error);
    throw new Error(verification.error || 'Incorrect password');
  }

  console.log('[RestoreFromBackup] Password verified successfully');

  // Everything below replaces the vault, so the whole backup is checked first
  await verifyBackupContents(backup, verification.keyBytes, onProgress);
  console.log('[RestoreFromBackup] Backup contents verified');

  // Disable sync during restore to prevent interference
  syncService.setRestoreInProgress(true);

  try {
    // Step 2: Delete and recreate database to fully free space
    // Using deleteDB instead of clearAllStores because IndexedDB doesn't
    // immediately reclaim space after clear() - only after deleteDatabase()
    console.log('[RestoreFromBackup] Deleting existing database to free space...');
    try {
      await deleteDB();
      console.log('[RestoreFromBackup] Database deleted, recreating...');
      await initDB();
      console.log('[RestoreFromBackup] Database recreated');
    } catch (error) {
      console.warn('[RestoreFromBackup] Failed to delete/recreate database:', error);
      // Fallback to clearing stores
      console.log('[RestoreFromBackup] Falling back to clearing stores...');
      await clearAllStores();
    }

    // Step 3: Store the master key BEFORE restoring (repositories need it for encryption)
    const masterKey: MasterKey = {
      key: verification.key,
      keyBytes: verification.keyBytes,
      derivedAt: Date.now(),
    };

    keyManager.setMasterKey(masterKey);
    console.log('[RestoreFromBackup] Master key stored in keyManager');

    // Step 4: Restore all data from backup (passing the keyBytes for JWE decryption)
    console.log('[RestoreFromBackup] Restoring data...');
    try {
      await restoreBackupData(backup, verification.keyBytes, onProgress);
    } catch (error) {
      // The backup is known to be good, so this is storage giving out. Go back to an
      // empty vault: part of one could be mistaken for the real thing, whereas the
      // backup file can simply be restored again.
      keyManager.clearMasterKey();
      await clearAllStores();
      throw error;
    }
    await encryptLegacySavedSearches();

    // Step 5: Log restored sync configuration for debugging
    try {
      const syncMetadata = await syncRepository.getMetadata();
      if (syncMetadata?.syncEnabled) {
        console.log('[RestoreFromBackup] Restored sync configuration:');
        console.log('  - Sync enabled:', syncMetadata.syncEnabled);
        console.log('  - Sync endpoint:', syncMetadata.syncEndpoint || '(not set)');
        console.log('  - Client ID:', syncMetadata.clientId || '(not set)');
        console.log('  - Has API key:', !!syncMetadata.apiKey);
      }
    } catch (error) {
      console.warn('[RestoreFromBackup] Could not read sync metadata:', error);
    }
  } finally {
    // Re-enable sync after restore completes (or fails)
    syncService.setRestoreInProgress(false);
  }

  // Start backup scheduler
  backupSchedulerService.start();
  console.log('[RestoreFromBackup] Backup scheduler started');

  console.log('[RestoreFromBackup] Restore complete!');
}
