/**
 * Encrypted backup and restore service
 *
 * Creates encrypted backups that can be safely stored in cloud storage
 * and restored on a fresh install using the same password.
 *
 * Key insight: Notes are already encrypted in IndexedDB. We export them as-is
 * without decrypting. The backup includes the encryption metadata (salt, iterations),
 * so the same password derives the same key on restore.
 */

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
import { base64ToUint8Array } from '../utils/base64';

const BACKUP_VERSION = '1.0';
const BACKUP_TYPE = 'jottery-encrypted-backup';
const BACKUP_FILE_EXTENSION = '.jottery-backup';

/**
 * Stored attachment with encrypted blob data
 */
interface StoredAttachment {
  id: string;
  data: string; // Base64 encoded encrypted blob
  thumbnail?: string; // Base64 encoded encrypted thumbnail
}

/**
 * Backup file structure
 */
export interface BackupData {
  version: string;
  type: typeof BACKUP_TYPE;
  createdAt: string;
  encryption: EncryptionMetadata;
  data: {
    notes: Note[];
    attachments: StoredAttachment[];
    versions: NoteVersion[];
    savedSearches: SavedSearch[];
    settings: UserSettings;
    syncMetadata?: SyncMetadata;
  };
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
  phase: 'validating' | 'notes' | 'attachments' | 'versions' | 'settings' | 'complete';
  current?: number;
  total?: number;
}) => void;

/**
 * Create an encrypted backup of all data
 *
 * The backup contains:
 * - Encryption metadata (salt, iterations, algorithm)
 * - All encrypted notes (as-is from IndexedDB)
 * - All encrypted attachments
 * - Version history
 * - Settings (theme, colours, etc.)
 * - Sync metadata (optional, encrypted API key)
 * - Saved searches
 */
export async function createBackup(): Promise<BackupData> {
  // Get encryption metadata (required)
  const encryption = await encryptionRepository.getMetadata();
  if (!encryption) {
    throw new Error('No encryption metadata found. Application not initialised.');
  }

  // Get all notes (including deleted - for complete backup)
  const notes = await noteRepository.getAll();

  // Get all attachments with their blob data
  const attachmentIds = await attachmentRepository.listAllIds();
  const attachments: StoredAttachment[] = [];

  for (const id of attachmentIds) {
    const blob = await attachmentRepository.getBlob(id);
    const thumbnail = await attachmentRepository.getThumbnail(id);

    if (blob) {
      // Convert ArrayBuffer to base64 for JSON serialisation
      const dataBase64 = arrayBufferToBase64(blob);
      const attachment: StoredAttachment = {
        id,
        data: dataBase64,
      };

      if (thumbnail) {
        attachment.thumbnail = arrayBufferToBase64(thumbnail);
      }

      attachments.push(attachment);
    }
  }

  // Get all note versions
  const db = getDB();
  const versions = await db.getAll(STORES.NOTE_VERSIONS);

  // Get saved searches (encrypted)
  let savedSearches: SavedSearch[] = [];
  try {
    savedSearches = await db.getAll(STORES.SAVED_SEARCHES);
  } catch {
    // Store might not exist in older databases
    console.warn('[backupService] Could not read saved searches');
  }

  // Get settings
  const settings = await settingsRepository.get();

  // Get sync metadata (optional - includes encrypted API key)
  const syncMetadata = await syncRepository.getMetadata();

  const backup: BackupData = {
    version: BACKUP_VERSION,
    type: BACKUP_TYPE,
    createdAt: new Date().toISOString(),
    encryption,
    data: {
      notes,
      attachments,
      versions,
      savedSearches,
      settings,
      syncMetadata: syncMetadata ?? undefined,
    },
  };

  return backup;
}

