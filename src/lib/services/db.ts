/**
 * IndexedDB database initialization and management
 */

import { openDB, type IDBPDatabase } from 'idb';
import type { Note, UserSettings, EncryptionMetadata } from '../types';

const DB_NAME = 'jottery';
const DB_VERSION = 1;

// Object store names
export const STORES = {
  NOTES: 'notes',
  ATTACHMENTS: 'attachments',
  THUMBNAILS: 'thumbnails',
  SETTINGS: 'settings',
  ENCRYPTION: 'encryption',
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
}

let dbInstance: IDBPDatabase<JotteryDB> | null = null;

/**
 * Initialize and open the database
 */
export async function initDB(): Promise<IDBPDatabase<JotteryDB>> {
  if (dbInstance) {
    return dbInstance;
  }

  dbInstance = await openDB<JotteryDB>(DB_NAME, DB_VERSION, {
    upgrade(db, oldVersion) {
      // Version 1: Initial schema
      if (oldVersion < 1) {
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

      // Future version upgrades can be added here
      // if (oldVersion < 2) { ... }
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
