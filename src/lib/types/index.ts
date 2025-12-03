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
  SearchQuery,
  SortOrder,
  Theme,
  LockState,
  ExportData,
  ExportNote,
  ExportAttachment,
  SyncOperation,
} from './models';

export { DEFAULT_NOTE, DEFAULT_SETTINGS } from './models';

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
