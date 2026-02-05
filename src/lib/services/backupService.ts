/**
 * Encrypted backup and restore service (v2.1 and v3.0)
 *
 * Creates fully encrypted backups where data is encrypted using JWE (JSON Web Encryption)
 * tokens using RFC 7516 standard. This provides:
 * - Standardized encryption format
 * - Self-describing algorithm headers
 * - Interoperability with other JWE-compatible tools
 * - Future-proofing for features like note sharing
 *
 * v2.1 Format (legacy):
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
 * Each JWE decrypts to: { type: "note"|"attachment"|..., data: {...} }
 *
 * v3.0 Format (batched, compressed):
 * {
 *   version: "3.0",
 *   type: "jottery-encrypted-backup",
 *   createdAt: "...",
 *   encryption: { salt, iterations, algorithm },
 *   batches: [
 *     { type: "notes", index: 0, count: 100, data: "JWE..." },
 *     ...
 *   ],
 *   summary: { notes: 250, attachments: 50, versions: 100, savedSearches: 5 }
 * }
 * Each JWE decrypts to compressed JSON array of items.
 */

import { CompactEncrypt, compactDecrypt } from 'jose';
import pako from 'pako';
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

const BACKUP_VERSION_LEGACY = '2.1';
const BACKUP_VERSION = '3.0';
const BACKUP_TYPE = 'jottery-encrypted-backup';
const BACKUP_FILE_EXTENSION = '.jottery-backup';

// Batch sizes for different record types
const NOTE_BATCH_SIZE = 250;
const ATTACHMENT_BATCH_SIZE = 50; // Larger items, smaller batches
const VERSION_BATCH_SIZE = 250;

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
 * Backup file structure (v2.1 - JWE format, legacy)
 */
export interface BackupDataV2 {
  version: string;
  type: typeof BACKUP_TYPE;
  createdAt: string;
  encryption: EncryptionMetadata;
  data: string[]; // Array of JWE compact serialization strings
}

/**
 * Batch record in v3.0 format
 */
export interface BatchRecord {
  type: 'notes' | 'attachments' | 'versions' | 'saved_searches' | 'settings' | 'sync_metadata';
  index: number;
  count: number;
  data: string; // JWE compact serialization of compressed batch
}

/**
 * Summary of backup contents
 */
export interface BackupSummary {
  notes: number;
  attachments: number;
  versions: number;
  savedSearches: number;
}

/**
 * Backup file structure (v3.0 - batched, compressed)
 */
export interface BackupDataV3 {
  version: string;
  type: typeof BACKUP_TYPE;
  createdAt: string;
  encryption: EncryptionMetadata;
  batches: BatchRecord[];
  summary: BackupSummary;
}

/**
 * Union type for backup data (supports both versions)
 */
export type BackupData = BackupDataV2 | BackupDataV3;

/**
 * Type guard to check if backup is v3.0 format
 */
function isBackupV3(backup: BackupData): backup is BackupDataV3 {
  return 'batches' in backup && Array.isArray(backup.batches);
}

/**
 * Type guard to check if backup is v2.x format
 */
