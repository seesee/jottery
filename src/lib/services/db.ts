/**
 * IndexedDB database initialization and management
 */

import { openDB, type IDBPDatabase } from 'idb';
import type { Note, UserSettings, EncryptionMetadata, NoteVersion, SavedSearch, SyncMetadata, NoteSyncMetadata } from '../types';

let DB_NAME = 'jottery'; // Default, can be changed before initialization
const DB_VERSION = 9;

// Track if database was terminated unexpectedly
let dbTerminated = false;

// Object store names
export const STORES = {
  NOTES: 'notes',
  ATTACHMENTS: 'attachments',
  THUMBNAILS: 'thumbnails',
  SETTINGS: 'settings',
  ENCRYPTION: 'encryption',
  SYNC_METADATA: 'sync_metadata',
  NOTE_VERSIONS: 'note_versions',
  SAVED_SEARCHES: 'saved_searches',
  DELETIONS: 'deletions',
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
      archived: number; // 0 or 1 for filtering
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
    value: SyncMetadata | NoteSyncMetadata;
  };
  note_versions: {
    key: string; // versionKey
    value: NoteVersion;
    indexes: {
      noteId: string;
      createdAt: string;
      'noteId-version': [string, number];
    };
  };
  saved_searches: {
    key: string; // UUID
    value: SavedSearch;
    indexes: {
      order: number;
      deleted: number;
      modifiedAt: string;
      'deleted-order': [number, number];
      needsSync: number;
    };
  };
  deletions: {
    key: string; // Note UUID that was deleted
    value: {
      id: string;
      deletedAt: string;
      synced: boolean;
    };
    indexes: {
      synced: number; // 0 or 1 for filtering
    };
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

  console.log(`[Jottery Client v${__APP_VERSION__}] Opening database. Current version: ${DB_VERSION}`);

  dbInstance = await openDB<JotteryDB>(DB_NAME, DB_VERSION, {
    async upgrade(db, oldVersion, newVersion, transaction) {
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

      // Version 3: Add note versions store
      if (oldVersion < 3) {
        console.log('[DB] Creating v3 schema (note_versions store)...');
        const versionsStore = db.createObjectStore(STORES.NOTE_VERSIONS, {
          keyPath: 'versionKey',
        });
        versionsStore.createIndex('noteId', 'noteId');
        versionsStore.createIndex('createdAt', 'createdAt');
        versionsStore.createIndex('noteId-version', ['noteId', 'version'], { unique: true });
        console.log('[DB] note_versions store created successfully');
      }

      // Version 4: Ensure note_versions store exists (fix for v3 migration issues)
      if (oldVersion < 4) {
        console.log('[DB] Applying v4 schema updates...');
        // Check if note_versions exists, create if missing
        if (!db.objectStoreNames.contains(STORES.NOTE_VERSIONS)) {
          console.log('[DB] Creating note_versions store (was missing from v3)...');
          const versionsStore = db.createObjectStore(STORES.NOTE_VERSIONS, {
            keyPath: 'versionKey',
          });
          versionsStore.createIndex('noteId', 'noteId');
          versionsStore.createIndex('createdAt', 'createdAt');
          versionsStore.createIndex('noteId-version', ['noteId', 'version'], { unique: true });
          console.log('[DB] note_versions store created successfully');
        } else {
          console.log('[DB] note_versions store already exists, no changes needed');
        }
      }

      // Version 5: Add needsSync index for faster sync queries
      if (oldVersion < 5) {
        console.log('[DB] Applying v5 schema updates (performance indices)...');

        // Access the notes object store through the upgrade transaction
        if (db.objectStoreNames.contains(STORES.NOTES)) {
          const notesStore = transaction.objectStore(STORES.NOTES);

          // Add index for needsSync flag (improves getNotesNeedingSync query)
          if (!notesStore.indexNames.contains('needsSync')) {
            notesStore.createIndex('needsSync', 'needsSync');
            console.log('[DB] Created needsSync index');
          }

          // Add compound index for efficient sync queries (needsSync + deleted)
          if (!notesStore.indexNames.contains('needsSync-deleted')) {
            notesStore.createIndex('needsSync-deleted', ['needsSync', 'deleted']);
            console.log('[DB] Created needsSync-deleted compound index');
          }
        }
        console.log('[DB] v5 schema updates complete');
      }

      // Version 6: Add saved_searches store
      if (oldVersion < 6) {
        console.log('[DB] Creating v6 schema (saved_searches store)...');
        const savedSearchesStore = db.createObjectStore(STORES.SAVED_SEARCHES, {
          keyPath: 'id',
        });

        savedSearchesStore.createIndex('order', 'order');
        savedSearchesStore.createIndex('deleted', 'deleted');
        savedSearchesStore.createIndex('modifiedAt', 'modifiedAt');
        savedSearchesStore.createIndex('deleted-order', ['deleted', 'order']);
        savedSearchesStore.createIndex('needsSync', 'needsSync');
        console.log('[DB] saved_searches store created successfully');
      }

      // Version 7: Defensive fix - ensure saved_searches exists (fix corrupted v6 databases)
      if (oldVersion < 7) {
        console.log('[DB] Applying v7 schema updates (defensive saved_searches check)...');

        // Check if saved_searches exists, create if missing
        if (!db.objectStoreNames.contains(STORES.SAVED_SEARCHES)) {
          console.log('[DB] saved_searches store is missing from v6 database, creating it now...');
          const savedSearchesStore = db.createObjectStore(STORES.SAVED_SEARCHES, {
            keyPath: 'id',
          });

          savedSearchesStore.createIndex('order', 'order');
          savedSearchesStore.createIndex('deleted', 'deleted');
          savedSearchesStore.createIndex('modifiedAt', 'modifiedAt');
          savedSearchesStore.createIndex('deleted-order', ['deleted', 'order']);
          savedSearchesStore.createIndex('needsSync', 'needsSync');
          console.log('[DB] saved_searches store created successfully (defensive fix)');
        } else {
          console.log('[DB] saved_searches store already exists, no migration needed');
        }

        console.log('[DB] v7 schema updates complete');
      }

      // Version 8: Add archive support (archived field and index)
      if (oldVersion < 8) {
        console.log('[DB] Applying v8 schema updates (archive support)...');

        // Add archived index to notes store
        if (db.objectStoreNames.contains(STORES.NOTES)) {
          const notesStore = transaction.objectStore(STORES.NOTES);

          if (!notesStore.indexNames.contains('archived')) {
            notesStore.createIndex('archived', 'archived');
            console.log('[DB] Created archived index');
          }

          // Add compound index for efficient archived queries
          if (!notesStore.indexNames.contains('archived-modifiedAt')) {
            notesStore.createIndex('archived-modifiedAt', ['archived', 'modifiedAt']);
            console.log('[DB] Created archived-modifiedAt compound index');
          }
        }

        // Migrate existing notes to add archived field (default: false)
        const notesStore = transaction.objectStore(STORES.NOTES);
        let notesCursor = await notesStore.openCursor();
        let migratedCount = 0;

        while (notesCursor) {
          const note = notesCursor.value;
          if (note.archived === undefined) {
            note.archived = false;
            note.archivedAt = undefined;
            await notesCursor.update(note);
            migratedCount++;
          }
          notesCursor = await notesCursor.continue();
        }

        console.log(`[DB] Migrated ${migratedCount} notes to add archived field`);
        console.log('[DB] v8 schema updates complete');
      }

      // Version 9: Add deletions store for tracking hard-deleted notes (for sync)
      if (oldVersion < 9) {
        console.log('[DB] Creating v9 schema (deletions store)...');

        const deletionsStore = db.createObjectStore(STORES.DELETIONS, {
          keyPath: 'id',
        });
        deletionsStore.createIndex('synced', 'synced');

        console.log('[DB] deletions store created successfully');
        console.log('[DB] v9 schema updates complete');
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
      console.error('Database connection terminated unexpectedly. Please refresh the page.');
      dbInstance = null;
      dbTerminated = true;
    },
  });

  console.log(`[DB] Database opened successfully. Version: ${dbInstance.version}`);
  console.log(`[DB] Available stores: ${[...dbInstance.objectStoreNames].join(', ')}`);

  // Verify required stores exist (defensive check)
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

  if (!dbInstance.objectStoreNames.contains(STORES.NOTE_VERSIONS)) {
    console.error(`[DB] ERROR: note_versions store is missing! Database is in inconsistent state.`);
    console.error(`[DB] This likely means you need to do a hard refresh (Ctrl+Shift+R / Cmd+Shift+R)`);
    console.error(`[DB] to load the updated code with the database migration.`);
    console.error(`[DB] Current database version: ${dbInstance.version}, expected: ${DB_VERSION}`);
    throw new Error(
      'Database is in inconsistent state. The note_versions store is missing. ' +
      'Please do a hard refresh (Ctrl+Shift+R or Cmd+Shift+R) to load the updated code.'
    );
  }

  if (!dbInstance.objectStoreNames.contains(STORES.SAVED_SEARCHES)) {
    console.error(`[DB] ERROR: saved_searches store is missing! Database is in inconsistent state.`);
    console.error(`[DB] This should have been fixed by v7 migration.`);
    console.error(`[DB] Please do a hard refresh (Ctrl+Shift+R / Cmd+Shift+R)`);
    console.error(`[DB] Current database version: ${dbInstance.version}, expected: ${DB_VERSION}`);
    throw new Error(
      'Database is in inconsistent state. The saved_searches store is missing. ' +
      'Please do a hard refresh (Ctrl+Shift+R or Cmd+Shift+R) to load the updated code.'
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
 * Check if database is available (initialized and not terminated)
 */
export function isDBAvailable(): boolean {
  return dbInstance !== null && !dbTerminated;
}

/**
 * Check if database was terminated unexpectedly
 */
export function wasDBTerminated(): boolean {
  return dbTerminated;
}

/**
 * Set the database name (must be called before initDB)
 * Allows different notebook paths to use different databases
 */
export function setDatabaseName(name: string): void {
  if (dbInstance) {
    throw new Error('Cannot change database name after initialization');
  }
  DB_NAME = name;
}

/**
 * Get the current database name
 */
export function getDatabaseName(): string {
  return DB_NAME;
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
 * Clear all data from all stores (for restore operations)
 * This is faster than deleting and recreating the database
 */
export async function clearAllStores(): Promise<void> {
  const db = getDB();
  const storeNames = Object.values(STORES);

  console.log('[DB] Clearing all stores:', storeNames);

  for (const storeName of storeNames) {
    try {
      await db.clear(storeName);
      console.log(`[DB] Cleared store: ${storeName}`);
    } catch (error) {
      console.warn(`[DB] Failed to clear store ${storeName}:`, error);
    }
  }

  console.log('[DB] All stores cleared');
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
