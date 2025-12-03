/**
 * Repository pattern interfaces for data access abstraction
 * Allows for future backend changes without affecting business logic
 */

import type { Note, UserSettings, EncryptionMetadata } from './models';

/**
 * Base repository interface
 */
export interface Repository<T> {
  getAll(): Promise<T[]>;
  getById(id: string): Promise<T | null>;
  create(item: T): Promise<T>;
  update(item: T): Promise<T>;
  delete(id: string): Promise<void>;
}

/**
 * Note repository with specialized query methods
 */
export interface NoteRepository extends Repository<Note> {
  /**
   * Get notes excluding deleted ones
   */
  getAllActive(): Promise<Note[]>;

  /**
   * Get deleted notes (recycle bin)
   */
  getDeleted(): Promise<Note[]>;

  /**
   * Get pinned notes
   */
  getPinned(): Promise<Note[]>;

  /**
   * Soft delete a note
   */
  softDelete(id: string): Promise<void>;

  /**
   * Restore a soft-deleted note
   */
  restore(id: string): Promise<void>;

  /**
   * Permanently delete a note
   */
  permanentDelete(id: string): Promise<void>;

  /**
   * Search notes by IDs
   */
  getByIds(ids: string[]): Promise<Note[]>;

  /**
   * Update note timestamp
   */
  touch(id: string): Promise<void>;

  /**
   * Get notes modified after a timestamp (for sync)
   */
  getModifiedAfter(timestamp: string): Promise<Note[]>;
}

/**
 * Attachment blob storage repository
 */
export interface AttachmentRepository {
  /**
   * Store attachment blob data
   */
  storeBlob(id: string, data: ArrayBuffer): Promise<void>;

  /**
   * Retrieve attachment blob data
   */
  getBlob(id: string): Promise<ArrayBuffer | null>;

  /**
   * Delete attachment blob
   */
  deleteBlob(id: string): Promise<void>;

  /**
   * Store thumbnail blob
   */
  storeThumbnail(id: string, data: ArrayBuffer): Promise<void>;

  /**
   * Retrieve thumbnail blob
   */
  getThumbnail(id: string): Promise<ArrayBuffer | null>;
}

/**
 * Settings repository
 */
export interface SettingsRepository {
  /**
   * Get current user settings
   */
  get(): Promise<UserSettings>;

  /**
   * Update user settings
   */
  update(settings: Partial<UserSettings>): Promise<UserSettings>;

  /**
   * Reset to default settings
   */
  reset(): Promise<UserSettings>;
}

/**
 * Encryption metadata repository
 */
export interface EncryptionRepository {
  /**
   * Get encryption metadata
   */
  getMetadata(): Promise<EncryptionMetadata | null>;

  /**
   * Store encryption metadata
   */
  setMetadata(metadata: EncryptionMetadata): Promise<void>;

  /**
   * Check if encryption is initialized
   */
  isInitialized(): Promise<boolean>;
}

/**
 * Sync operations repository (Phase 3)
 */
export interface SyncRepository {
  /**
   * Get pending sync operations
   */
  getPendingOperations(): Promise<any[]>;

  /**
   * Mark operation as synced
   */
  markSynced(operationId: string): Promise<void>;

  /**
   * Mark operation as failed
   */
  markFailed(operationId: string, error: string): Promise<void>;
}
