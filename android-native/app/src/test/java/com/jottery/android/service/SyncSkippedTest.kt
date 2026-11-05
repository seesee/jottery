package com.jottery.android.service

import com.jottery.android.model.ConflictInfo
import com.jottery.android.model.ConflictResolutionStrategy
import com.jottery.android.model.SavedSearch
import com.jottery.android.model.SyncPushResponse
import com.jottery.android.testing.SyncHarness
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
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
import java.time.Instant
import java.time.temporal.ChronoUnit
import java.util.UUID

/** What a server leaves out of `accepted` and `rejected`, and what that has to mean. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class SyncSkippedTest {
    private lateinit var h: SyncHarness
    private val id = UUID.randomUUID().toString()

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    private suspend fun queued(noteId: String = id, modifiedAt: String? = null) {
        val note = h.seedSynced(noteId, "Queued", "hash-$noteId")
        h.noteRepo.insertOrReplace(note.copy(needsSync = true, modifiedAt = modifiedAt ?: note.modifiedAt))
    }

    private fun skipEverything() {
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), skippedDeleted = listOf(id)) }
    }

    private fun inDays(days: Long): String = Instant.now().plus(days, ChronoUnit.DAYS).toString()

    // skippedDeleted

    @Test fun `a note deleted elsewhere goes to the bin, and isn't sent again`() = runBlocking {
        queued()
        skipEverything()

        h.service().fullSync()
        h.service().fullSync()

        val row = h.noteRepo.getRaw(id)!!
        assertTrue("kept for the user, because a server can't be trusted to delete", row.deleted)
        assertNotNull(row.localDeletedAt)
        assertFalse(row.needsSync)
        assertEquals("sent once", 1, h.pushBodies.size)
    }

    @Test fun `a note already in the bin here keeps its place in the queue for purging`() = runBlocking {
        val note = h.seedSynced(id, "Binned", "hash")
        val binnedAt = "2026-01-05T00:00:00.000Z"
        h.noteRepo.insertOrReplace(note.copy(needsSync = true, deleted = true, deletedAt = binnedAt, localDeletedAt = binnedAt))
        skipEverything()

        h.service().fullSync()

        val row = h.noteRepo.getRaw(id)!!
        assertEquals(binnedAt, row.localDeletedAt)
        assertEquals(binnedAt, row.deletedAt)
        assertFalse(row.needsSync)
    }

    @Test fun `an edit made since the push survives, and goes up again`() = runBlocking {
        queued()
        h.onPush = {
            runBlocking {
                val row = h.noteRepo.getRaw(id)!!
                h.noteRepo.insertOrReplace(row.copy(modifiedAt = "2026-02-02T00:00:00.000Z"))
            }
            SyncPushResponse(emptyList(), emptyList(), skippedDeleted = listOf(id))
        }

        h.service().push()

        val row = h.noteRepo.getRaw(id)!!
        assertFalse("later than the deletion, so the server will take it", row.deleted)
        assertTrue(row.needsSync)
    }

    @Test fun `a server can only name notes this push sent`() = runBlocking {
        val other = UUID.randomUUID().toString()
        h.seedSynced(other, "Not part of this push", "other-hash")
        queued()
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), skippedDeleted = listOf(other, "not-a-uuid")) }

        h.service().push()

        assertFalse(h.noteRepo.getRaw(other)!!.deleted)
        assertFalse(h.noteRepo.getRaw(id)!!.deleted)
    }

    @Test fun `many notes going this way is worth mentioning, like any other deletion`() = runBlocking {
        val ids = List(SyncService.NOTABLE_BIN_COUNT) { UUID.randomUUID().toString() }
        ids.forEach { queued(it) }
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), skippedDeleted = ids) }
        var told: Int? = null
        val service = h.service()
        service.onManyNotesBinned = { told = it }

        service.fullSync()

        assertEquals(ids.size, told)
    }

    @Test fun `an older server's response reads as nothing skipped`() {
        val parsed = Json { ignoreUnknownKeys = true }
            .decodeFromString(SyncPushResponse.serializer(), """{"accepted":[],"rejected":[]}""")

        assertNull(parsed.skippedDeleted)
    }

    // A clock that's wrong

    private suspend fun syncCountingNotices(service: SyncService = h.service()): Int {
        var notices = 0
        service.onClockLooksWrong = { notices++ }
        service.fullSync()
        return notices
    }

    private fun serverTimeIs(time: String) {
        h.onPull = { SyncHarness.emptyPull().copy(syncedAt = time) }
    }

    @Test fun `a note the server won't answer for, dated days ahead of it, means the clock may be wrong`() = runBlocking {
        queued(modifiedAt = inDays(3))
        serverTimeIs(inDays(0))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), errors = listOf("Skipped note: timestamp in the future")) }

        assertEquals(1, syncCountingNotices())
        assertTrue("it stays, for when the clock is right", h.noteRepo.getRaw(id)!!.needsSync)
    }

    @Test fun `it's the server's clock that counts, because this device's is in doubt`() = runBlocking {
        queued(modifiedAt = inDays(0))
        serverTimeIs(inDays(-3))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), errors = listOf("skipped")) }

        assertEquals(1, syncCountingNotices())
    }

    @Test fun `any of a note's times can be the one in the future`() = runBlocking {
        val note = h.seedSynced(id, "Archived in the future", "hash")
        h.noteRepo.insertOrReplace(note.copy(needsSync = true, archived = true, archivedAt = inDays(3)))
        serverTimeIs(inDays(0))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), errors = listOf("skipped")) }

        assertEquals(1, syncCountingNotices())
    }

    @Test fun `within a day of the server is no cause for alarm`() = runBlocking {
        queued(modifiedAt = Instant.now().plus(23, ChronoUnit.HOURS).toString())
        serverTimeIs(inDays(0))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), errors = listOf("skipped")) }

        assertEquals(0, syncCountingNotices())
    }

    @Test fun `an answered note says nothing about the clock`() = runBlocking {
        queued(modifiedAt = inDays(3))
        serverTimeIs(inDays(0))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), skippedDeleted = listOf(id)) }

        assertEquals(0, syncCountingNotices())
    }

    @Test fun `one notice an hour is enough, however often it syncs`() = runBlocking {
        queued(modifiedAt = inDays(3))
        serverTimeIs(inDays(0))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), errors = listOf("skipped")) }
        val service = h.service()

        assertEquals(1, syncCountingNotices(service))
        assertEquals(0, syncCountingNotices(service))
    }

    // Saved searches, which have no accepted list of their own

    private suspend fun queuedSearch(modifiedAt: String): SavedSearch {
        val search = h.savedSearchRepo.create("Work", "#work", h.key)
        val row = h.savedSearchRepo.listNeedingSync().single { it.id == search.id }.copy(modifiedAt = modifiedAt)
        h.db.savedSearchDao().insertOrReplace(row)
        return row
    }

    @Test fun `a saved search the server skipped stays queued`() = runBlocking {
        queuedSearch(inDays(3))
        serverTimeIs(inDays(0))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), errors = listOf("skipped")) }

        assertEquals(1, syncCountingNotices())
        assertEquals(1, h.savedSearchRepo.listNeedingSync().size)
    }

    @Test fun `a saved search the server took is done with`() = runBlocking {
        queuedSearch(inDays(0))
        serverTimeIs(inDays(0))

        assertEquals(0, syncCountingNotices())
        assertTrue(h.savedSearchRepo.listNeedingSync().isEmpty())
    }

    // What the web track's fixes needed here as well

    @Test fun `a note waiting on a conflict isn't replaced, even with nothing queued`() = runBlocking {
        val server = h.remoteNote(id, "First server version", "server-hash", null, listOf("server-hash"))
        h.seedSynced(id, "Mine", "local-hash")
        val service = h.service()
        h.noteRepo.insertOrReplace(h.noteRepo.getRaw(id)!!.copy(needsSync = true))
        h.onPush = { SyncPushResponse(emptyList(), listOf(h.rejection(server))) }
        service.push()
        h.noteRepo.updateRaw(h.noteRepo.getRaw(id)!!.copy(needsSync = false))

        // Newer, and includes this device's hash, so without a conflict it would be taken
        val newer = h.remoteNote(id, "Second server version", "newer-hash", "server-hash", listOf("newer-hash", "local-hash"))
        h.onPull = { SyncHarness.emptyPull(newer) }
        service.pull()

        assertEquals("Mine", h.noteRepo.get(id, h.key)!!.content)
        assertEquals("the server's side stays current", "newer-hash", h.syncRepo.listStoredConflicts().single().serverContentHash)
    }

    @Test fun `version tags go out one ciphertext to a tag, as notes' do`() = runBlocking {
        val note = h.noteRepo.create("With tags", listOf("one", "two"), h.key)
        h.noteRepo.update(note.copy(content = "Edited"), h.key)
        assertTrue(h.versionRepo.getVersions(note.id).isNotEmpty())

        h.service().push()

        val versions = Json.parseToJsonElement(h.pushBodies.first()).jsonObject["versions"]!!.jsonArray
        assertTrue(versions.isNotEmpty())
        for (version in versions) {
            val tags = version.jsonObject["tags"]!!.jsonArray.map { it.jsonPrimitive.content }
            assertEquals(setOf("one", "two"), tags.map { h.decryptTag(it) }.toSet())
        }
    }

    // Rejections of a note the server has in its bin

    @Test fun `a conflict knows when the server's copy is in the bin`() = runBlocking {
        queued()
        val deletedAt = "2026-01-20T00:00:00.000Z"
        val server = h.remoteNote(id, "Server copy", "server-hash", null, listOf("server-hash"))
        h.onPush = {
            SyncPushResponse(emptyList(), listOf(h.rejection(server).copy(serverDeleted = true, serverDeletedAt = deletedAt)))
        }
        val conflicts = mutableListOf<ConflictInfo>()
        val service = h.service()
        service.onConflictDetected = { conflicts += it }

        service.push()
        service.resolveConflict(conflicts.single(), ConflictResolutionStrategy.KEEP_SERVER)

        val row = h.noteRepo.getRaw(id)!!
        assertTrue("taking the server's side takes it to the bin", row.deleted)
        assertEquals(deletedAt, row.deletedAt)
        assertNotNull("or it would never be purged", row.localDeletedAt)
    }

    @Test fun `an older server's rejection reads as not in the bin`() = runBlocking {
        queued()
        val server = h.remoteNote(id, "Server copy", "server-hash", null, listOf("server-hash"))
        h.onPush = { SyncPushResponse(emptyList(), listOf(h.rejection(server))) }
        val conflicts = mutableListOf<ConflictInfo>()
        val service = h.service()
        service.onConflictDetected = { conflicts += it }

        service.push()
        service.resolveConflict(conflicts.single(), ConflictResolutionStrategy.KEEP_SERVER)

        assertFalse(h.noteRepo.getRaw(id)!!.deleted)
    }

    @Test fun `a deletion time that won't parse isn't stored`() = runBlocking {
        queued()
        val server = h.remoteNote(id, "Server copy", "server-hash", null, listOf("server-hash"))
        h.onPush = {
            SyncPushResponse(emptyList(), listOf(h.rejection(server).copy(serverDeleted = true, serverDeletedAt = "<script>")))
        }

        h.service().push()

        assertNull(h.syncRepo.listStoredConflicts().single().serverDeletedAt)
    }
}
