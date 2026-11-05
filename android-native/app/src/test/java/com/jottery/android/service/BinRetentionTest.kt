package com.jottery.android.service

import androidx.room.withTransaction
import com.jottery.android.model.SyncDeletion
import com.jottery.android.model.SyncPushResponse
import com.jottery.android.testing.SyncHarness
import com.jottery.android.util.DateUtils
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.time.Duration
import java.time.Instant
import java.util.UUID

/**
 * The server isn't trusted, so what it says can put a note in the bin but never hurry
 * it out. Thirty days counts from when this device saw it go there, by this device's
 * clock, whatever the server dates a deletion or however often it repeats one.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class BinRetentionTest {
    private lateinit var h: SyncHarness
    private val id = UUID.randomUUID().toString()
    private val longAgo = "2020-01-01T00:00:00.000Z"

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    private fun purger() = NotePurger(h.noteRepo, h.versionRepo, h.attachmentRepo, h.syncRepo) { block ->
        h.db.withTransaction { block() }
    }

    private fun daysAgo(days: Long) = DateUtils.formatISO8601(Instant.now().minus(Duration.ofDays(days)))

    private fun serverDeletes(noteId: String = id, deletedAt: String = longAgo) {
        h.onPull = { SyncHarness.emptyPull().copy(deletions = listOf(SyncDeletion(noteId, deletedAt))) }
    }

    @Test fun `a deletion dated long ago doesn't start the note's time in the past`() = runBlocking {
        h.seedSynced(id, "Mine", "hash")
        serverDeletes()

        h.service().pull(fullPull = true)
        purger().purgeExpired()

        val row = h.noteRepo.getRaw(id)
        assertNotNull("still in the bin", row)
        assertTrue(row!!.deleted)
    }

    @Test fun `a server repeating a deletion can't take a note out of the bin`() = runBlocking {
        h.seedSynced(id, "Mine", "hash")
        serverDeletes()
        val service = h.service()

        service.pull(fullPull = true)
        val firstSeen = h.noteRepo.getRaw(id)!!.localDeletedAt
        repeat(3) {
            service.pull(fullPull = true)
            purger().purgeExpired()
        }

        val row = h.noteRepo.getRaw(id)
        assertNotNull("repeating it changes nothing", row)
        assertEquals("nor does it restart or shorten the clock", firstSeen, row!!.localDeletedAt)
    }

    @Test fun `a note pulled in the bin, dated long ago, gets its 30 days`() = runBlocking {
        val note = h.remoteNote(id, "Binned elsewhere", "hash", null, listOf("hash"))
            .copy(deleted = true, deletedAt = longAgo)
        h.onPull = { SyncHarness.emptyPull(note) }

        h.service().pull(fullPull = true)
        purger().purgeExpired()

        assertNotNull(h.noteRepo.getRaw(id))
    }

    @Test fun `a row with no date of this device's own is never purged on the server's`() = runBlocking {
        val note = h.seedSynced(id, "From an older build", "hash")
        h.noteRepo.insertOrReplace(note.copy(deleted = true, deletedAt = longAgo, localDeletedAt = null))

        purger().purgeExpired()

        val row = h.noteRepo.getRaw(id)
        assertNotNull("the time in deletedAt can be the server's", row)
        assertNotNull("its 30 days start now", row!!.localDeletedAt)
    }

    @Test fun `a date left on a live note can't hurry it through the bin`() = runBlocking {
        val note = h.seedSynced(id, "Restored once", "hash")
        h.noteRepo.insertOrReplace(note.copy(needsSync = true, localDeletedAt = longAgo))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), skippedDeleted = listOf(id)) }

        h.service().push()
        purger().purgeExpired()

        val row = h.noteRepo.getRaw(id)
        assertNotNull("it only went to the bin just now", row)
        assertTrue(row!!.deleted)
    }

    @Test fun `thirty days by this device's clock and it goes, with word to the others`() = runBlocking {
        val note = h.seedSynced(id, "Time's up", "hash")
        h.noteRepo.insertOrReplace(note.copy(deleted = true, deletedAt = daysAgo(31), localDeletedAt = daysAgo(31)))

        assertEquals(1, purger().purgeExpired())

        assertNull(h.noteRepo.getRaw(id))
        assertEquals(listOf(id), h.syncRepo.listDeletions().map { it.id })
    }

    @Test fun `twenty-nine days isn't thirty`() = runBlocking {
        val note = h.seedSynced(id, "Not yet", "hash")
        h.noteRepo.insertOrReplace(note.copy(deleted = true, deletedAt = longAgo, localDeletedAt = daysAgo(29)))

        assertEquals(0, purger().purgeExpired())

        assertFalse(h.noteRepo.getRaw(id) == null)
    }
}
