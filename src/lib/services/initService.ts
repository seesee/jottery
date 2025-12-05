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

const DEFAULT_ITERATIONS = 100000;

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
  const initialized = await isInitialized();
  if (initialized) {
    throw new Error('Application is already initialized');
  }

  // Generate salt for key derivation
  const salt = cryptoService.generateSalt();

  // Store encryption metadata
  const metadata: EncryptionMetadata = {
    salt: arrayBufferToBase64(salt),
    iterations: DEFAULT_ITERATIONS,
    createdAt: new Date().toISOString(),
    algorithm: 'AES-256-GCM',
  };

  await encryptionRepository.setMetadata(metadata);

  // Derive master key and unlock
  await unlock(password);

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

  // Handle imported credentials (IMPORT: marker)
  console.log('[Unlock] Checking for imported credentials...');
  await handleImportedCredentials(key);

  // Setup auto-lock
  const settings = await settingsRepository.get();
  setupActivityListeners(settings.autoLockTimeout);

  console.log('[Unlock] ✓ Unlock complete! Auto-lock timeout:', settings.autoLockTimeout, 'minutes');
}

/**
 * Handle imported credentials after successful unlock
 * Detects IMPORT: marker, encrypts plaintext API key, enables sync
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
      apiKeyPrefix: metadata.apiKey.substring(0, 20) + '...',
      syncEnabled: metadata.syncEnabled,
    });

    // Check for IMPORT: marker (plaintext API key from import)
    if (metadata.apiKey.startsWith('IMPORT:')) {
      console.log('[ImportHandler] ✓ IMPORT marker detected! Processing imported credentials...');

      // Extract plaintext API key
      const plaintextApiKey = metadata.apiKey.substring(7); // Remove "IMPORT:" prefix
      console.log('[ImportHandler] Plaintext API key length:', plaintextApiKey.length);

      console.log('[ImportHandler] Encrypting API key with master key...');
      const encryptedApiKey = await cryptoService.encryptText(plaintextApiKey, masterKey);
      console.log('[ImportHandler] ✓ API key encrypted');

      console.log('[ImportHandler] Updating sync metadata...');
      await syncRepository.updateMetadata({
        apiKey: JSON.stringify(encryptedApiKey),
        syncEnabled: true,
      });
      console.log('[ImportHandler] ✓ Sync metadata updated (syncEnabled: true)');

      console.log('[ImportHandler] Updating settings repository...');
      await settingsRepository.update({
        syncEnabled: true,
      });
      console.log('[ImportHandler] ✓ Settings updated (syncEnabled: true)');

      console.log('[ImportHandler] ✓✓✓ Import credentials processed successfully! Sync is now enabled.');
    } else {
      console.log('[ImportHandler] No IMPORT marker - credentials already encrypted');
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

/**
 * Helper: Convert ArrayBuffer to Base64
 */
function arrayBufferToBase64(buffer: ArrayBuffer | Uint8Array): string {
  const bytes = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
  let binary = '';
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

/**
 * Helper: Convert Base64 to Uint8Array
 */
function base64ToUint8Array(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
