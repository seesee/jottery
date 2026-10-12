/**
 * Application initialization service
 * Handles first-time setup and unlock/lock operations
 */

import type { EncryptionMetadata, MasterKey } from '../types';
import type { BackupData } from './backupService';
import { encryptionRepository } from './encryptionRepository';
import { settingsRepository } from './settingsRepository';
import { noteRepository } from './noteRepository';
import { syncRepository } from './syncRepository';
import { cryptoService } from './crypto';
import { keyManager, setupActivityListeners } from './keyManager';
import { sessionStorageService } from './sessionStorageService';
import { authService } from './authService';
import { arrayBufferToBase64, base64ToUint8Array } from '../utils/base64';
import { CRYPTO_PBKDF2_ITERATIONS, CRYPTO_WRAPPING_ITERATIONS, CRYPTO_WRAPPING_KDF_VERSION } from '../constants';
import type { EncryptionResult } from '../types/crypto';
import { restoreFromBackup as restoreBackupData, verifyBackupPassword } from './backupService';
import { backupSchedulerService } from './backupSchedulerService';
import { clearAllStores, deleteDB, initDB } from './db';
import { syncService } from './syncService';

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

  // Verify the key is correct by attempting to decrypt an existing note
  // This prevents the UI from loading with a wrong password
  const notes = await noteRepository.getAllActive();
  console.log('[Unlock] Active notes count:', notes.length);

  if (notes.length > 0) {
    try {
      console.log('[Unlock] Verifying password by decrypting first note...');
      // Try to decrypt the first note's content as a verification
      const testNote = notes[0];
      const encryptedContent = JSON.parse(testNote.content);
      await cryptoService.decryptText(encryptedContent, key);
      console.log('[Unlock] ✓ Password verified (note decrypted successfully)');
    } catch (error) {
      console.error('[Unlock] Password verification failed:', error);
      throw new Error('Incorrect password');
    }
  } else {
    console.log('[Unlock] ⚠️ No notes to verify password against - skipping verification');
  }

  // Store the master key with bytes for JWE operations
  const masterKey: MasterKey = {
    key,
    keyBytes,
    derivedAt: Date.now(),
  };

  keyManager.setMasterKey(masterKey);

  // Get settings to check rememberPassword
  const settings = await settingsRepository.get();

  // Handle restored credentials (RESTORE: marker from backup restore)
  await handleImportedCredentials(key);

  // Attempt migration from legacy to envelope encryption
  if (!metadata.envelopeVersion) {
    await tryMigrateToEnvelope(password, keyBytes);
  }

  // Make sure other devices can onboard: the server needs a copy of the key.
  // Runs in the background so a slow or offline server never delays unlock.
  void ensureServerWrappedKey(password);

  // Setup auto-lock (disabled if rememberPassword is enabled)
  if (settings.rememberPassword) {
    console.log('[Unlock] ⚠️ Remember Password enabled - auto-lock DISABLED');
  } else {
    setupActivityListeners(settings.autoLockTimeout);
    console.log('[Unlock] ✓ Auto-lock enabled with timeout:', settings.autoLockTimeout, 'minutes');
  }

  // Start backup scheduler
  backupSchedulerService.start();
  console.log('[Unlock] ✓ Backup scheduler started');

  console.log('[Unlock] ✓ Unlock complete!');
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
 * Lock the application
 */
export function lock(): void {
  keyManager.clearMasterKey();
  sessionStorageService.clear();
  backupSchedulerService.stop();
}

/**
 * Check if the application is locked
 */
export function isLocked(): boolean {
  return keyManager.isLocked();
}

/**
 * Change the password (enabled by envelope encryption — no re-encryption needed)
 */
