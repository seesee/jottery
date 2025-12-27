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
  wordWrap?: boolean; // Word wrap enabled (default: true)
  syntaxLanguage?: string; // Syntax highlighting language (any supported language ID)
  showPreview?: boolean; // Show preview panel for markdown/html (default: false)
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
 * Keyboard shortcut configuration
 */
export interface KeyboardShortcut {
  key: string; // The key to press (e.g., 'k', 'n', ',')
  ctrl?: boolean; // Ctrl on Windows/Linux, Cmd on Mac
  alt?: boolean;
  shift?: boolean;
}

/**
 * All available keyboard shortcuts
 */
export interface KeyboardShortcuts {
  focusSearch: KeyboardShortcut;
  newNote: KeyboardShortcut;
  lockApp: KeyboardShortcut;
  openSettings: KeyboardShortcut;
  showShortcuts: KeyboardShortcut;
  copyNote: KeyboardShortcut;
  undo: KeyboardShortcut;
  redo: KeyboardShortcut;
  versionHistory: KeyboardShortcut;
  noteInfo: KeyboardShortcut;
}

/**
 * User application settings
 * Stored unencrypted in IndexedDB
 */
export interface UserSettings {
  language: string; // i18n locale code
  theme: 'light' | 'dark' | 'auto';
  layoutMode: 'auto' | 'mobile' | 'desktop'; // Layout mode override
  sortOrder: 'recent' | 'oldest' | 'alpha' | 'created';
  autoLockTimeout: number; // Minutes
  fontSize: 'auto' | 'small' | 'medium' | 'large'; // Editor font size (auto = mobile-aware)
  syncEnabled: boolean;
  syncEndpoint?: string;
  rememberPassword: boolean; // DANGER: Store password in localStorage (insecure)
  keyboardShortcuts?: KeyboardShortcuts; // Custom keyboard shortcuts
  enabledSyntaxLanguages?: string[]; // Enabled syntax highlighting languages (defaults to core languages)
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
 * Note version snapshot (for version history)
 * Stores encrypted content at a specific point in time
 */
export interface NoteVersion {
  versionKey: string; // `${noteId}:${version}`
  noteId: string;
  version: number;
  createdAt: string; // When version captured
  syncedAt: string; // When sync completed
  content: string; // Encrypted
  tags: string[]; // Encrypted (same format as Note)
  attachments: Attachment[];
  syntaxLanguage?: string;
  wordWrap?: boolean;
  reason: 'sync' | 'manual-sync';
}

/**
 * Decrypted version of NoteVersion (in-memory only)
 * Used for displaying version history
 */
export interface DecryptedNoteVersion extends Omit<NoteVersion, 'content' | 'tags'> {
  content: string; // Decrypted
  tags: string[]; // Decrypted
  characterCount?: number;
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
  wordWrap?: boolean;
  syntaxLanguage?: string; // Any supported language ID
  showPreview?: boolean;
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
  wordWrap: true,
  syntaxLanguage: 'markdown',
  showPreview: false,
};

/**
 * Default keyboard shortcuts (non-clashing with browser shortcuts)
 */
export const DEFAULT_KEYBOARD_SHORTCUTS: KeyboardShortcuts = {
  focusSearch: { key: 'k', ctrl: true }, // Ctrl/Cmd+K
  newNote: { key: 'n', alt: true }, // Alt+N
  lockApp: { key: 'l', alt: true }, // Alt+L
  openSettings: { key: ',', ctrl: true }, // Ctrl/Cmd+,
  showShortcuts: { key: '/', alt: true }, // Alt+/
  copyNote: { key: 'c', alt: true }, // Alt+C
  undo: { key: 'z', ctrl: true }, // Ctrl/Cmd+Z
  redo: { key: 'z', ctrl: true, shift: true }, // Ctrl/Cmd+Shift+Z
  versionHistory: { key: 'h', alt: true }, // Alt+H
  noteInfo: { key: 'i', alt: true }, // Alt+I
};

/**
 * Default user settings
 */
export const DEFAULT_SETTINGS: UserSettings = {
  language: 'en-GB',
  theme: 'auto',
  layoutMode: 'auto',
  sortOrder: 'recent',
  autoLockTimeout: 15, // 15 minutes
  fontSize: 'auto', // Auto-detects mobile and uses appropriate size
  syncEnabled: false,
  rememberPassword: false,
  keyboardShortcuts: DEFAULT_KEYBOARD_SHORTCUTS,
  enabledSyntaxLanguages: ['javascript', 'typescript', 'python', 'markdown', 'json', 'xml', 'css', 'bash', 'sql'], // Core languages + SQL
};
