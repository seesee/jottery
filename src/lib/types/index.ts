/**
 * Barrel export for all type definitions
 */

// Models
export type {
  Note,
  Attachment,
  UserSettings,
  EncryptionMetadata,
  DecryptedNote,
  NoteVersion,
  DecryptedNoteVersion,
  SearchQuery,
  SortOrder,
  Theme,
  LockState,
  ExportData,
  ExportNote,
  ExportAttachment,
  SyncOperation,
  KeyboardShortcut,
  KeyboardShortcuts,
  QuickCommandConfig,
  SavedSearch,
  ColorPalette,
} from './models';

export { DEFAULT_NOTE, DEFAULT_SETTINGS, DEFAULT_KEYBOARD_SHORTCUTS, DEFAULT_QUICK_COMMANDS, DEFAULT_COLOR_PALETTE } from './models';

// Repository interfaces
export type {
  Repository,
  NoteRepository,
  AttachmentRepository,
  SettingsRepository,
  EncryptionRepository,
  SyncRepository,
} from './repository';

// Crypto types
export type {
  MasterKey,
  KeyDerivationParams,
  EncryptionResult,
  DecryptionParams,
  CryptoService,
  KeyManager,
  ConflictResolver,
} from './crypto';

// Sync types
export type {
  SyncMetadata,
  NoteSyncMetadata,
  SyncStatus,
  SyncPushRequest,
  SyncPushResponse,
  SyncPullRequest,
  SyncPullResponse,
  SyncNote,
  SyncNoteVersion,
  SyncAttachment,
  SyncSavedSearch,
  AttachmentRef,
  SyncAccepted,
  SyncRejected,
  SyncDeletion,
  SyncStatusResponse,
  AuthRegisterRequest,
  AuthRegisterResponse,
  ConflictData,
} from './sync';