export async function changePassword(
  currentPassword: string,
  newPassword: string
): Promise<void> {
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
  const syncMetadata = await syncRepository.getMetadata();
  if (syncMetadata?.syncEnabled && syncMetadata.userId && syncMetadata.apiKey && syncMetadata.syncEndpoint) {
    try {
      const wrappingKey = await cryptoService.deriveWrappingKey(newPassword, syncMetadata.userId);
      const serverWrapped = await cryptoService.wrapMasterKey(wrappingKey, currentMasterKey.keyBytes);

      // Get plaintext API key for auth
      const encryptedApiKey = JSON.parse(syncMetadata.apiKey);
      const plaintextApiKey = await cryptoService.decryptText(encryptedApiKey, currentMasterKey.key);

      await authService.putWrappedKey(syncMetadata.syncEndpoint, plaintextApiKey, {
        blob: JSON.stringify(serverWrapped),
        kdfVersion: CRYPTO_WRAPPING_KDF_VERSION,
        kdfIterations: CRYPTO_WRAPPING_ITERATIONS,
      });
      console.log('[ChangePassword] ✓ Server wrapped key updated');
    } catch (error) {
      console.warn('[ChangePassword] Failed to update server wrapped key:', error);
      // Non-fatal — local password change still succeeded
    }
  }

  console.log('[ChangePassword] ✓ Password changed successfully (no re-encryption needed)');
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
async function uploadWrappedKey(password: string, masterKeyBytes: Uint8Array): Promise<void> {
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
  });
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

  await uploadWrappedKey(notesPassword, masterKey.keyBytes);

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
 * Make sure the server holds a wrapped copy of the master key, uploading one
 * if it does not. Runs after unlock, when the notes password is to hand.
 *
 * This repairs accounts where the original upload failed or was skipped, which
 * otherwise leaves every other device unable to connect. An existing server
 * copy is never overwritten, and all failures (including being offline) are
 * logged and ignored.
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
    if (existing) {
      return;
    }

    console.log('[Unlock] Server has no wrapped key for this account, uploading one...');
    await uploadWrappedKey(notesPassword, masterKey.keyBytes);
    console.log('[Unlock] ✓ Wrapped key uploaded to server');
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

    await uploadWrappedKey(password, masterKeyBytes);
    console.log('[Migration] ✓ Wrapped key uploaded to server');

    await convertLocalToEnvelope(password, masterKeyBytes);

    console.log('[Migration] ✓ Local metadata migrated to envelope format');
    console.log('[Migration] ✓✓✓ Migration complete!');
  } catch (error) {
    console.warn('[Migration] Envelope migration failed (non-fatal, will retry next unlock):', error);
    // Non-fatal — legacy path still works
  }
}

/**
 * Onboard a new device by downloading the wrapped master key from the server.
 * Called during sync setup when the user provides email + password.
 *
 * The blob is wrapped with the notes password, so `password` should be that.
 * Clients before this fix wrapped it with the server password instead, so
 * `alternatePassword` (the server password) is tried if the first attempt
 * fails to unwrap. Nothing is written locally unless one of them works.
 */
export async function onboardFromServer(
  password: string,
  endpoint: string,
  userId: string,
  apiKey: string,
  localPassword?: string,
  alternatePassword?: string
): Promise<void> {
  console.log('[Onboard] Downloading wrapped key from server...');

  // Get the wrapped key from the server
  const wrappedKeyResponse = await authService.getWrappedKey(endpoint, apiKey);
  if (!wrappedKeyResponse) {
    throw new Error('No wrapped key found on server. The source device may not have migrated to envelope encryption yet.');
  }

  const serverWrapped: EncryptionResult = JSON.parse(wrappedKeyResponse.blob);

  // Try each candidate password; the wrapping key is derived from password + userId
  const candidates = [password];
  if (alternatePassword && alternatePassword !== password) {
    candidates.push(alternatePassword);
  }

  let masterKeyBytes: Uint8Array | undefined;
  let unwrapError: unknown;
  for (const candidate of candidates) {
    try {
      const wrappingKey = await cryptoService.deriveWrappingKey(candidate, userId);
      masterKeyBytes = await cryptoService.unwrapMasterKey(wrappingKey, serverWrapped);
      if (candidate !== password) {
        console.log('[Onboard] Server blob was wrapped with the alternate password (older client)');
      }
      break;
    } catch (error) {
      unwrapError = error;
    }
  }
  if (!masterKeyBytes) {
    throw unwrapError;
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
      try {
        await clearAllStores();
      } catch (clearError) {
        console.warn('[RestoreFromBackup] Failed to clear stores:', clearError);
      }
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
    await restoreBackupData(backup, verification.keyBytes, onProgress);

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

  // Step 6: Setup auto-lock based on restored settings
  const settings = await settingsRepository.get();

  if (settings.rememberPassword) {
    console.log('[RestoreFromBackup] Remember Password enabled - auto-lock DISABLED');
  } else {
    setupActivityListeners(settings.autoLockTimeout);
    console.log('[RestoreFromBackup] Auto-lock enabled with timeout:', settings.autoLockTimeout, 'minutes');
  }

  // Start backup scheduler
  backupSchedulerService.start();
  console.log('[RestoreFromBackup] Backup scheduler started');

  console.log('[RestoreFromBackup] Restore complete!');
}
