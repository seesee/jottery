/**
 * Encrypted backup and restore service (v2.1)
 *
 * Creates fully encrypted backups where each record is a JWE (JSON Web Encryption)
 * token using RFC 7516 standard. This provides:
 * - Standardized encryption format
 * - Self-describing algorithm headers
 * - Interoperability with other JWE-compatible tools
 * - Future-proofing for features like note sharing
 *
 * Format:
 * {
 *   version: "2.1",
 *   type: "jottery-encrypted-backup",
 *   createdAt: "...",
 *   encryption: { salt, iterations, algorithm },
 *   data: [
 *     "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..IV.ciphertext.tag",
 *     ...
 *   ]
 * }
 *
 * Each JWE decrypts to: { type: "note"|"attachment"|..., data: {...} }
 */

import { CompactEncrypt, compactDecrypt } from 'jose';
import type {
  Note,
  NoteVersion,
  SavedSearch,
  UserSettings,
  EncryptionMetadata,
  SyncMetadata,
} from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { settingsRepository } from './settingsRepository';
import { encryptionRepository } from './encryptionRepository';
import { syncRepository } from './syncRepository';
import { getDB, STORES } from './db';
import { cryptoService } from './crypto';
import { keyManager } from './keyManager';
import { base64ToUint8Array } from '../utils/base64';

const BACKUP_VERSION = '2.1';
const BACKUP_TYPE = 'jottery-encrypted-backup';
const BACKUP_FILE_EXTENSION = '.jottery-backup';

// JWE algorithms
const JWE_ALG = 'dir'; // Direct encryption (no key wrapping)
const JWE_ENC = 'A256GCM'; // AES-256-GCM content encryption

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
 * Backup file structure (v2.1 - JWE format)
 */
