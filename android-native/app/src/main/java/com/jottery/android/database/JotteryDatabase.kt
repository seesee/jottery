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
    version = 1,
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

        private fun buildDatabase(context: Context): JotteryDatabase {
            return Room.databaseBuilder(
                context.applicationContext,
                JotteryDatabase::class.java,
                DATABASE_NAME,
            )
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
