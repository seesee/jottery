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
  archived: boolean; // Archive status
  archivedAt?: string; // Archive timestamp
  locked: boolean; // Lock status (prevents editing)
  lockedAt?: string; // Lock timestamp
  deleted: boolean; // Soft delete flag
  deletedAt?: string; // Deletion timestamp
  syncHash?: string; // Hash for conflict detection
  version: number; // Optimistic locking
  wordWrap?: boolean; // Word wrap enabled (default: true)
  syntaxLanguage?: string; // Syntax highlighting language (any supported language ID)
  showPreview?: boolean; // Show preview panel for markdown/html (default: false)
  color?: string; // Semantic color name (e.g., 'red', 'blue') - unencrypted
  needsSync?: boolean; // Local-only flag indicating note needs to be synced (never sent to server)
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
 * Color palette for notes and tags
 * Maps semantic color names to hex codes for light and dark modes
 */
export interface ColorPalette {
  [colorName: string]: {
    light: string; // Hex code for light mode (e.g., '#FFE5E5')
    dark: string; // Hex code for dark mode (e.g., '#5C1A1A')
    displayName?: string; // Optional user-defined display name (ENCRYPTED, e.g., 'Project X', 'Client ABC')
  };
}

/**
 * Default 8-color palette
 * Users can customise these hex values in settings
 */
export const DEFAULT_COLOR_PALETTE: ColorPalette = {
  red: { light: '#FFE5E5', dark: '#5C1A1A' },
  orange: { light: '#FFF0E0', dark: '#5C3A1A' },
  yellow: { light: '#FFFACD', dark: '#5C5520' },
  green: { light: '#E5F5E5', dark: '#1A4D1A' },
  blue: { light: '#E5F0FF', dark: '#1A3A5C' },
  purple: { light: '#F0E5FF', dark: '#3A1A5C' },
  pink: { light: '#FFE5F0', dark: '#5C1A3A' },
  gray: { light: '#F0F0F0', dark: '#2A2A2A' },
};

/**
 * Array of default color names for easy iteration
 */
export const COLOR_NAMES = Object.keys(DEFAULT_COLOR_PALETTE);

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
  deleteNote: KeyboardShortcut;
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
  timezone: string; // IANA timezone (e.g., 'UTC', 'America/New_York', 'local' for browser timezone)
  syncEnabled: boolean;
  syncEndpoint?: string;
  rememberPassword: boolean; // DANGER: Store password in localStorage (insecure)
  keyboardShortcuts?: KeyboardShortcuts; // Custom keyboard shortcuts
  enabledSyntaxLanguages?: string[]; // Enabled syntax highlighting languages (defaults to core languages)
  defaultSyntaxLanguage?: string; // Default language for new notes (defaults to 'markdown')
  openLinksInNewTab?: boolean; // Open external links in new tab (defaults to true)
  vimMode?: boolean; // Enable vim keybindings in editor (defaults to false)
  quickCommandsEnabled?: boolean; // Enable quick commands (defaults to true)
  quickCommandsList?: QuickCommandConfig[]; // Custom quick commands
  persistSession?: boolean; // Enable session persistence across page refresh (tab-scoped)
  persistSessionTimeout?: number; // Session expiry in minutes (default: 30)
  colorPalette?: ColorPalette; // User-customised color palette (defaults to DEFAULT_COLOR_PALETTE)
  tagColors?: Record<string, string>; // Global tag name → color name mapping
}

/**
 * Quick command configuration
 * Defines a slash command that expands to generated content
 */
export interface QuickCommandConfig {
  id: string; // Unique identifier for the command
  trigger: string; // The slash command (e.g., "/now")
  type: 'datetime' | 'date' | 'time' | 'uuid' | 'text'; // Type of content to generate
  format?: string; // Format string for datetime/date/time types (e.g., "YYYY-MM-DD")
  value?: string; // Static text for 'text' type
  description: string; // Description shown in autocomplete dropdown
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
  showPreview?: boolean;
  color?: string; // Semantic color name (unencrypted)
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
  // Advanced search modifiers
  hasAttachment?: boolean; // Filter notes with attachments
  createdAfter?: string; // ISO date string - notes created after this date
  createdBefore?: string; // ISO date string - notes created before this date
  modifiedAfter?: string; // ISO date string - notes modified after this date
  modifiedBefore?: string; // ISO date string - notes modified before this date
  wordCountMin?: number; // Minimum word count
  wordCountMax?: number; // Maximum word count
  colors?: string[]; // Color names to match (OR logic)
  excludeColors?: string[]; // Color names to exclude (AND logic)
  colorTarget?: 'note' | 'tag' | 'both'; // Where to look for colors (default: 'both')
  archived?: boolean; // Filter archived notes (true = archived only, false = non-archived only, undefined = both)
}

