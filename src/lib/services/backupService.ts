/**
 * Encrypted backup and restore service (v2.0)
 *
 * Creates fully encrypted backups where each record is individually encrypted.
 * The backup file only reveals the encryption parameters and record count -
 * all data (including types, metadata, and content) is encrypted.
 *
 * Format:
 * {
 *   version: "2.0",
 *   type: "jottery-encrypted-backup",
 *   createdAt: "...",
 *   encryption: { salt, iterations, algorithm },
 *   data: [
 *     { iv, ciphertext },  // Each is an encrypted { type, data } object
 *     ...
 *   ]
 * }
 */

import type {
  Note,
  NoteVersion,
  SavedSearch,
  UserSettings,
  EncryptionMetadata,
  SyncMetadata,
  EncryptionResult,
} from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { settingsRepository } from './settingsRepository';
import { encryptionRepository } from './encryptionRepository';
import { syncRepository } from './syncRepository';
import { getDB, STORES } from './db';
import { cryptoService, encryptJSON, decryptJSON } from './crypto';
import { keyManager } from './keyManager';
import { base64ToUint8Array } from '../utils/base64';

const BACKUP_VERSION = '2.0';
const BACKUP_TYPE = 'jottery-encrypted-backup';
const BACKUP_FILE_EXTENSION = '.jottery-backup';

/**
 * Record types stored in the backup
 */
type BackupRecordType =
  | 'note'
  | 'attachment'
  | 'version'
  | 'saved_search'
  | 'settings'
  | 'sync_metadata';

/**
 * Encrypted record wrapper (decrypted form)
 */
interface BackupRecord {
  type: BackupRecordType;
  data: unknown;
}

/**
 * Attachment record data
 */
interface AttachmentRecord {
  id: string;
  blob: string; // Base64 encoded
  thumbnail?: string; // Base64 encoded
}

/**
 * Backup file structure (v2.0)
 */
export interface BackupData {
  version: string;
  type: typeof BACKUP_TYPE;
  createdAt: string;
  encryption: EncryptionMetadata;
  data: EncryptionResult[]; // Array of encrypted records
}

/**
 * Validation result
 */
export interface BackupValidationResult {
  valid: boolean;
  error?: string;
  backup?: BackupData;
}

/**
 * Restore progress callback
 */
export type RestoreProgressCallback = (progress: {
  phase: 'validating' | 'decrypting' | 'restoring' | 'complete';
  current?: number;
  total?: number;
}) => void;

/**
 * Create an encrypted backup of all data
 *
 * Each record (note, attachment, version, etc.) is individually encrypted.
 * The backup file only reveals the record count and encryption parameters.
 */
export async function createBackup(): Promise<BackupData> {
  // Get master key (required - must be unlocked)
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new Error('Application is locked. Please unlock to create a backup.');
  }

  // Get encryption metadata (required)
  const encryption = await encryptionRepository.getMetadata();
  if (!encryption) {
    throw new Error('No encryption metadata found. Application not initialised.');
  }

  const encryptedRecords: EncryptionResult[] = [];

  // Encrypt all notes
  const notes = await noteRepository.getAll();
  for (const note of notes) {
    const record: BackupRecord = { type: 'note', data: note };
    const encrypted = await encryptJSON(record, masterKey.key);
    encryptedRecords.push(encrypted);
  }

  // Encrypt all attachments with their blob data
  const attachmentIds = await attachmentRepository.listAllIds();
  for (const id of attachmentIds) {
    const blob = await attachmentRepository.getBlob(id);
    const thumbnail = await attachmentRepository.getThumbnail(id);

    if (blob) {
      const attachmentData: AttachmentRecord = {
        id,
        blob: arrayBufferToBase64(blob),
        thumbnail: thumbnail ? arrayBufferToBase64(thumbnail) : undefined,
      };
      const record: BackupRecord = { type: 'attachment', data: attachmentData };
      const encrypted = await encryptJSON(record, masterKey.key);
      encryptedRecords.push(encrypted);
    }
  }

  // Encrypt all note versions
  const db = getDB();
  const versions = await db.getAll(STORES.NOTE_VERSIONS);
  for (const version of versions) {
    const record: BackupRecord = { type: 'version', data: version };
    const encrypted = await encryptJSON(record, masterKey.key);
    encryptedRecords.push(encrypted);
  }

  // Encrypt saved searches
  try {
    const savedSearches = await db.getAll(STORES.SAVED_SEARCHES);
    for (const savedSearch of savedSearches) {
      const record: BackupRecord = { type: 'saved_search', data: savedSearch };
      const encrypted = await encryptJSON(record, masterKey.key);
      encryptedRecords.push(encrypted);
    }
  } catch {
    // Store might not exist in older databases
    console.warn('[backupService] Could not read saved searches');
  }

  // Encrypt settings
  const settings = await settingsRepository.get();
  const settingsRecord: BackupRecord = { type: 'settings', data: settings };
  const encryptedSettings = await encryptJSON(settingsRecord, masterKey.key);
  encryptedRecords.push(encryptedSettings);

  // Encrypt sync metadata (if present)
  const syncMetadata = await syncRepository.getMetadata();
  if (syncMetadata) {
    const syncRecord: BackupRecord = { type: 'sync_metadata', data: syncMetadata };
    const encrypted = await encryptJSON(syncRecord, masterKey.key);
    encryptedRecords.push(encrypted);
  }

  const backup: BackupData = {
    version: BACKUP_VERSION,
    type: BACKUP_TYPE,
    createdAt: new Date().toISOString(),
    encryption,
    data: encryptedRecords,
  };

  return backup;
}

