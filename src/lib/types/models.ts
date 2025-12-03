/**
 * Core data models for Jottery
 * Based on jottery-spec.md section 2.3
 */

/**
 * Represents a note in the system
 * Content, tags, and attachments are encrypted
 */
export interface Note {
  id: string; // UUID v4
  createdAt: string; // ISO 8601 with timezone
  modifiedAt: string; // ISO 8601 with timezone
  syncedAt?: string; // ISO 8601 with timezone
  content: string; // Encrypted note content
  tags: string[]; // Encrypted array of tags
  attachments: Attachment[]; // Array of attachment references
  pinned: boolean; // Pin status
  deleted: boolean; // Soft delete flag
  deletedAt?: string; // Deletion timestamp
  syncHash?: string; // Hash for conflict detection
  version: number; // Optimistic locking
}

/**
 * Represents a file attachment
 * Filename is encrypted, data is a reference to encrypted blob store
 */
export interface Attachment {
  id: string; // UUID v4
  filename: string; // Original filename (encrypted)
  mimeType: string; // MIME type
  size: number; // Size in bytes
  data: string; // Reference to encrypted blob store
  thumbnailData?: string; // Optional thumbnail for images
}

/**
 * User application settings
 * Stored unencrypted in IndexedDB
 */
export interface UserSettings {
  language: string; // i18n locale code
  theme: 'light' | 'dark' | 'auto';
  sortOrder: 'recent' | 'oldest' | 'alpha' | 'created';
  autoLockTimeout: number; // Minutes
  syncEnabled: boolean;
  syncEndpoint?: string;
}

/**
 * Encryption metadata stored per user
 */
export interface EncryptionMetadata {
  salt: string; // Unique salt for key derivation
  iterations: number; // PBKDF2 iterations
  createdAt: string; // When encryption was set up
  algorithm: 'AES-256-GCM'; // Encryption algorithm
}

/**
 * Decrypted note content (in-memory only)
 * Used for caching and display purposes
 */
export interface DecryptedNote extends Omit<Note, 'content' | 'tags'> {
  content: string; // Decrypted content
  tags: string[]; // Decrypted tags
  decryptedAt: number; // Timestamp when decrypted (for cache management)
}

/**
 * Search query structure
 */
export interface SearchQuery {
  text?: string; // Full-text search terms
  tags?: string[]; // Tags to filter by (AND logic)
  orTags?: string[]; // Tags to filter by (OR logic)
  excludeText?: string[]; // Text to exclude
  excludeTags?: string[]; // Tags to exclude
}

/**
 * Sort options for note list
 */
export type SortOrder = 'recent' | 'oldest' | 'alpha' | 'created';

/**
 * Theme options
 */
export type Theme = 'light' | 'dark' | 'auto';

/**
 * Application lock state
 */
export interface LockState {
  isLocked: boolean;
  lastActivityAt: number;
  autoLockTimeout: number; // in milliseconds
}

/**
 * Export format for notes
 */
export interface ExportData {
  version: string;
  exportDate: string; // ISO 8601
  notes: ExportNote[];
}

/**
 * Exported note format (decrypted for export)
 */
export interface ExportNote {
  id: string;
  createdAt: string;
  modifiedAt: string;
  content: string; // Decrypted
  tags: string[]; // Decrypted
  attachments: ExportAttachment[];
  pinned: boolean;
}

/**
 * Exported attachment format
 */
export interface ExportAttachment {
  filename: string; // Decrypted
  mimeType: string;
  data: string; // Base64 encoded
}

/**
 * Sync operations for Phase 3
 */
export interface SyncOperation {
  id: string;
  type: 'create' | 'update' | 'delete';
  noteId: string;
  timestamp: string;
  status: 'pending' | 'synced' | 'failed';
}

/**
 * Default values for new notes
 */
export const DEFAULT_NOTE: Omit<Note, 'id' | 'createdAt' | 'modifiedAt'> = {
  content: '',
  tags: [],
  attachments: [],
  pinned: false,
  deleted: false,
  version: 1,
};

/**
 * Default user settings
 */
export const DEFAULT_SETTINGS: UserSettings = {
  language: 'en-GB',
  theme: 'auto',
  sortOrder: 'recent',
  autoLockTimeout: 15, // 15 minutes
  syncEnabled: false,
};
