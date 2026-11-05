package com.jottery.android.database

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import com.jottery.android.database.dao.AttachmentDao
import com.jottery.android.database.dao.EncryptionMetadataDao
import com.jottery.android.database.dao.NoteDeletionDao
import com.jottery.android.database.dao.NoteDao
import com.jottery.android.database.dao.NoteSyncMetadataDao
import com.jottery.android.database.dao.NoteVersionDao
import com.jottery.android.database.dao.SavedSearchDao
import com.jottery.android.database.dao.SettingsDao
import com.jottery.android.database.dao.SyncMetadataDao
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.NoteDeletion
import com.jottery.android.model.NoteRecord
import com.jottery.android.model.NoteSyncMetadata
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.model.SavedSearch
import com.jottery.android.model.SyncMetadata
import com.jottery.android.model.UserSettings

/**
 * Room database matching the iOS GRDB schema exactly.
 * All 9 tables with identical column names, types, and defaults.
 */
@Database(
    entities = [
        NoteRecord::class,
        NoteVersionRecord::class,
        AttachmentRecord::class,
        UserSettings::class,
        EncryptionMetadata::class,
        SyncMetadata::class,
        NoteSyncMetadata::class,
        NoteDeletion::class,
        SavedSearch::class,
    ],
    version = 5,
    exportSchema = true,
)
abstract class JotteryDatabase : RoomDatabase() {

    abstract fun noteDao(): NoteDao
    abstract fun noteVersionDao(): NoteVersionDao
    abstract fun attachmentDao(): AttachmentDao
    abstract fun settingsDao(): SettingsDao
    abstract fun encryptionMetadataDao(): EncryptionMetadataDao
    abstract fun syncMetadataDao(): SyncMetadataDao
    abstract fun noteSyncMetadataDao(): NoteSyncMetadataDao
    abstract fun noteDeletionDao(): NoteDeletionDao
    abstract fun savedSearchDao(): SavedSearchDao