/**
 * Saved search query
 * Allows users to save frequently used searches
 */
export interface SavedSearch {
  id: string; // UUID v4
  name: string; // User-provided name (ENCRYPTED)
  query: string; // Search query string (ENCRYPTED)
  order: number; // Display order
  createdAt: string; // ISO 8601 with timezone
  modifiedAt: string; // ISO 8601 with timezone
  syncedAt?: string; // ISO 8601 with timezone
  deleted: boolean; // Soft delete flag
  deletedAt?: string; // Deletion timestamp
  version: number; // Optimistic locking
  needsSync?: boolean; // Local-only flag indicating search needs to be synced
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
  settings?: {
    colorPalette?: ColorPalette;
    tagColors?: Record<string, string>;
  };
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
  archived: boolean;
  locked: boolean;
  wordWrap?: boolean;
  syntaxLanguage?: string; // Any supported language ID
  showPreview?: boolean;
  color?: string; // Semantic color name
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
  archived: false,
  locked: false,
  deleted: false,
  version: 0,
  wordWrap: true,
  syntaxLanguage: 'markdown',
  showPreview: false,
  needsSync: true, // New notes should be synced
};

/**
 * Default keyboard shortcuts (non-clashing with browser shortcuts)
 */
export const DEFAULT_KEYBOARD_SHORTCUTS: KeyboardShortcuts = {
  focusSearch: { key: 'k', ctrl: true }, // Ctrl/Cmd+K
  newNote: { key: 'n', alt: true }, // Alt+N
  deleteNote: { key: 'Delete' }, // Delete key (no modifiers)
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
 * Default quick commands
 * Users can customise these or add their own
 */
export const DEFAULT_QUICK_COMMANDS: QuickCommandConfig[] = [
  {
    id: 'now',
    trigger: '/now',
    type: 'datetime',
    format: 'YYYY-MM-DD HH:mm:ss',
    description: 'Current date and time',
  },
  {
    id: 'date',
    trigger: '/date',
    type: 'date',
    format: 'YYYY-MM-DD',
    description: 'Current date',
  },
  {
    id: 'time',
    trigger: '/time',
    type: 'time',
    format: 'HH:mm:ss',
    description: 'Current time',
  },
  {
    id: 'uuid',
    trigger: '/uuid',
    type: 'uuid',
    description: 'Random UUID',
  },
  {
    id: 'hr',
    trigger: '/hr',
    type: 'text',
    value: '---',
    description: 'Horizontal rule',
  },
];

/**
 * Default user settings
 */
export const DEFAULT_SETTINGS: UserSettings = {
  language: '', // Empty = auto-detect browser language
  theme: 'auto',
  layoutMode: 'auto',
  sortOrder: 'recent',
  autoLockTimeout: 15, // 15 minutes
  fontSize: 'auto', // Auto-detects mobile and uses appropriate size
  timezone: 'local', // Use browser's local timezone by default
  syncEnabled: false,
  rememberPassword: false,
  keyboardShortcuts: DEFAULT_KEYBOARD_SHORTCUTS,
  enabledSyntaxLanguages: ['javascript', 'typescript', 'python', 'perl', 'markdown', 'json', 'xml', 'css', 'bash', 'sql', 'calc', 'outliner'], // Core languages + SQL
  defaultSyntaxLanguage: 'markdown', // Default language for new notes
  openLinksInNewTab: true, // Open external links in new tab by default
  vimMode: false, // Vim keybindings disabled by default
  quickCommandsEnabled: true, // Quick commands enabled by default
  quickCommandsList: DEFAULT_QUICK_COMMANDS, // Default quick commands
  persistSession: false, // Session persistence disabled by default
  persistSessionTimeout: 30, // 30 minutes default expiry
  colorPalette: DEFAULT_COLOR_PALETTE, // Default color palette
  tagColors: {}, // No tag colors by default
};
