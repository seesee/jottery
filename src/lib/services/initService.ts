/**
 * Application initialization service
 * Handles first-time setup and unlock/lock operations
 */

import type { EncryptionMetadata, MasterKey } from '../types';
import { encryptionRepository } from './encryptionRepository';
import { settingsRepository } from './settingsRepository';
import { noteRepository } from './noteRepository';
import { syncRepository } from './syncRepository';
import { cryptoService } from './crypto';
import { keyManager, setupActivityListeners } from './keyManager';
import { sessionStorageService } from './sessionStorageService';
import { authService } from './authService';
import { arrayBufferToBase64, base64ToUint8Array } from '../utils/base64';
import { CRYPTO_PBKDF2_ITERATIONS } from '../constants';

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
    // Fresh installation - generate new salt
    console.log('[Initialize] Fresh installation - generating new encryption metadata');

    const salt = cryptoService.generateSalt();

    const metadata: EncryptionMetadata = {
      salt: arrayBufferToBase64(salt),
      iterations: CRYPTO_PBKDF2_ITERATIONS,
      createdAt: new Date().toISOString(),
      algorithm: 'AES-256-GCM',
    };

    await encryptionRepository.setMetadata(metadata);

    // Derive master key and unlock
    await unlock(password);
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

  console.log('[Unlock] Encryption metadata found, deriving key...');
  console.log('[Unlock] Salt length:', metadata.salt.length, 'Iterations:', metadata.iterations);

  // Derive master key
  const salt = base64ToUint8Array(metadata.salt);
  const key = await cryptoService.deriveKey({
    password,
    salt,
    iterations: metadata.iterations,
    algorithm: 'PBKDF2',
  });

  console.log('[Unlock] ✓ Master key derived');

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

  // Store the master key
  const masterKey: MasterKey = {
    key,
    derivedAt: Date.now(),
  };

  keyManager.setMasterKey(masterKey);
  console.log('[Unlock] ✓ Master key stored in keyManager');

  // Get settings to check rememberPassword
  const settings = await settingsRepository.get();

  // Handle imported credentials (IMPORT: marker)
  console.log('[Unlock] Checking for imported credentials...');
  await handleImportedCredentials(key);

  // Setup auto-lock (disabled if rememberPassword is enabled)
  if (settings.rememberPassword) {
    console.log('[Unlock] ⚠️ Remember Password enabled - auto-lock DISABLED');
  } else {
    setupActivityListeners(settings.autoLockTimeout);
    console.log('[Unlock] ✓ Auto-lock enabled with timeout:', settings.autoLockTimeout, 'minutes');
  }

  console.log('[Unlock] ✓ Unlock complete!');
}

/**
 * Handle imported credentials after successful unlock
 * Detects IMPORT: or ENCRYPTED: marker, calls clone-device to register as new device
 */