function isBackupV2(backup: BackupData): backup is BackupDataV2 {
  return 'data' in backup && Array.isArray(backup.data);
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

// ============================================================================
// v3.0 Batched Backup - Compression Utilities
// ============================================================================

/**
 * Compress a JSON string using pako deflate
 */
function compressData(jsonString: string): Uint8Array {
  const textEncoder = new TextEncoder();
  const bytes = textEncoder.encode(jsonString);
  return pako.deflate(bytes);
}

/**
 * Decompress data using pako inflate
 */
function decompressData(compressed: Uint8Array): string {
  const decompressed = pako.inflate(compressed);
  const textDecoder = new TextDecoder();
  return textDecoder.decode(decompressed);
}

/**
 * Encrypt a batch of items: JSON.stringify → compress → JWE encrypt
 */
async function encryptBatch(items: unknown[], keyBytes: Uint8Array): Promise<string> {
  // 1. Serialise to JSON
  const jsonString = JSON.stringify(items);

  // 2. Compress
  const compressed = compressData(jsonString);

  // 3. Encrypt as JWE
  const jwe = await new CompactEncrypt(compressed)
    .setProtectedHeader({ alg: JWE_ALG, enc: JWE_ENC })
    .encrypt(keyBytes);

  return jwe;
}

/**
 * Decrypt a batch: JWE decrypt → decompress → JSON.parse
 */
async function decryptBatch<T>(jwe: string, keyBytes: Uint8Array): Promise<T[]> {
  // 1. Decrypt JWE
  const { plaintext } = await compactDecrypt(jwe, keyBytes);

  // 2. Decompress
  const jsonString = decompressData(plaintext);

  // 3. Parse JSON
  return JSON.parse(jsonString);
}

/**
 * Split an array into batches of specified size
 */
function splitIntoBatches<T>(items: T[], batchSize: number): T[][] {
  const batches: T[][] = [];
  for (let i = 0; i < items.length; i += batchSize) {
    batches.push(items.slice(i, i + batchSize));
  }
  return batches;
}

/**
 * Progress callback for backup creation
 */
export type BackupProgressCallback = (progress: {
  phase: 'counting' | 'loading' | 'encrypting' | 'complete';
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

  const backup: BackupDataV2 = {
    version: BACKUP_VERSION_LEGACY,
    type: BACKUP_TYPE,
    createdAt: new Date().toISOString(),
    encryption,
    data: jweRecords,
  };

  return backup;
}

/**
 * Create a batched, compressed backup (v3.0)
 *
 * Items are batched, then each batch is: JSON → compress → encrypt.
 * This significantly reduces backup size and memory usage for large collections.
 */
export async function createBatchedBackup(
  onProgress?: BackupProgressCallback
): Promise<BackupDataV3> {
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

  const keyBytes = masterKey.keyBytes;

  onProgress?.({ phase: 'counting' });

  // Gather all data
  const notes = await noteRepository.getAll();
  const attachmentIds = await attachmentRepository.listAllIds();
  const db = getDB();
  const versions = await db.getAll(STORES.NOTE_VERSIONS);
  let savedSearches: SavedSearch[] = [];
  try {
    savedSearches = await db.getAll(STORES.SAVED_SEARCHES);
  } catch {
    console.warn('[backupService] Could not read saved searches');
  }

  // Calculate total batches for progress
  const noteBatches = Math.ceil(notes.length / NOTE_BATCH_SIZE) || 1;
  const attachmentBatches = Math.ceil(attachmentIds.length / ATTACHMENT_BATCH_SIZE) || 1;
  const versionBatches = Math.ceil(versions.length / VERSION_BATCH_SIZE) || 1;
  const totalBatches = noteBatches + attachmentBatches + versionBatches + 2; // +2 for settings/sync batches

  let currentBatch = 0;
  const batches: BatchRecord[] = [];

  // Process notes in batches
  const noteBatchArrays = splitIntoBatches(notes, NOTE_BATCH_SIZE);
  for (let i = 0; i < noteBatchArrays.length; i++) {
    const batch = noteBatchArrays[i];
    const jwe = await encryptBatch(batch, keyBytes);
    batches.push({
      type: 'notes',
      index: i,
      count: batch.length,
      data: jwe,
    });
    currentBatch++;
    onProgress?.({ phase: 'encrypting', current: currentBatch, total: totalBatches, item: 'notes' });
  }

  // If no notes, still report progress
  if (noteBatchArrays.length === 0) {
    currentBatch++;
    onProgress?.({ phase: 'encrypting', current: currentBatch, total: totalBatches, item: 'notes' });
  }

  // Process attachments in batches (need to load blob data first)
  // Report progress during loading since this can be slow for many attachments
  const attachmentRecords: AttachmentRecord[] = [];
  for (let idx = 0; idx < attachmentIds.length; idx++) {
    const id = attachmentIds[idx];
    onProgress?.({
      phase: 'loading',
      current: idx + 1,
      total: attachmentIds.length,
      item: 'attachments',
    });

    const blob = await attachmentRepository.getBlob(id);
    const thumbnail = await attachmentRepository.getThumbnail(id);

    if (blob) {
      attachmentRecords.push({
        id,
        blob: arrayBufferToBase64(blob),
        thumbnail: thumbnail ? arrayBufferToBase64(thumbnail) : undefined,
      });
    }
  }

  const attachmentBatchArrays = splitIntoBatches(attachmentRecords, ATTACHMENT_BATCH_SIZE);
  for (let i = 0; i < attachmentBatchArrays.length; i++) {
    const batch = attachmentBatchArrays[i];
    const jwe = await encryptBatch(batch, keyBytes);
    batches.push({
      type: 'attachments',
      index: i,
      count: batch.length,
      data: jwe,
    });
    currentBatch++;
    onProgress?.({
      phase: 'encrypting',
      current: currentBatch,
      total: totalBatches,
      item: 'attachments',
    });
  }

  // If no attachments, still report progress
  if (attachmentBatchArrays.length === 0) {
    currentBatch++;
    onProgress?.({
      phase: 'encrypting',
      current: currentBatch,
      total: totalBatches,
      item: 'attachments',
    });
  }

  // Process versions in batches
  const versionBatchArrays = splitIntoBatches(versions, VERSION_BATCH_SIZE);
  for (let i = 0; i < versionBatchArrays.length; i++) {
    const batch = versionBatchArrays[i];
    const jwe = await encryptBatch(batch, keyBytes);
    batches.push({
      type: 'versions',
      index: i,
      count: batch.length,
      data: jwe,
    });
    currentBatch++;
    onProgress?.({
      phase: 'encrypting',
      current: currentBatch,
      total: totalBatches,
      item: 'versions',
    });
  }

  // If no versions, still report progress
  if (versionBatchArrays.length === 0) {
    currentBatch++;
    onProgress?.({
      phase: 'encrypting',
      current: currentBatch,
      total: totalBatches,
      item: 'versions',
    });
  }

  // Process saved searches (usually small, single batch)
  if (savedSearches.length > 0) {
    const jwe = await encryptBatch(savedSearches, keyBytes);
    batches.push({
      type: 'saved_searches',
      index: 0,
      count: savedSearches.length,
      data: jwe,
    });
  }
  currentBatch++;
  onProgress?.({ phase: 'encrypting', current: currentBatch, total: totalBatches, item: 'searches' });

  // Process settings (single item batch)
  const settings = await settingsRepository.get();
  const settingsJwe = await encryptBatch([settings], keyBytes);
  batches.push({
    type: 'settings',
    index: 0,
    count: 1,
    data: settingsJwe,
  });
  currentBatch++;
  onProgress?.({ phase: 'encrypting', current: currentBatch, total: totalBatches, item: 'settings' });

  // Process sync metadata (if present)
  const syncMetadata = await syncRepository.getMetadata();
  if (syncMetadata) {
    const jwe = await encryptBatch([syncMetadata], keyBytes);
    batches.push({
      type: 'sync_metadata',
      index: 0,
      count: 1,
      data: jwe,
    });
  }

  onProgress?.({ phase: 'complete', current: totalBatches, total: totalBatches });

  const backup: BackupDataV3 = {
    version: BACKUP_VERSION,
    type: BACKUP_TYPE,
    createdAt: new Date().toISOString(),
    encryption,
    batches,
    summary: {
      notes: notes.length,
      attachments: attachmentRecords.length,
      versions: versions.length,
      savedSearches: savedSearches.length,
    },
  };

  return backup;
}

/**
 * Parse a backup file using streaming for large files
 * Falls back to full parse for smaller files or legacy formats
 */
async function parseBackupFile(file: File): Promise<BackupData | null> {
  // For files under 100MB, try direct parse
  if (file.size < 100 * 1024 * 1024) {
    try {
      const text = await file.text();
      return JSON.parse(text);
    } catch {
      // Fall through to streaming parse
    }
  }

  // For large files, use streaming parse
  try {
    // Parse header
    const header = await parseBackupHeader(file);
    if (!header || !header.version || !header.type || !header.encryption) {
      return null;
    }

    // For v3.0, we can reconstruct the backup from header + streaming batches + summary
    if (header.version === '3.0') {
      const summary = await parseBackupSummary(file);
      if (!summary) return null;

      // Parse batches by reading the file in chunks and extracting JSON objects
      const batches = await parseBackupBatches(file);
      if (!batches) return null;

      return {
        version: header.version,
        type: header.type,
        createdAt: header.createdAt || new Date().toISOString(),
        encryption: header.encryption,
        batches,
        summary,
      } as BackupDataV3;
    }

    // For v2.x, parse the data array
    if (header.version === '2.1' || header.version === '2.0') {
      const data = await parseBackupV2Data(file);
      if (!data) return null;

      return {
        version: header.version,
        type: header.type,
        createdAt: header.createdAt || new Date().toISOString(),
        encryption: header.encryption,
        data,
      } as BackupDataV2;
    }

    return null;
  } catch {
    return null;
  }
}

/**
 * Parse batches array from a v3.0 backup file using streaming
 */
async function parseBackupBatches(file: File): Promise<BatchRecord[] | null> {
  const batches: BatchRecord[] = [];
  const chunkSize = 10 * 1024 * 1024; // 10MB chunks
  let buffer = '';
  let inBatchesArray = false;
  let braceDepth = 0;
  let currentBatch = '';

  for (let offset = 0; offset < file.size; offset += chunkSize) {
    const chunk = file.slice(offset, Math.min(offset + chunkSize, file.size));
    const text = await chunk.text();
    buffer += text;

    // Find start of batches array if not found yet
    if (!inBatchesArray) {
      const batchesStart = buffer.indexOf('"batches"');
      if (batchesStart === -1) {
        // Keep last 100 chars in case "batches" is split across chunks
        buffer = buffer.slice(-100);
        continue;
      }
      // Find the opening bracket
      const bracketPos = buffer.indexOf('[', batchesStart);
      if (bracketPos === -1) {
        buffer = buffer.slice(batchesStart);
        continue;
      }
      buffer = buffer.slice(bracketPos + 1);
      inBatchesArray = true;
    }

    // Parse batch objects from buffer
    let i = 0;
    while (i < buffer.length) {
      const char = buffer[i];

      if (char === '{') {
        if (braceDepth === 0) {
          currentBatch = '';
        }
        braceDepth++;
        currentBatch += char;
      } else if (char === '}') {
        braceDepth--;
        currentBatch += char;
        if (braceDepth === 0 && currentBatch) {
          try {
            const batch = JSON.parse(currentBatch);
            batches.push(batch);
          } catch {
            // Skip malformed batch
          }
          currentBatch = '';
        }
      } else if (char === ']' && braceDepth === 0) {
        // End of batches array
        return batches;
      } else if (braceDepth > 0) {
        currentBatch += char;
      }

      i++;
    }

    // Keep unprocessed part of buffer
    if (braceDepth > 0) {
      // We're in the middle of a batch object, keep it
      buffer = currentBatch;
      currentBatch = '';
    } else {
      buffer = '';
    }
  }

  return batches.length > 0 ? batches : null;
}

/**
 * Parse data array from a v2.x backup file using streaming
 */
async function parseBackupV2Data(file: File): Promise<string[] | null> {
  const data: string[] = [];
  const chunkSize = 10 * 1024 * 1024; // 10MB chunks
  let buffer = '';
  let inDataArray = false;

  for (let offset = 0; offset < file.size; offset += chunkSize) {
    const chunk = file.slice(offset, Math.min(offset + chunkSize, file.size));
    const text = await chunk.text();
    buffer += text;

    // Find start of data array if not found yet
    if (!inDataArray) {
      const dataStart = buffer.indexOf('"data"');
      if (dataStart === -1) {
        buffer = buffer.slice(-50);
        continue;
      }
      const bracketPos = buffer.indexOf('[', dataStart);
      if (bracketPos === -1) {
        buffer = buffer.slice(dataStart);
        continue;
      }
      buffer = buffer.slice(bracketPos + 1);
      inDataArray = true;
    }

    // Extract JWE tokens (strings starting with "eyJ")
    const tokenRegex = /"(eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)"/g;
    let match;
    while ((match = tokenRegex.exec(buffer)) !== null) {
      data.push(match[1]);
    }

    // Check for end of array
    if (buffer.includes(']}') || buffer.includes(']\n}')) {
      break;
    }

    // Keep last part in case token spans chunks
    buffer = buffer.slice(-1000);
  }

  return data.length > 0 ? data : null;
}

/**
 * Validate a backup file structure
 *
 * Checks:
 * - JSON structure is valid
 * - Required fields are present
 * - Version is compatible (v2.1 or v3.0)
 * - Data format matches version
 *
 * Uses streaming parsing for large files to avoid memory issues.
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

    // Parse file (uses streaming for large files)
    const backup = await parseBackupFile(file);
    if (!backup) {
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

    // Check version compatibility (supports 2.x and 3.x)
    const [major, minor] = backup.version.split('.').map(n => parseInt(n, 10));
    if (major > 3 || (major === 3 && minor > 0)) {
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

    // v3.0: batched format
    if (major === 3) {
      if (!isBackupV3(backup)) {
        return {
          valid: false,
          error: 'Invalid backup file. v3.0 backup must have batches array.',
        };
      }

      if (!backup.batches || !Array.isArray(backup.batches)) {
        return {
          valid: false,
          error: 'Invalid backup file. Missing batches section.',
        };
      }

      // Validate batch records
      for (const batch of backup.batches) {
        if (typeof batch.type !== 'string' || typeof batch.index !== 'number' || typeof batch.count !== 'number') {
          return {
            valid: false,
            error: 'Invalid backup file. Malformed batch record.',
          };
        }
        if (typeof batch.data !== 'string') {
          return {
            valid: false,
            error: 'Invalid backup file. Batch data must be a JWE string.',
          };
        }
        // Basic JWE format check (5 parts separated by dots)
        const parts = batch.data.split('.');
        if (parts.length !== 5) {
          return {
            valid: false,
            error: 'Invalid backup file. Malformed JWE token in batch.',
          };
        }
      }

      if (!backup.summary) {
        return {
          valid: false,
          error: 'Invalid backup file. Missing summary section.',
        };
      }

      return {
        valid: true,
        backup,
      };
    }

    // v2.x: legacy format
    if (!isBackupV2(backup)) {
      return {
        valid: false,
        error: 'Invalid backup file. v2.x backup must have data array.',
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
 * Supports both v2.x and v3.0 backup formats.
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

    // Verify by attempting to decrypt the first record/batch
    if (isBackupV3(backup)) {
      // v3.0: decrypt first batch
      if (backup.batches.length > 0) {
        try {
          await decryptBatch(backup.batches[0].data, keyBytes);
        } catch {
          return {
            valid: false,
            error: 'Incorrect password',
          };
        }
      }
    } else if (isBackupV2(backup)) {
      // v2.x: decrypt first record
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
 * Restore data from a v2.x backup (legacy)
 *
 * Decrypts each JWE record and restores based on its type.
 *
 * IMPORTANT: This should only be called on a fresh/empty database.
 * Existing data will be overwritten.
 */
export async function restoreBackup(
  backup: BackupDataV2,
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
 * Restore data from a v3.0 batched backup
 *
 * Decrypts and decompresses each batch, then restores items.
 *
 * IMPORTANT: This should only be called on a fresh/empty database.
 * Existing data will be overwritten.
 */
export async function restoreBatchedBackup(
  backup: BackupDataV3,
  keyBytes: Uint8Array,
  onProgress?: RestoreProgressCallback
): Promise<void> {
  const db = getDB();
  const totalBatches = backup.batches.length;

  onProgress?.({ phase: 'validating' });

  // 1. Restore encryption metadata first
  await encryptionRepository.setMetadata(backup.encryption);

  onProgress?.({ phase: 'decrypting', current: 0, total: totalBatches });

  // 2. Process each batch
  for (let i = 0; i < backup.batches.length; i++) {
    const batch = backup.batches[i];

    try {
      onProgress?.({ phase: 'decrypting', current: i, total: totalBatches });

      // Decrypt and decompress the batch
      const items = await decryptBatch<unknown>(batch.data, keyBytes);

      onProgress?.({ phase: 'restoring', current: i + 1, total: totalBatches });

      // Restore items based on batch type
      switch (batch.type) {
        case 'notes':
          for (const note of items) {
            await db.put(STORES.NOTES, note as Note);
          }
          break;

        case 'attachments':
          for (const item of items) {
            const attachment = item as AttachmentRecord;
            const blob = base64ToArrayBuffer(attachment.blob);
            await attachmentRepository.storeBlob(attachment.id, blob);

            if (attachment.thumbnail) {
              const thumbnail = base64ToArrayBuffer(attachment.thumbnail);
              await attachmentRepository.storeThumbnail(attachment.id, thumbnail);
            }
          }
          break;

        case 'versions':
          for (const version of items) {
            await db.put(STORES.NOTE_VERSIONS, version as NoteVersion);
          }
          break;

        case 'saved_searches':
          for (const savedSearch of items) {
            await db.put(STORES.SAVED_SEARCHES, savedSearch as SavedSearch);
          }
          break;

        case 'settings':
          // Settings batch contains a single item
          if (items.length > 0) {
            await settingsRepository.update(items[0] as UserSettings);
          }
          break;

        case 'sync_metadata':
          // Sync metadata batch contains a single item
          if (items.length > 0) {
            const syncMeta = items[0] as SyncMetadata;
            // Mark the API key for re-registration so restored device gets a new ID
            if (syncMeta.apiKey && !syncMeta.apiKey.startsWith('RESTORE:')) {
              syncMeta.apiKey = 'RESTORE:' + syncMeta.apiKey;
            }
            await syncRepository.updateMetadata(syncMeta);
          }
          break;

        default:
          console.warn(`[backupService] Unknown batch type: ${batch.type}`);
      }
    } catch (error) {
      console.error(`[backupService] Failed to restore batch ${i} (${batch.type}):`, error);
      throw new Error(
        `Failed to restore batch ${i + 1} (${batch.type}): ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  }

  onProgress?.({ phase: 'complete' });
}

/**
 * Unified restore function that handles both v2.x and v3.0 backup formats
 *
 * Automatically detects the backup version and delegates to the appropriate
 * restore function.
 */
export async function restoreFromBackup(
  backup: BackupData,
  keyBytes: Uint8Array,
  onProgress?: RestoreProgressCallback
): Promise<void> {
  if (isBackupV3(backup)) {
    return restoreBatchedBackup(backup, keyBytes, onProgress);
  }

  if (isBackupV2(backup)) {
    return restoreBackup(backup, keyBytes, onProgress);
  }

  throw new Error('Unknown backup format');
}

/**
 * Download a backup as a file
 *
 * Uses chunked JSON generation to avoid string length limits on large backups.
 * Handles both v2.x (data array) and v3.0 (batches array) formats.
 */
export function downloadBackup(backup: BackupData): void {
  // Build JSON in chunks to avoid string length limits
  // Use newlines between records to enable streaming parsing on import
  const chunks: string[] = [];

  // Opening structure (with newlines for readability and streaming)
  chunks.push('{\n"version":');
  chunks.push(JSON.stringify(backup.version));
  chunks.push(',\n"type":');
  chunks.push(JSON.stringify(backup.type));
  chunks.push(',\n"createdAt":');
  chunks.push(JSON.stringify(backup.createdAt));
  chunks.push(',\n"encryption":');
  chunks.push(JSON.stringify(backup.encryption));

  if (isBackupV3(backup)) {
    // v3.0 format: batches array and summary
    // Each batch on its own line for streaming parsing
    chunks.push(',\n"batches":[\n');

    for (let i = 0; i < backup.batches.length; i++) {
      if (i > 0) chunks.push(',\n');
      chunks.push(JSON.stringify(backup.batches[i]));
    }

    chunks.push('\n],\n"summary":');
    chunks.push(JSON.stringify(backup.summary));
    chunks.push('\n}');
  } else if (isBackupV2(backup)) {
    // v2.x format: data array
    // Each record on its own line for streaming parsing
    chunks.push(',\n"data":[\n');

    for (let i = 0; i < backup.data.length; i++) {
      if (i > 0) chunks.push(',\n');
      chunks.push(JSON.stringify(backup.data[i]));
    }

    chunks.push('\n]\n}');
  }

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

  // Clean up blob URL after delay to ensure download initiates
  // (immediate revocation can fail on slow systems or large files)
  setTimeout(() => {
    URL.revokeObjectURL(url);
  }, 1000);
}

/**
 * Parse backup header from the beginning of a file
 * This avoids loading the entire file into memory
 */
async function parseBackupHeader(file: File): Promise<{
  version?: string;
  type?: string;
  createdAt?: string;
  encryption?: EncryptionMetadata;
} | null> {
  // Read first 10KB which should contain all header fields
  const headerSize = Math.min(10 * 1024, file.size);
  const headerSlice = file.slice(0, headerSize);
  const headerText = await headerSlice.text();

  try {
    // Extract individual fields using regex (works on partial JSON)
    const versionMatch = headerText.match(/"version"\s*:\s*"([^"]+)"/);
    const typeMatch = headerText.match(/"type"\s*:\s*"([^"]+)"/);
    const createdAtMatch = headerText.match(/"createdAt"\s*:\s*"([^"]+)"/);

    // Extract encryption object (it's relatively small and near the start)
    const encryptionMatch = headerText.match(/"encryption"\s*:\s*(\{[^}]+\})/);

    if (!versionMatch || !typeMatch) {
      return null;
    }

    return {
      version: versionMatch[1],
      type: typeMatch[1],
      createdAt: createdAtMatch?.[1],
      encryption: encryptionMatch ? JSON.parse(encryptionMatch[1]) : undefined,
    };
  } catch {
    return null;
  }
}

/**
 * Parse backup summary from the end of a v3.0 file
 */
async function parseBackupSummary(file: File): Promise<BackupSummary | null> {
  // Read last 1KB which should contain the summary
  const tailSize = Math.min(1024, file.size);
  const tailSlice = file.slice(file.size - tailSize);
  const tailText = await tailSlice.text();

  try {
    // Find the summary object
    const summaryMatch = tailText.match(/"summary"\s*:\s*(\{[^}]+\})/);
    if (summaryMatch) {
      return JSON.parse(summaryMatch[1]);
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * Count records in a v2.x backup by counting JWE tokens
 * Uses streaming to avoid loading entire file
 */
async function countV2Records(file: File): Promise<number> {
  // For v2.x, count the JWE tokens in the data array
  // Each JWE token starts with "eyJ" (base64 of '{"')
  let count = 0;
  const chunkSize = 1024 * 1024; // 1MB chunks

  for (let offset = 0; offset < file.size; offset += chunkSize) {
    const chunk = file.slice(offset, offset + chunkSize);
    const text = await chunk.text();
    // Count occurrences of JWE token starts (rough count)
    const matches = text.match(/"eyJ[A-Za-z0-9_-]+\./g);
    if (matches) {
      count += matches.length;
    }
  }

  return count;
}

/**
 * Get backup file stats without full validation
 *
 * For v2.x backups, we can only show record count (types are encrypted).
 * For v3.0 backups, we can show the full summary (notes, attachments, versions, saved searches).
 *
 * Uses streaming parsing to handle large files without loading everything into memory.
 */
export async function getBackupStats(file: File): Promise<{
  valid: boolean;
  version?: string;
  createdAt?: string;
  recordCount?: number;
  summary?: BackupSummary;
  error?: string;
}> {
  try {
    // Parse header without loading entire file
    const header = await parseBackupHeader(file);

    if (!header) {
      return { valid: false, error: 'Could not read backup file header' };
    }

    if (header.type !== BACKUP_TYPE) {
      return { valid: false, error: 'Not a Jottery backup file' };
    }

    // v3.0: get summary from end of file
    if (header.version === '3.0') {
      const summary = await parseBackupSummary(file);
      if (summary) {
        return {
          valid: true,
          version: header.version,
          createdAt: header.createdAt,
          summary,
          recordCount:
            summary.notes + summary.attachments + summary.versions + summary.savedSearches,
        };
      }
      return { valid: false, error: 'Could not read backup summary' };
    }

    // v2.x: count records by scanning file
    if (header.version === '2.1' || header.version === '2.0') {
      const recordCount = await countV2Records(file);
      return {
        valid: true,
        version: header.version,
        createdAt: header.createdAt,
        recordCount,
      };
    }

    return { valid: false, error: 'Unknown backup format' };
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
  createBatchedBackup,
  validateBackup,
  verifyBackupPassword,
  restoreBackup,
  restoreBatchedBackup,
  restoreFromBackup,
  downloadBackup,
  getBackupStats,
  BACKUP_FILE_EXTENSION,
  // Constants for external use
  NOTE_BATCH_SIZE,
  ATTACHMENT_BATCH_SIZE,
  VERSION_BATCH_SIZE,
};
