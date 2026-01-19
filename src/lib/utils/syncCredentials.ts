/**
 * Sync Credentials Utilities
 *
 * Handles exporting and importing sync credentials for device setup.
 * Supports both legacy (unencrypted) and modern (encrypted) formats.
 *
 * Extracted from SettingsModal for better testability and reusability.
 */

import { cryptoService, keyManager, syncRepository, encryptionRepository } from '../services';

export interface SyncCredentials {
  endpoint: string;
  clientId: string;
  apiKey: string;
}

export interface LegacyCredentials extends SyncCredentials {
  salt: string;
}

export interface ExportResult {
  success: boolean;
  credentials?: string;
  error?: string;
}

export interface ImportResult {
  success: boolean;
  isEncrypted: boolean;
  salt?: string;
  error?: string;
}

/**
 * Export sync credentials as a portable string
 *
 * Modern format: jottery:v1:<salt>.<encrypted_payload>
 * Legacy format: base64(JSON) with salt included (for older Jottery versions)
 */
export async function exportCredentials(useLegacyFormat: boolean = false): Promise<ExportResult> {
  try {
    // Get encryption key
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      return { success: false, error: 'Application is locked' };
    }

    // Get sync metadata
    const metadata = await syncRepository.getMetadata();
    if (!metadata || !metadata.apiKey || !metadata.clientId) {
      return { success: false, error: 'Sync not configured' };
    }

    // Get encryption metadata (for salt)
    const encryptionMeta = await encryptionRepository.getMetadata();
    if (!encryptionMeta) {
      return { success: false, error: 'Encryption not initialized' };
    }

    // Decrypt API key
    const encryptedApiKey = JSON.parse(metadata.apiKey);
    const apiKey = await cryptoService.decryptText(encryptedApiKey, masterKey.key);

    let credentials: string;

    if (useLegacyFormat) {
      // Legacy format: plain base64 JSON with salt inside (for older Jottery versions)
      const legacyCredentials: LegacyCredentials = {
        endpoint: metadata.syncEndpoint || '',
        clientId: metadata.clientId,
        apiKey: apiKey,
        salt: encryptionMeta.salt,
      };
      credentials = btoa(JSON.stringify(legacyCredentials));
    } else {
      // Encrypted format: jottery:v1:<salt_base64>.<encrypted_payload_base64>
      const syncCredentials: SyncCredentials = {
        endpoint: metadata.syncEndpoint || '',
        clientId: metadata.clientId,
        apiKey: apiKey,
      };
      const json = JSON.stringify(syncCredentials);
      const encrypted = await cryptoService.encryptText(json, masterKey.key);
      const encryptedJson = JSON.stringify(encrypted);
      credentials = `jottery:v1:${encryptionMeta.salt}.${btoa(encryptedJson)}`;
    }

    return { success: true, credentials };
  } catch (error) {
    console.error('Failed to export credentials:', error);
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Failed to export credentials',
    };
  }
}

/**
 * Parse and validate imported credentials
 *
 * This function parses the credentials and stores them for deferred decryption.
 * The actual decryption happens when the user unlocks the app with their password.
 *
 * Returns information about the import but does NOT enable sync yet.
 */
export async function parseAndStoreImportedCredentials(input: string): Promise<ImportResult> {
  try {
    const trimmedInput = input.trim();

    if (!trimmedInput) {
      return { success: false, isEncrypted: false, error: 'Please paste the credentials' };
    }

    // Check for encrypted format: jottery:v1:<salt>.<encrypted_payload>
    if (trimmedInput.startsWith('jottery:v1:')) {
      return await handleEncryptedFormat(trimmedInput);
    } else {
      return await handleLegacyFormat(trimmedInput);
    }
  } catch (error) {
    console.error('Failed to import credentials:', error);
    return {
      success: false,
      isEncrypted: false,
      error: error instanceof Error ? error.message : 'Failed to import credentials',
    };
  }
}

/**
 * Handle encrypted credentials format
 */
async function handleEncryptedFormat(input: string): Promise<ImportResult> {
  const payload = input.substring('jottery:v1:'.length);
  const dotIndex = payload.indexOf('.');

  if (dotIndex === -1) {
    return { success: false, isEncrypted: true, error: 'Invalid encrypted credentials format' };
  }

  const salt = payload.substring(0, dotIndex);
  const encryptedPayload = payload.substring(dotIndex + 1);

  // Store the encrypted payload for deferred decryption after unlock
  await encryptionRepository.setMetadata({
    salt: salt,
    iterations: 100000,
    createdAt: new Date().toISOString(),
    algorithm: 'AES-256-GCM',
  });

  await syncRepository.updateMetadata({
    syncEnabled: false,
    // Store encrypted payload with marker - will be decrypted on unlock
    apiKey: `ENCRYPTED:${encryptedPayload}`,
  });

  return { success: true, isEncrypted: true, salt };
}

/**
 * Handle legacy (unencrypted) credentials format
 */
async function handleLegacyFormat(input: string): Promise<ImportResult> {
  let credentials: LegacyCredentials;

  try {
    const json = atob(input);
    credentials = JSON.parse(json);
  } catch {
    return { success: false, isEncrypted: false, error: 'Invalid credentials format' };
  }

  // Validate structure - legacy format includes salt
  if (!credentials.endpoint || !credentials.clientId || !credentials.apiKey) {
    return { success: false, isEncrypted: false, error: 'Invalid credentials format - missing required fields' };
  }

  if (!credentials.salt) {
    return { success: false, isEncrypted: false, error: 'Invalid credentials format - missing salt (legacy format)' };
  }

  // Store encryption metadata
  await encryptionRepository.setMetadata({
    salt: credentials.salt,
    iterations: 100000,
    createdAt: new Date().toISOString(),
    algorithm: 'AES-256-GCM',
  });

  // Store sync metadata with IMPORT marker for processing on unlock
  await syncRepository.updateMetadata({
    clientId: credentials.clientId,
    syncEndpoint: credentials.endpoint,
    syncEnabled: false,
    apiKey: `IMPORT:${credentials.apiKey}`,
  });

  return { success: true, isEncrypted: false, salt: credentials.salt };
}

/**
 * Copy text to clipboard with error handling
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (error) {
    console.warn('Clipboard copy failed:', error);
    return false;
  }
}