async function handleImportedCredentials(masterKey: CryptoKey): Promise<void> {
  try {
    console.log('[ImportHandler] Fetching sync metadata...');
    const metadata = await syncRepository.getMetadata();

    if (!metadata) {
      console.log('[ImportHandler] No sync metadata found - skipping');
      return;
    }

    if (!metadata.apiKey) {
      console.log('[ImportHandler] No API key in metadata - skipping');
      return;
    }

    console.log('[ImportHandler] Sync metadata found:', {
      hasClientId: !!metadata.clientId,
      hasEndpoint: !!metadata.syncEndpoint,
      hasApiKey: !!metadata.apiKey,
      hasPendingDeviceName: !!metadata.pendingDeviceName,
      syncEnabled: metadata.syncEnabled,
    });

    // Get device name (default to 'Imported Device' if not set)
    const deviceName = metadata.pendingDeviceName || 'Imported Device';

    // Check for IMPORT: marker (plaintext API key from legacy unencrypted import)
    if (metadata.apiKey.startsWith('IMPORT:')) {
      console.log('[ImportHandler] ✓ IMPORT marker detected! Processing imported credentials...');

      // Extract plaintext API key
      const plaintextApiKey = metadata.apiKey.substring(7); // Remove "IMPORT:" prefix
      const endpoint = metadata.syncEndpoint;

      if (!endpoint) {
        throw new Error('No sync endpoint found for imported credentials');
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
        pendingDeviceName: undefined, // Clear the pending name
      });
      console.log('[ImportHandler] ✓ Sync metadata updated with new device credentials');

      await settingsRepository.update({
        syncEnabled: true,
      });
      console.log('[ImportHandler] ✓ Settings updated (syncEnabled: true)');

      console.log('[ImportHandler] ✓✓✓ Import credentials processed successfully! Registered as new device.');
    }
    // Check for ENCRYPTED: marker (encrypted credentials from v1 format import)
    else if (metadata.apiKey.startsWith('ENCRYPTED:')) {
      console.log('[ImportHandler] ✓ ENCRYPTED marker detected! Decrypting imported credentials...');

      // Extract encrypted payload (base64 encoded JSON of EncryptedData)
      const encryptedPayload = metadata.apiKey.substring(10); // Remove "ENCRYPTED:" prefix

      try {
        // Decode and parse the encrypted data
        const encryptedJson = atob(encryptedPayload);
        const encryptedData = JSON.parse(encryptedJson);

        // Decrypt the credentials JSON
        const credentialsJson = await cryptoService.decryptText(encryptedData, masterKey);
        const credentials = JSON.parse(credentialsJson);
        console.log('[ImportHandler] ✓ Credentials decrypted successfully');

        // Validate decrypted credentials
        if (!credentials.endpoint || !credentials.apiKey) {
          throw new Error('Invalid decrypted credentials - missing required fields');
        }

        // Call clone-device to register as a NEW device
        console.log('[ImportHandler] Calling clone-device to register new device...');
        const cloneResult = await authService.cloneDevice(credentials.endpoint, credentials.apiKey, deviceName);
        console.log('[ImportHandler] ✓ Clone-device successful, got new clientId:', cloneResult.clientId);

        // Encrypt the NEW API key
        const encryptedApiKey = await cryptoService.encryptText(cloneResult.apiKey, masterKey);
        console.log('[ImportHandler] ✓ New API key encrypted');

        // Update sync metadata with NEW credentials
        await syncRepository.updateMetadata({
          clientId: cloneResult.clientId,
          userId: cloneResult.userId,
          syncEndpoint: credentials.endpoint,
          apiKey: JSON.stringify(encryptedApiKey),
          syncEnabled: true,
          pendingDeviceName: undefined, // Clear the pending name
        });
        console.log('[ImportHandler] ✓ Sync metadata updated with new device credentials');

        await settingsRepository.update({
          syncEndpoint: credentials.endpoint,
          syncEnabled: true,
        });
        console.log('[ImportHandler] ✓ Settings updated (syncEnabled: true)');

        console.log('[ImportHandler] ✓✓✓ Encrypted credentials processed successfully! Registered as new device.');
      } catch (decryptError) {
        console.error('[ImportHandler] Failed to process credentials:', decryptError);
        // Clear the invalid encrypted data so user can try again
        await syncRepository.updateMetadata({
          apiKey: '',
          syncEnabled: false,
          pendingDeviceName: undefined,
        });

        // Provide more specific error message based on where the error occurred
        const errorMessage = decryptError instanceof Error ? decryptError.message : String(decryptError);

        // Check if this is a device registration/API error (not a decryption error)
        const isApiError = errorMessage.includes('Invalid API key') ||
                          errorMessage.includes('device') ||
                          errorMessage.includes('cloning') ||
                          errorMessage.includes('admin approval') ||
                          errorMessage.includes('deactivated') ||
                          errorMessage.includes('401') ||
                          errorMessage.includes('403');

        if (isApiError) {
          throw new Error(`Failed to register device: ${errorMessage}`);
        }
        throw new Error('Failed to decrypt sync credentials. Please ensure you are using the same password as the source device.');
      }
    } else {
      console.log('[ImportHandler] No IMPORT/ENCRYPTED marker - credentials already encrypted');
    }
  } catch (error) {
    console.error('[ImportHandler] ERROR handling imported credentials:', error);
    console.error('[ImportHandler] Stack:', error instanceof Error ? error.stack : 'N/A');
    // Don't throw - this shouldn't prevent unlock
  }
}

/**
 * Lock the application
 */
export function lock(): void {
  keyManager.clearMasterKey();
  sessionStorageService.clear();
}

/**
 * Check if the application is locked
 */
export function isLocked(): boolean {
  return keyManager.isLocked();
}

/**
 * Change the password
 */
export async function changePassword(
  currentPassword: string,
  _newPassword: string
): Promise<void> {
  // Verify current password by unlocking
  if (isLocked()) {
    await unlock(currentPassword);
  }

  // This is a simplified version - in reality, we'd need to:
  // 1. Decrypt all notes with old key
  // 2. Derive new key from new password
  // 3. Re-encrypt all notes with new key
  // 4. Update encryption metadata
  // For now, we'll throw an error as this requires more complex migration

  throw new Error(
    'Password change not yet implemented. This requires re-encrypting all data.'
  );
}