export interface BackupData {
  version: string;
  type: typeof BACKUP_TYPE;
  createdAt: string;
  encryption: EncryptionMetadata;
  data: string[]; // Array of JWE compact serialization strings
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
 * Export CryptoKey as raw bytes for use with jose
 */
async function exportKeyAsBytes(key: CryptoKey): Promise<Uint8Array> {
  const rawKey = await crypto.subtle.exportKey('raw', key);
  return new Uint8Array(rawKey);
}

/**
 * Encrypt a record as a JWE compact serialization string
 */
async function encryptRecordAsJWE(record: BackupRecord, keyBytes: Uint8Array): Promise<string> {
  const plaintext = new TextEncoder().encode(JSON.stringify(record));

  const jwe = await new CompactEncrypt(plaintext)
    .setProtectedHeader({ alg: JWE_ALG, enc: JWE_ENC })
    .encrypt(keyBytes);

  return jwe;
}

/**
 * Decrypt a JWE compact serialization string to a record
 */
async function decryptJWEToRecord(jwe: string, keyBytes: Uint8Array): Promise<BackupRecord> {
  const { plaintext } = await compactDecrypt(jwe, keyBytes);
  const json = new TextDecoder().decode(plaintext);
  return JSON.parse(json);
}

/**
 * Progress callback for backup creation
 */
export type BackupProgressCallback = (progress: {
  phase: 'counting' | 'encrypting' | 'complete';
  current?: number;
  total?: number;
  item?: string;
}) => void;

/**
 * Create an encrypted backup of all data
 *
 * Each record (note, attachment, version, etc.) is encrypted as a JWE token.
 * The backup file only reveals the record count and encryption parameters.
 */
export async function createBackup(onProgress?: BackupProgressCallback): Promise<BackupData> {
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

  // Use pre-exported key bytes from keyManager
  const keyBytes = masterKey.keyBytes;

  onProgress?.({ phase: 'counting' });

  // Count total items first
  const notes = await noteRepository.getAll();
  const attachmentIds = await attachmentRepository.listAllIds();
  const db = getDB();
  const versions = await db.getAll(STORES.NOTE_VERSIONS);
  let savedSearches: unknown[] = [];
  try {
    savedSearches = await db.getAll(STORES.SAVED_SEARCHES);
  } catch {
    console.warn('[backupService] Could not read saved searches');
  }

  const total = notes.length + attachmentIds.length + versions.length + savedSearches.length + 2; // +2 for settings and sync metadata
  let current = 0;

  const jweRecords: string[] = [];

  // Encrypt all notes
  for (const note of notes) {
    const record: BackupRecord = { type: 'note', data: note };
    const jwe = await encryptRecordAsJWE(record, keyBytes);
    jweRecords.push(jwe);
    current++;
    onProgress?.({ phase: 'encrypting', current, total, item: 'notes' });
  }

  // Encrypt all attachments with their blob data
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
      const jwe = await encryptRecordAsJWE(record, keyBytes);
      jweRecords.push(jwe);
    }
    current++;
    onProgress?.({ phase: 'encrypting', current, total, item: 'attachments' });
  }

  // Encrypt all note versions
  for (const version of versions) {
    const record: BackupRecord = { type: 'version', data: version };
    const jwe = await encryptRecordAsJWE(record, keyBytes);
    jweRecords.push(jwe);
    current++;
    onProgress?.({ phase: 'encrypting', current, total, item: 'versions' });
  }

  // Encrypt saved searches
  for (const savedSearch of savedSearches) {
    const record: BackupRecord = { type: 'saved_search', data: savedSearch };
    const jwe = await encryptRecordAsJWE(record, keyBytes);
    jweRecords.push(jwe);
    current++;
    onProgress?.({ phase: 'encrypting', current, total, item: 'searches' });
  }

  // Encrypt settings
  const settings = await settingsRepository.get();
  const settingsRecord: BackupRecord = { type: 'settings', data: settings };
  const settingsJwe = await encryptRecordAsJWE(settingsRecord, keyBytes);
  jweRecords.push(settingsJwe);
  current++;
  onProgress?.({ phase: 'encrypting', current, total, item: 'settings' });

  // Encrypt sync metadata (if present)
  const syncMetadata = await syncRepository.getMetadata();
  if (syncMetadata) {
    const syncRecord: BackupRecord = { type: 'sync_metadata', data: syncMetadata };
    const jwe = await encryptRecordAsJWE(syncRecord, keyBytes);
    jweRecords.push(jwe);
  }
  current++;
  onProgress?.({ phase: 'encrypting', current, total, item: 'metadata' });

  onProgress?.({ phase: 'complete', current: total, total });

  const backup: BackupData = {
    version: BACKUP_VERSION,
    type: BACKUP_TYPE,
    createdAt: new Date().toISOString(),
    encryption,
    data: jweRecords,
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
 * - Data is an array of JWE strings
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

    // Check version compatibility
    const [major, minor] = backup.version.split('.').map(n => parseInt(n, 10));
    if (major > 2 || (major === 2 && minor > 1)) {
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

    if (!Array.isArray(backup.data)) {
      return {
        valid: false,
        error: 'Invalid backup file. Data section must be an array.',
      };
    }

    // v2.1: data is an array of JWE strings
    if (major === 2 && minor >= 1) {
      for (const record of backup.data) {
        if (typeof record !== 'string') {
          return {
            valid: false,
            error: 'Invalid backup file. Expected JWE tokens as strings.',
          };
        }
        // Basic JWE format check (5 parts separated by dots)
        const parts = record.split('.');
        if (parts.length !== 5) {
          return {
            valid: false,
            error: 'Invalid backup file. Malformed JWE token.',
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
 * to decrypt the first JWE record to verify the password.
 */
export async function verifyBackupPassword(
  backup: BackupData,
  password: string
): Promise<{ valid: boolean; key?: CryptoKey; keyBytes?: Uint8Array; error?: string }> {
  try {
    // Derive key using backup's salt and iterations (extractable for JWE)
    const salt = base64ToUint8Array(backup.encryption.salt);
    const key = await cryptoService.deriveKey({
      password,
      salt,
      iterations: backup.encryption.iterations,
      algorithm: 'PBKDF2',
      extractable: true,
    });

    // Export key as raw bytes for jose
    const keyBytes = await exportKeyAsBytes(key);

    // Verify by attempting to decrypt the first record
    if (backup.data.length > 0) {
      try {
        await decryptJWEToRecord(backup.data[0], keyBytes);
      } catch {
        return {
          valid: false,
          error: 'Incorrect password',
        };
      }
    }

    return { valid: true, key, keyBytes };
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
 * Decrypts each JWE record and restores based on its type.
 *
 * IMPORTANT: This should only be called on a fresh/empty database.
 * Existing data will be overwritten.
 */
export async function restoreBackup(
  backup: BackupData,
  keyBytes: Uint8Array,
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
    const jwe = backup.data[i];

    try {
      const record = await decryptJWEToRecord(jwe, keyBytes);

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

        case 'sync_metadata': {
          // Mark the API key for re-registration so restored device gets a new ID
          const syncMeta = record.data as SyncMetadata;
          if (syncMeta.apiKey && !syncMeta.apiKey.startsWith('RESTORE:')) {
            syncMeta.apiKey = 'RESTORE:' + syncMeta.apiKey;
          }
          await syncRepository.updateMetadata(syncMeta);
          break;
        }

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
 *
 * Uses chunked JSON generation to avoid string length limits on large backups.
 */
export function downloadBackup(backup: BackupData): void {
  // Build JSON in chunks to avoid string length limits
  const chunks: string[] = [];

  // Opening structure (everything except the data array contents)
  chunks.push('{"version":');
  chunks.push(JSON.stringify(backup.version));
  chunks.push(',"type":');
  chunks.push(JSON.stringify(backup.type));
  chunks.push(',"createdAt":');
  chunks.push(JSON.stringify(backup.createdAt));
  chunks.push(',"encryption":');
  chunks.push(JSON.stringify(backup.encryption));
  chunks.push(',"data":[');

  // Add each JWE record individually
  for (let i = 0; i < backup.data.length; i++) {
    if (i > 0) chunks.push(',');
    chunks.push(JSON.stringify(backup.data[i]));
  }

  // Close the structure
  chunks.push(']}');

  const blob = new Blob(chunks, { type: 'application/json' });
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
 * For v2.x backups, we can only show record count (types are encrypted).
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
