package com.jottery.android.database

import androidx.room.testing.MigrationTestHelper
import androidx.test.platform.app.InstrumentationRegistry
import com.jottery.android.util.DateUtils
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.time.Duration
import java.time.Instant

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class MigrationTest {
    private val dbName = "migration-test"

    @get:Rule
    val helper = MigrationTestHelper(
        InstrumentationRegistry.getInstrumentation(),
        JotteryDatabase::class.java,
    )

    @Test fun `3 to 4 keeps every row, adds the flag and clears plaintext filenames`() {
        helper.createDatabase(dbName, 3).use { db ->
            db.execSQL(
                "INSERT INTO attachments (id, filename, mime_type, size, data) " +
                    "VALUES ('a', 'divorce-petition.pdf', 'application/pdf', 3, x'010203')"
            )
            db.execSQL(
                "INSERT INTO notes (id, created_at, modified_at, content, tags, attachments, pinned, archived, " +
                    "locked, deleted, version, word_wrap, syntax_language, show_preview, needs_sync) " +
                    "VALUES ('n', 't', 't', 'c', 'g', '[]', 0, 0, 0, 0, 1, 1, 'markdown', 0, 0)"
            )
        }

        helper.runMigrationsAndValidate(dbName, 4, true, JotteryDatabase.MIGRATION_3_4).use { db ->
            db.query("SELECT filename, mime_type, size, data, uploaded FROM attachments WHERE id = 'a'").use {
                it.moveToFirst()
                assertEquals("", it.getString(0))
                assertEquals("application/pdf", it.getString(1))
                assertEquals(3, it.getInt(2))
                assertEquals(3, it.getBlob(3).size)
                assertEquals(0, it.getInt(4))
            }
            db.query("SELECT COUNT(*) FROM notes").use {
                it.moveToFirst()
                assertEquals(1, it.getInt(0))
            }
        }
    }

    @Test fun `4 to 5 keeps the connection and starts with nothing known about the wrapped key`() {
        helper.createDatabase(dbName, 4).use { db ->
            db.execSQL(
                "INSERT INTO sync_metadata (id, last_sync_at, sync_enabled, user_id) " +
                    "VALUES (1, '2026-01-01T00:00:00.000Z', 1, 'user')"
            )
            for ((id, deleted, deletedAt) in listOf(Triple("live", 0, "NULL"), Triple("bin", 1, "'2020-01-01T00:00:00.000Z'"))) {
                db.execSQL(
                    "INSERT INTO notes (id, created_at, modified_at, content, tags, attachments, pinned, archived, " +
                        "locked, deleted, deleted_at, version, word_wrap, syntax_language, show_preview, needs_sync) " +
                        "VALUES ('$id', 't', 't', 'c', 'g', '[]', 0, 0, 0, $deleted, $deletedAt, 1, 1, 'markdown', 0, 0)"
                )
            }
        }

        helper.runMigrationsAndValidate(dbName, 5, true, JotteryDatabase.MIGRATION_4_5).use { db ->
            db.query(
                "SELECT last_sync_at, user_id, wrapped_key_opened, pending_wrapped_key, pull_cursor, cursor_adopted " +
                    "FROM sync_metadata WHERE id = 1"
            ).use {
                it.moveToFirst()
                assertEquals("2026-01-01T00:00:00.000Z", it.getString(0))
                assertEquals("user", it.getString(1))
                assertEquals(0, it.getInt(2))
                assertEquals(true, it.isNull(3))
                assertEquals(true, it.isNull(4))
                assertEquals(0, it.getInt(5))
            }
            db.query("SELECT local_deleted_at FROM notes WHERE id = 'live'").use {
                it.moveToFirst()
                assertEquals(true, it.isNull(0))
            }
            // A date from the server mustn't purge it, so its time starts now
            db.query("SELECT deleted_at, local_deleted_at FROM notes WHERE id = 'bin'").use {
                it.moveToFirst()
                assertEquals("2020-01-01T00:00:00.000Z", it.getString(0))
                val binned = DateUtils.parseISO8601(it.getString(1))!!
                assertTrue(Duration.between(binned, Instant.now()).abs() < Duration.ofMinutes(5))
            }
        }
    }
}
