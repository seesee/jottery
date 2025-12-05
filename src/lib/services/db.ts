/**
 * IndexedDB database initialization and management
 */

import { openDB, type IDBPDatabase } from 'idb';
import type { Note, UserSettings, EncryptionMetadata } from '../types';

const DB_NAME = 'jottery';
const DB_VERSION = 2;

// Object store names
export const STORES = {
  NOTES: 'notes',
  ATTACHMENTS: 'attachments',
  THUMBNAILS: 'thumbnails',
  SETTINGS: 'settings',
  ENCRYPTION: 'encryption',
  SYNC_METADATA: 'sync_metadata',
} as const;

export interface JotteryDB {
  notes: {
    key: string;
    value: Note;
    indexes: {
      modifiedAt: string;
      createdAt: string;
      deleted: number; // 0 or 1 for filtering
      pinned: number; // 0 or 1 for filtering
    };
  };
  attachments: {
    key: string; // attachment ID
    value: ArrayBuffer;
  };
  thumbnails: {
    key: string; // attachment ID
    value: ArrayBuffer;
  };
  settings: {
    key: string; // Always 'user-settings'
    value: UserSettings;
  };
  encryption: {
    key: string; // Always 'metadata'
    value: EncryptionMetadata;
  };
  sync_metadata: {
    key: string; // 'metadata' or 'note:<uuid>'
    value: any; // SyncMetadata | NoteSyncMetadata (will be defined in types/sync.ts)
  };
}

let dbInstance: IDBPDatabase<JotteryDB> | null = null;

/**
 * Initialize and open the database
 */
export async function initDB(): Promise<IDBPDatabase<JotteryDB>> {
  if (dbInstance) {
    return dbInstance;
  }

  console.log(`[Jottery Client v0.1.6] Opening database. Current version: ${DB_VERSION}`);

  dbInstance = await openDB<JotteryDB>(DB_NAME, DB_VERSION, {
    upgrade(db, oldVersion, newVersion) {
      console.log(`[DB] Upgrading database from v${oldVersion} to v${newVersion}`);

      // Version 1: Initial schema
      if (oldVersion < 1) {
        console.log('[DB] Creating v1 schema...');
        // Notes store
        const notesStore = db.createObjectStore(STORES.NOTES, {
          keyPath: 'id',
        });
        notesStore.createIndex('modifiedAt', 'modifiedAt');
        notesStore.createIndex('createdAt', 'createdAt');
        notesStore.createIndex('deleted', 'deleted');
        notesStore.createIndex('pinned', 'pinned');

        // Compound index for active notes sorting
        notesStore.createIndex('deleted-modifiedAt', ['deleted', 'modifiedAt']);

        // Attachments store (blob storage)
        db.createObjectStore(STORES.ATTACHMENTS);

        // Thumbnails store (blob storage)
        db.createObjectStore(STORES.THUMBNAILS);

        // Settings store
        db.createObjectStore(STORES.SETTINGS);

        // Encryption metadata store
        db.createObjectStore(STORES.ENCRYPTION);
      }

      // Version 2: Add sync metadata store
      if (oldVersion < 2) {
        console.log('[DB] Creating v2 schema (sync_metadata store)...');
        db.createObjectStore(STORES.SYNC_METADATA);
        console.log('[DB] sync_metadata store created successfully');
      }

      console.log('[DB] Upgrade complete');
    },
    blocked() {
      console.warn(
        'Database upgrade blocked. Please close other tabs with this app open.'
      );
    },
    blocking() {
      console.warn('This tab is blocking a database upgrade in another tab.');
      // Close the database to allow upgrade
      if (dbInstance) {
        dbInstance.close();
        dbInstance = null;
      }
    },
    terminated() {
      console.error('Database connection terminated unexpectedly.');
      dbInstance = null;
    },
  });

  console.log(`[DB] Database opened successfully. Version: ${dbInstance.version}`);
  console.log(`[DB] Available stores: ${[...dbInstance.objectStoreNames].join(', ')}`);

  // Verify sync_metadata store exists (defensive check)
  if (!dbInstance.objectStoreNames.contains(STORES.SYNC_METADATA)) {
    console.error(`[DB] ERROR: sync_metadata store is missing! Database is in inconsistent state.`);
    console.error(`[DB] Please delete the database and reload. Instructions:`);
    console.error(`[DB] 1. Open DevTools > Application > IndexedDB`);
    console.error(`[DB] 2. Right-click "${DB_NAME}" and select "Delete database"`);
    console.error(`[DB] 3. Reload the page`);
    throw new Error(
      'Database is in inconsistent state. The sync_metadata store is missing. ' +
      'Please delete the IndexedDB database and reload the page.'
    );
  }

  return dbInstance;
}

/**
 * Get the database instance (must call initDB first)
 */
export function getDB(): IDBPDatabase<JotteryDB> {
  if (!dbInstance) {
    throw new Error('Database not initialized. Call initDB() first.');
  }
  return dbInstance;
}

/**
 * Close the database connection
 */
export function closeDB(): void {
  if (dbInstance) {
    dbInstance.close();
    dbInstance = null;
  }
}

/**
 * Delete the entire database (for testing or reset)
 */
export async function deleteDB(): Promise<void> {
  closeDB();
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.deleteDatabase(DB_NAME);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
    request.onblocked = () => {
      console.warn('Database deletion blocked. Close all tabs with this app.');
    };
  });
}
