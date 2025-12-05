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
  syncEnabled: boolean;
  syncEndpoint: string;
  autoSyncInterval?: number;      // Minutes (0 = disabled, default: 5)
}

// Per-note sync tracking
export interface NoteSyncMetadata {
  noteId: string;
  syncedAt: string;               // ISO 8601
  syncHash: string;               // SHA-256 of encrypted content
  serverVersion: number;          // Version on server
  lastSyncStatus: 'synced' | 'pending' | 'conflict' | 'error';
  errorMessage?: string;
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

// Push request payload
export interface SyncPushRequest {
  notes: SyncNote[];
  attachments: SyncAttachment[];
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
  deleted: boolean;
  deletedAt?: string;
  version: number;
  wordWrap?: boolean;
  syntaxLanguage?: string;
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
  errors: string[];
}

// Accepted note info
export interface SyncAccepted {
  id: string;
  serverVersion: number;
  syncedAt: string;
}

// Rejected note info (conflict)
export interface SyncRejected {
  id: string;
  reason: string;
  serverModifiedAt: string;
}

// Pull request payload
export interface SyncPullRequest {
  lastSyncAt?: string;
  knownNoteIds: string[];
}

// Pull response from server
export interface SyncPullResponse {
  notes: SyncNote[];
  deletions: SyncDeletion[];
  attachments: SyncAttachment[];
  syncedAt: string;
}

// Deleted note info
export interface SyncDeletion {
  id: string;
  deletedAt: string;
}

// Server status response
export interface SyncStatusResponse {
  clientId: string;
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
  createdAt: string;
}
