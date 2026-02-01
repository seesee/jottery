/**
 * Sync-related types and interfaces
 */

// Sync metadata stored in IndexedDB
export interface SyncMetadata {
  lastSyncAt?: string;           // ISO 8601 - last successful sync
  lastPushAt?: string;            // ISO 8601 - last push attempt
  lastPullAt?: string;            // ISO 8601 - last pull attempt
  apiKey?: string;                // Encrypted API key (JSON stringified EncryptionResult)
  clientId?: string;              // UUID assigned by server
  userId?: string;                // User ID (for multi-user systems)
  userEmail?: string;             // User email (for multi-user systems)
  syncEnabled: boolean;
  syncEndpoint: string;
  autoSyncInterval?: number;      // Minutes (0 = disabled, default: 5)
  pendingDeviceName?: string;     // Device name for pending clone-device call (during import)
}

// Per-note sync tracking
export interface NoteSyncMetadata {
  noteId: string;
  syncedAt: string;               // ISO 8601
  syncHash: string;               // SHA-256 of encrypted content
  serverVersion: number;          // Version on server
  lastSyncStatus: 'synced' | 'pending' | 'conflict' | 'error';
  errorMessage?: string;
  conflictData?: ConflictData;    // Server version data when in conflict
}

// Current sync status for UI display
export interface SyncStatus {
  isEnabled: boolean;
  isSyncing: boolean;
  lastSyncAt?: string;
  lastError?: string;
  pendingNotes: number;
  conflictCount: number;
  clientId?: string;
  syncEndpoint?: string;
}

// Sync saved search structure (matches server expectations)
export interface SyncSavedSearch {
  id: string;
  name: string;
  query: string;
  order: number;
  createdAt: string;
  modifiedAt: string;
  deleted: boolean;
  deletedAt?: string;
  version: number;
}

// Push request payload
export interface SyncPushRequest {
  notes: SyncNote[];
  attachments: SyncAttachment[];
  versions: SyncNoteVersion[];
  savedSearches?: SyncSavedSearch[];
  /** Hard-deleted note IDs (from emptying trash) */
  deletions?: SyncDeletion[];
}

// Note structure for sync (matches server expectations)
export interface SyncNote {
  id: string;
  createdAt: string;
  modifiedAt: string;
  content: string;              // Encrypted JSON string
  tags: string[];               // Array of encrypted JSON strings
  attachments: AttachmentRef[];
  pinned: boolean;
  archived: boolean;
  archivedAt?: string;
  locked?: boolean;
  lockedAt?: string;
  deleted: boolean;
  deletedAt?: string;
  version: number;
  wordWrap?: boolean;
  syntaxLanguage?: string;
  showPreview?: boolean;
  color?: string;               // Semantic color name (unencrypted)
}

// Attachment reference (metadata only)
export interface AttachmentRef {
  id: string;
  filename: string;             // Encrypted
  mimeType: string;
  size: number;
  data: string;                 // Reference ID
}

// Attachment with binary data (for sync transfer)
export interface SyncAttachment {
  id: string;
  data: string;                 // Base64 encoded encrypted blob
}

// Push response from server
export interface SyncPushResponse {
  accepted: SyncAccepted[];
  rejected: SyncRejected[];
  errors?: string[]; // Optional, for backward compatibility
}

// Accepted note info
export interface SyncAccepted {
  id: string;
  serverVersion?: number; // Optional for backward compatibility
  syncedAt?: string; // Optional for backward compatibility
}

// Rejected note info (conflict) - includes full server note data for resolution
export interface SyncRejected {
  id: string;
  reason: string;
  serverModifiedAt: string;
  // Server note data for conflict resolution
  serverContent: string;           // Encrypted content
  serverTags: string[];            // Encrypted tags
  serverVersion: number;
  serverAttachments: AttachmentRef[];
  serverPinned: boolean;
  serverSyntaxLanguage?: string;
  serverWordWrap?: boolean;
  serverShowPreview?: boolean;
}

// Stored conflict data for resolution UI
export interface ConflictData {
  noteId: string;
  serverContent: string;           // Encrypted content
  serverTags: string[];            // Encrypted tags
  serverModifiedAt: string;
  serverVersion: number;
  serverAttachments: AttachmentRef[];
  serverPinned: boolean;
  serverSyntaxLanguage?: string;
  serverWordWrap?: boolean;
  serverShowPreview?: boolean;
  detectedAt: string;              // When conflict was detected (ISO 8601)
}

// Pull request payload
export interface SyncPullRequest {
  lastSyncAt?: string;
  knownNoteIds: string[];
  knownAttachmentIds: string[];
  // Pagination support for large datasets
  limit?: number;
  offset?: number;
}

// Pull response from server
export interface SyncPullResponse {
  notes: SyncNote[];
  deletions?: SyncDeletion[]; // Optional for backward compatibility
  attachments: SyncAttachment[];
  versions: SyncNoteVersion[];
  savedSearches?: SyncSavedSearch[];
  syncedAt?: string; // Optional for backward compatibility
  // Pagination metadata for large datasets
  totalCount?: number;
  hasMore?: boolean;
}

// Deleted note info
export interface SyncDeletion {
  id: string;
  deletedAt: string;
}

// Note version for sync
export interface SyncNoteVersion {
  versionKey: string;        // `${noteId}:${version}`
  noteId: string;
  version: number;
  createdAt: string;
  syncedAt: string;
  content: string;           // Encrypted JSON string
  tags: string[];            // Array of encrypted JSON strings
  attachments: AttachmentRef[];
  syntaxLanguage?: string;
  wordWrap?: boolean;
  showPreview?: boolean;
  color?: string;            // Semantic color name (unencrypted)
  reason: 'sync' | 'manual-sync';
}

// Server status response
export interface SyncStatusResponse {
  clientId: string;
  deviceName: string;
  serverLastModified: string;
  noteCount: number;
  lastSyncedAt?: string;
}

// Authentication/registration types
export interface AuthRegisterRequest {
  deviceName: string;
  deviceType: 'web' | 'cli';
}

export interface AuthRegisterResponse {
  apiKey: string;
  clientId: string;
  createdAt?: string; // Optional for backward compatibility
}
