package com.jottery.android.database

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Attachment data lives in its row, so blanking a filename rewrites the data too. That's
 * why this happens a few rows at a time, after launch, and never in a migration.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class AttachmentFilenameCleanupTest {
    private lateinit var database: JotteryDatabase

    @Before fun setUp() {
        database = Room.inMemoryDatabaseBuilder(
            ApplicationProvider.getApplicationContext<Context>(),
            JotteryDatabase::class.java,
        ).allowMainThreadQueries().build()
        val db = database.openHelper.writableDatabase
        repeat(12) { index ->
            db.execSQL(
                "INSERT INTO attachments (id, filename, mime_type, size, data, uploaded) " +
                    "VALUES ('a$index', 'scan.pdf', 'application/pdf', 1, x'00', 0)"
            )
        }
    }

    @After fun tearDown() = database.close()

    @Test fun `old names are cleared a few at a time`() = runBlocking {
        val dao = database.attachmentDao()

        assertEquals(5, dao.clearOldFilenames(5))
        assertEquals(5, dao.clearOldFilenames(5))
        assertEquals(2, dao.clearOldFilenames(5))
        assertEquals(0, dao.clearOldFilenames(5))
    }
}