/**
 * Validate a backup file structure
 *
 * Checks:
 * - JSON structure is valid
 * - Required fields are present
 * - Version is compatible
 * - Data is an array of encrypted records
 */
export async function validateBackup(file: File): Promise<BackupValidationResult> {
  try {
    // Check file extension
    if (!file.name.endsWith(BACKUP_FILE_EXTENSION) && !file.name.endsWith('.json')) {
      return {
        valid: false,
        error: 'Invalid file type. Expected a .jottery-backup file.',
      };
    }

    // Read file content
    const text = await file.text();
    let backup: BackupData;

    try {
      backup = JSON.parse(text);
    } catch {
      return {
        valid: false,
        error: 'Invalid backup file. Could not parse JSON.',
      };
    }

    // Validate structure
    if (backup.type !== BACKUP_TYPE) {
      return {
        valid: false,
        error: 'Invalid backup file. Not a Jottery encrypted backup.',
      };
    }

    if (!backup.version) {
      return {
        valid: false,
        error: 'Invalid backup file. Missing version.',
      };
    }

    // Check version compatibility (support v1.x and v2.x)
    const [major] = backup.version.split('.');
    const majorVersion = parseInt(major, 10);
    if (majorVersion > 2) {
      return {
        valid: false,
        error: `Backup version ${backup.version} is not supported. Please update Jottery.`,
      };
    }

    if (!backup.encryption) {
      return {
        valid: false,
        error: 'Invalid backup file. Missing encryption metadata.',
      };
    }

    if (!backup.encryption.salt || !backup.encryption.iterations || !backup.encryption.algorithm) {
      return {
        valid: false,
        error: 'Invalid backup file. Incomplete encryption metadata.',
      };
    }

    if (!backup.data) {
      return {
        valid: false,
        error: 'Invalid backup file. Missing data section.',
      };
    }

    // v2.0: data is an array of encrypted records
    if (majorVersion >= 2) {
      if (!Array.isArray(backup.data)) {
        return {
          valid: false,
          error: 'Invalid backup file. Data section must be an array.',
        };
      }

      // Verify each record has the expected encrypted structure
      for (const record of backup.data) {
        if (!record.iv || !record.ciphertext) {
          return {
            valid: false,
            error: 'Invalid backup file. Encrypted records are malformed.',
          };
        }
      }
    }

    return {
      valid: true,
      backup,
    };
  } catch (error) {
    return {
      valid: false,
      error: `Failed to validate backup: ${error instanceof Error ? error.message : 'Unknown error'}`,
    };
  }
}

/**
 * Verify password against a backup
 *
 * Derives the key using the backup's encryption metadata and attempts
 * to decrypt the first record to verify the password.
 */