    companion object {
        private const val DATABASE_NAME = "jottery.db"

        @Volatile
        private var INSTANCE: JotteryDatabase? = null

        fun getInstance(context: Context): JotteryDatabase {
            return INSTANCE ?: synchronized(this) {
                INSTANCE ?: buildDatabase(context).also { INSTANCE = it }
            }
        }

        /** Close and clear the singleton so the next getInstance rebuilds it. */
        fun destroyInstance() {
            synchronized(this) {
                INSTANCE?.close()
                INSTANCE = null
            }
        }

        /** Migration 1→2: Add envelope encryption columns and userId. */
        internal val MIGRATION_1_2 = object : Migration(1, 2) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL("ALTER TABLE encryption_metadata ADD COLUMN envelope_version INTEGER")
                db.execSQL("ALTER TABLE encryption_metadata ADD COLUMN device_salt TEXT")
                db.execSQL("ALTER TABLE encryption_metadata ADD COLUMN local_wrapped_master TEXT")
                db.execSQL("ALTER TABLE encryption_metadata ADD COLUMN wrapping_kdf_version INTEGER")
                db.execSQL("ALTER TABLE sync_metadata ADD COLUMN user_id TEXT")
            }
        }

        /** Migration 2→3: Make salt and iterations nullable for envelope encryption. */
        internal val MIGRATION_2_3 = object : Migration(2, 3) {
            override fun migrate(db: SupportSQLiteDatabase) {
                // SQLite doesn't support ALTER COLUMN, so recreate the table
                db.execSQL("""
                    CREATE TABLE encryption_metadata_new (
                        id INTEGER NOT NULL PRIMARY KEY,
                        salt TEXT,
                        iterations INTEGER,
                        created_at TEXT NOT NULL,
                        algorithm TEXT NOT NULL DEFAULT 'AES-256-GCM',
                        verification TEXT,
                        envelope_version INTEGER,
                        device_salt TEXT,
                        local_wrapped_master TEXT,
                        wrapping_kdf_version INTEGER
                    )
                """)
                db.execSQL("""
                    INSERT INTO encryption_metadata_new
                    SELECT id, salt, iterations, created_at, algorithm,
                           verification, envelope_version, device_salt,
                           local_wrapped_master, wrapping_kdf_version
                    FROM encryption_metadata
                """)
                db.execSQL("DROP TABLE encryption_metadata")
                db.execSQL("ALTER TABLE encryption_metadata_new RENAME TO encryption_metadata")
            }
        }

        /**
         * Migration 3→4: track which blobs the server already holds, and clear the
         * plaintext filenames earlier builds left here. The encrypted name is in
         * the note's attachment reference, and nothing reads this column.
         */
        internal val MIGRATION_3_4 = object : Migration(3, 4) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL("ALTER TABLE attachments ADD COLUMN uploaded INTEGER NOT NULL DEFAULT 0")
                // The filenames are cleared later, a few at a time. Attachment data
                // lives in its row, so changing a row's size rewrites the data too,
                // and a migration doing that to a whole vault can hold up the app
                // for long enough to matter. See AttachmentDao.clearOldFilenames.
            }
        }

        /**
         * Migration 4→5: remember what this device knows about the account's
         * wrapped key and how far it has pulled, and when each note in the bin
         * arrived there. Notes already in the bin get today, because their
         * deleted_at may have come from the server: they stay a little longer.
         */
        internal val MIGRATION_4_5 = object : Migration(4, 5) {
            override fun migrate(db: SupportSQLiteDatabase) {
                db.execSQL("ALTER TABLE sync_metadata ADD COLUMN wrapped_key_opened INTEGER NOT NULL DEFAULT 0")
                db.execSQL("ALTER TABLE sync_metadata ADD COLUMN pending_wrapped_key TEXT")
                db.execSQL("ALTER TABLE sync_metadata ADD COLUMN pull_cursor TEXT")
                db.execSQL("ALTER TABLE sync_metadata ADD COLUMN cursor_adopted INTEGER NOT NULL DEFAULT 0")
                db.execSQL("ALTER TABLE notes ADD COLUMN local_deleted_at TEXT")
                db.execSQL(
                    "UPDATE notes SET local_deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE deleted = 1"
                )
            }
        }

        private fun buildDatabase(context: Context): JotteryDatabase {
            return Room.databaseBuilder(
                context.applicationContext,
                JotteryDatabase::class.java,
                DATABASE_NAME,
            )
                .addMigrations(MIGRATION_1_2, MIGRATION_2_3, MIGRATION_3_4, MIGRATION_4_5)
                .addCallback(object : Callback() {
                    override fun onCreate(db: SupportSQLiteDatabase) {
                        super.onCreate(db)
                        // Enable foreign keys
                        db.execSQL("PRAGMA foreign_keys = ON")

                        // Insert default settings row (singleton id=1)
                        db.execSQL("""
                            INSERT OR IGNORE INTO settings
                            (id, language, theme, sort_order, auto_lock_timeout, sync_enabled)
                            VALUES (1, 'system', 'auto', 'recent', 15, 0)
                        """)

                        // Insert default sync_metadata row (singleton id=1)
                        db.execSQL("""
                            INSERT OR IGNORE INTO sync_metadata (id, sync_enabled)
                            VALUES (1, 0)
                        """)
                    }
                })
                .build()
        }

        /** For testing only — in-memory database. */
        fun buildInMemory(context: Context): JotteryDatabase {
            return Room.inMemoryDatabaseBuilder(
                context.applicationContext,
                JotteryDatabase::class.java,
            )
                .allowMainThreadQueries()
                .addCallback(object : Callback() {
                    override fun onCreate(db: SupportSQLiteDatabase) {
                        super.onCreate(db)
                        db.execSQL("PRAGMA foreign_keys = ON")
                        db.execSQL("""
                            INSERT OR IGNORE INTO settings
                            (id, language, theme, sort_order, auto_lock_timeout, sync_enabled)
                            VALUES (1, 'system', 'auto', 'recent', 15, 0)
                        """)
                        db.execSQL("""
                            INSERT OR IGNORE INTO sync_metadata (id, sync_enabled)
                            VALUES (1, 0)
                        """)
                    }
                })
                .build()
        }
    }
}
