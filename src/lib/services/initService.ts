/**
 * Application initialization service
 * Handles first-time setup and unlock/lock operations
 */

import type { EncryptionMetadata, MasterKey } from '../types';
import { encryptionRepository } from './encryptionRepository';
import { settingsRepository } from './settingsRepository';
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
  const metadata = await encryptionRepository.getMetadata();
  if (!metadata) {
    throw new Error('Application not initialized. Please initialize first.');
  }

  // Derive master key
  const salt = base64ToUint8Array(metadata.salt);
  const key = await cryptoService.deriveKey({
    password,
    salt,
    iterations: metadata.iterations,
    algorithm: 'PBKDF2',
  });

  // Verify the key is correct by attempting to decrypt a test value
  // For now, we'll just trust it - could add a verification step later

  // Store the master key
  const masterKey: MasterKey = {
    key,
    derivedAt: Date.now(),
  };

  keyManager.setMasterKey(masterKey);

  // Setup auto-lock
  const settings = await settingsRepository.get();
  setupActivityListeners(settings.autoLockTimeout);
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