export async function verifyBackupPassword(
  backup: BackupData,
  password: string
): Promise<{ valid: boolean; key?: CryptoKey; error?: string }> {
  try {
    // Derive key using backup's salt and iterations
    const salt = base64ToUint8Array(backup.encryption.salt);
    const key = await cryptoService.deriveKey({
      password,
      salt,
      iterations: backup.encryption.iterations,
      algorithm: 'PBKDF2',
    });

    // Verify by attempting to decrypt the first record
    if (backup.data.length > 0) {
      try {
        const firstRecord = backup.data[0];
        await decryptJSON<BackupRecord>(firstRecord, key);
      } catch {
        return {
          valid: false,
          error: 'Incorrect password',
        };
      }
    }

    return { valid: true, key };
  } catch (error) {
    return {
      valid: false,
      error: error instanceof Error ? error.message : 'Failed to verify password',
    };
  }
}

/**
 * Restore data from a backup
 *
 * Decrypts each record and restores based on its type.
 *
 * IMPORTANT: This should only be called on a fresh/empty database.
 * Existing data will be overwritten.
 */
export async function restoreBackup(
  backup: BackupData,
  key: CryptoKey,
  onProgress?: RestoreProgressCallback
): Promise<void> {
  const db = getDB();
  const total = backup.data.length;

  onProgress?.({ phase: 'validating' });

  // 1. Restore encryption metadata first
  await encryptionRepository.setMetadata(backup.encryption);

  onProgress?.({ phase: 'decrypting', current: 0, total });

  // 2. Decrypt and restore each record
  for (let i = 0; i < backup.data.length; i++) {
    const encryptedRecord = backup.data[i];

    try {
      const record = await decryptJSON<BackupRecord>(encryptedRecord, key);

      onProgress?.({ phase: 'restoring', current: i + 1, total });

      switch (record.type) {
        case 'note':
          await db.put(STORES.NOTES, record.data as Note);
          break;

        case 'attachment': {
          const attachment = record.data as AttachmentRecord;
          const blob = base64ToArrayBuffer(attachment.blob);
          await attachmentRepository.storeBlob(attachment.id, blob);

          if (attachment.thumbnail) {
            const thumbnail = base64ToArrayBuffer(attachment.thumbnail);
            await attachmentRepository.storeThumbnail(attachment.id, thumbnail);
          }
          break;
        }

        case 'version':
          await db.put(STORES.NOTE_VERSIONS, record.data as NoteVersion);
          break;

        case 'saved_search':
          await db.put(STORES.SAVED_SEARCHES, record.data as SavedSearch);
          break;

        case 'settings':
          await settingsRepository.update(record.data as UserSettings);
          break;

        case 'sync_metadata':
          await syncRepository.updateMetadata(record.data as SyncMetadata);
          break;

        default:
          console.warn(`[backupService] Unknown record type: ${(record as BackupRecord).type}`);
      }
    } catch (error) {
      console.error(`[backupService] Failed to restore record ${i}:`, error);
      throw new Error(`Failed to restore record ${i + 1}: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  }

  onProgress?.({ phase: 'complete' });
}

/**
 * Download a backup as a file
 */
export function downloadBackup(backup: BackupData): void {
  const json = JSON.stringify(backup);
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);

  // Generate filename with date
  const date = new Date().toISOString().split('T')[0]; // YYYY-MM-DD
  const filename = `jottery-backup-${date}${BACKUP_FILE_EXTENSION}`;

  // Create download link
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);

  // Clean up blob URL
  URL.revokeObjectURL(url);
}

/**
 * Get backup file stats without full validation
 *
 * For v2.0 backups, we can only show record count (types are encrypted).
 */
export async function getBackupStats(file: File): Promise<{
  valid: boolean;
  createdAt?: string;
  recordCount?: number;
  error?: string;
}> {
  try {
    const text = await file.text();
    const backup: BackupData = JSON.parse(text);

    if (backup.type !== BACKUP_TYPE) {
      return { valid: false, error: 'Not a Jottery backup file' };
    }

    return {
      valid: true,
      createdAt: backup.createdAt,
      recordCount: Array.isArray(backup.data) ? backup.data.length : 0,
    };
  } catch {
    return { valid: false, error: 'Could not read backup file' };
  }
}

// Helper functions for base64 conversion
function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function base64ToArrayBuffer(base64: string): ArrayBuffer {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

/**
 * Backup service singleton export
 */
export const backupService = {
  createBackup,
  validateBackup,
  verifyBackupPassword,
  restoreBackup,
  downloadBackup,
  getBackupStats,
  BACKUP_FILE_EXTENSION,
};