/**
 * Validate a backup file
 *
 * Checks:
 * - JSON structure is valid
 * - Required fields are present
 * - Version is compatible
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
    const [major] = backup.version.split('.');
    if (parseInt(major, 10) > 1) {
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

    if (!Array.isArray(backup.data.notes)) {
      return {
        valid: false,
        error: 'Invalid backup file. Missing or invalid notes array.',
      };
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
 * to decrypt the first note's content to verify the password.
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

    // If there are notes, verify by attempting to decrypt the first one
    if (backup.data.notes.length > 0) {
      const testNote = backup.data.notes.find(n => n.content && n.content !== '');
      if (testNote && testNote.content) {
        try {
          const encryptedContent = JSON.parse(testNote.content);
          await cryptoService.decryptText(encryptedContent, key);
        } catch {
          return {
            valid: false,
            error: 'Incorrect password',
          };
        }
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
 * This function:
 * 1. Sets the encryption metadata from the backup
 * 2. Restores all notes, attachments, versions, settings
 * 3. Optionally restores sync metadata
 *
 * IMPORTANT: This should only be called on a fresh/empty database.
 * Existing data will be overwritten.
 */
export async function restoreBackup(
  backup: BackupData,
  onProgress?: RestoreProgressCallback
): Promise<void> {
  const db = getDB();

  onProgress?.({ phase: 'validating' });

  // 1. Restore encryption metadata
  await encryptionRepository.setMetadata(backup.encryption);

  // 2. Restore notes
  onProgress?.({ phase: 'notes', current: 0, total: backup.data.notes.length });

  for (let i = 0; i < backup.data.notes.length; i++) {
    const note = backup.data.notes[i];
    await db.put(STORES.NOTES, note);
    onProgress?.({ phase: 'notes', current: i + 1, total: backup.data.notes.length });
  }

  // 3. Restore attachments
  onProgress?.({ phase: 'attachments', current: 0, total: backup.data.attachments.length });

  for (let i = 0; i < backup.data.attachments.length; i++) {
    const attachment = backup.data.attachments[i];

    // Convert base64 back to ArrayBuffer
    const blob = base64ToArrayBuffer(attachment.data);
    await attachmentRepository.storeBlob(attachment.id, blob);

    if (attachment.thumbnail) {
      const thumbnail = base64ToArrayBuffer(attachment.thumbnail);
      await attachmentRepository.storeThumbnail(attachment.id, thumbnail);
    }

    onProgress?.({ phase: 'attachments', current: i + 1, total: backup.data.attachments.length });
  }

  // 4. Restore versions
  onProgress?.({ phase: 'versions', current: 0, total: backup.data.versions.length });

  for (let i = 0; i < backup.data.versions.length; i++) {
    const version = backup.data.versions[i];
    await db.put(STORES.NOTE_VERSIONS, version);
    onProgress?.({ phase: 'versions', current: i + 1, total: backup.data.versions.length });
  }

  // 5. Restore saved searches
  if (backup.data.savedSearches && backup.data.savedSearches.length > 0) {
    for (const savedSearch of backup.data.savedSearches) {
      await db.put(STORES.SAVED_SEARCHES, savedSearch);
    }
  }

  // 6. Restore settings
  onProgress?.({ phase: 'settings' });
  await settingsRepository.update(backup.data.settings);

  // 7. Restore sync metadata (optional)
  // Note: The API key in sync metadata is already encrypted with the user's key
  // and will work after restore since we're using the same encryption metadata
  if (backup.data.syncMetadata) {
    await syncRepository.updateMetadata(backup.data.syncMetadata);
  }

  onProgress?.({ phase: 'complete' });
}

/**
 * Download a backup as a file
 */
export function downloadBackup(backup: BackupData): void {
  const json = JSON.stringify(backup, null, 2);
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
 */
export async function getBackupStats(file: File): Promise<{
  valid: boolean;
  createdAt?: string;
  noteCount?: number;
  attachmentCount?: number;
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
      noteCount: backup.data?.notes?.length ?? 0,
      attachmentCount: backup.data?.attachments?.length ?? 0,
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
