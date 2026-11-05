package com.jottery.android.service

import androidx.room.withTransaction
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.ConflictInfo
import com.jottery.android.model.ConflictResolutionStrategy
import com.jottery.android.model.NoteDeletion
import com.jottery.android.model.SyncAccepted
import com.jottery.android.model.SyncAttachment
import com.jottery.android.model.SyncDeletion
import com.jottery.android.model.SyncPullRequest
import com.jottery.android.model.SyncPushRequest
import com.jottery.android.model.SyncPushResponse
import com.jottery.android.testing.SyncHarness
import com.jottery.android.util.DateUtils
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
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
import java.util.Base64

/** docs/sync-protocol.md, including against a server that predates its additions. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class SyncProtocolTest {
    private lateinit var h: SyncHarness
    private val parser = Json { ignoreUnknownKeys = true }
    private val noteId = "11111111-1111-4111-8111-111111111111"

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    private fun pullRequest(body: String) = parser.decodeFromString<SyncPullRequest>(body)
    private fun pushRequest(body: String) = parser.decodeFromString<SyncPushRequest>(body)
    private fun id(n: Int) = "00000000-0000-4000-8000-%012d".format(n)

    private fun accepted(request: SyncPushRequest) = SyncPushResponse(
        accepted = request.notes.map { SyncAccepted(it.id, 2, "2026-02-01T00:00:00.000Z") },
        rejected = emptyList(),
        rejectedDeletions = emptyList(),
    )

    private suspend fun seedAdopted(cursor: String) {
        h.syncRepo.updateLastSyncAt("2026-01-01T00:00:00.000Z")
        h.syncRepo.savePullCursor(cursor)
    }

    private fun ref(id: String) =
        AttachmentRef(id = id, filename = h.encrypt("f.txt"), mimeType = "text/plain", size = 1, data = id)

    // MARK: - Cursor

    @Test fun `the first pull starts a cursor, and the next sends it back`() = runBlocking {
        h.onPull = { SyncHarness.emptyPull().copy(nextCursor = "c1") }
        h.service().pull()
        h.service().pull()

        assertEquals("", pullRequest(h.pullBodies[0]).cursor)
        assertEquals("c1", pullRequest(h.pullBodies[1]).cursor)
    }

    @Test fun `known note ids aren't sent, but the field is, as older servers require it`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash")
        h.service().pull()

        assertTrue(h.pullBodies[0].contains("\"knownNoteIds\":[]"))
    }

    @Test fun `a page with no notes doesn't end the pull when the server has more`() = runBlocking {
        h.onPull = { body ->
            when (pullRequest(body).cursor) {
                "" -> SyncHarness.emptyPull().copy(hasMore = true, nextCursor = "c1")
                else -> SyncHarness.emptyPull(h.remoteNote(noteId, "AFTER AN EMPTY PAGE", "h", null, listOf("h")))
                    .copy(nextCursor = "c2")
            }
        }

        h.service().pull()

        assertEquals("AFTER AN EMPTY PAGE", h.noteRepo.get(noteId, h.key)?.content)
        assertEquals("c2", h.syncRepo.getSyncMetadata()!!.pullCursor)
    }

    @Test fun `a cursor is saved only after its page is applied`() = runBlocking {
        h.onPull = { body ->
            when (pullRequest(body).cursor) {
                "" -> SyncHarness.emptyPull(h.remoteNote(id(1), "ONE", "h1", null, listOf("h1")))
                    .copy(hasMore = true, nextCursor = "c1")
                else -> throw IllegalStateException("network gone")
            }
        }

        runCatching { h.service().pull() }

        assertEquals("the applied page sticks", "c1", h.syncRepo.getSyncMetadata()!!.pullCursor)
        assertNotNull(h.noteRepo.getRaw(id(1)))
    }

    @Test fun `a page that fails to apply leaves the cursor where it was`() = runBlocking {
        seedAdopted("c0")
        h.onPull = { SyncHarness.emptyPull().copy(nextCursor = "c1") }

        runCatching { h.service(transaction = { throw IllegalStateException("disk full") }).pull() }

        assertEquals("c0", h.syncRepo.getSyncMetadata()!!.pullCursor)
    }

    @Test fun `a repeated page is harmless`() = runBlocking {
        val remote = h.remoteNote(noteId, "SAME", "h", null, listOf("h"))
        h.onPull = { SyncHarness.emptyPull(remote).copy(nextCursor = "c1") }
        h.service().pull()
        val first = h.noteRepo.getRaw(noteId)

        h.service().pull()

        assertEquals(first, h.noteRepo.getRaw(noteId))
    }

    @Test fun `the watermark keeps following the server, for a fallback`() = runBlocking {
        h.onPull = { SyncHarness.emptyPull().copy(nextCursor = "c1") }

        h.service().pull()

        assertEquals("2026-03-01T00:00:00.000Z", h.syncRepo.getSyncMetadata()!!.lastSyncAt)
    }

    @Test fun `the watermark is never the device clock`() = runBlocking {
        h.onPull = { SyncHarness.emptyPull().copy(syncedAt = null) }

        h.service().pull()

        assertNull(h.syncRepo.getSyncMetadata()!!.lastSyncAt)
    }

    @Test fun `a server stuck on one cursor doesn't loop forever`() = runBlocking {
        seedAdopted("c1")
        h.onPull = { SyncHarness.emptyPull().copy(hasMore = true, nextCursor = "c1") }

        h.service().pull()

        assertEquals(1, h.pullBodies.size)
        assertEquals("a stalled pull doesn't claim to be finished",
            "2026-01-01T00:00:00.000Z", h.syncRepo.getSyncMetadata()!!.lastSyncAt)
    }

    @Test fun `a hostile cursor is ignored`() = runBlocking {
        h.onPull = { SyncHarness.emptyPull().copy(nextCursor = "bad\ncursor") }

        h.service().pull()

        assertNull(h.syncRepo.getSyncMetadata()!!.pullCursor)
    }

    // MARK: - First adoption

    @Test fun `the first sight of a cursor triggers one full pull, and only one`() = runBlocking {
        h.syncRepo.updateLastSyncAt("2026-01-01T00:00:00.000Z")
        h.onPull = { SyncHarness.emptyPull().copy(nextCursor = "c1") }

        h.service().pull()

        assertEquals(2, h.pullBodies.size)
        assertEquals("2026-01-01T00:00:00.000Z", pullRequest(h.pullBodies[0]).lastSyncAt)
        assertEquals("", pullRequest(h.pullBodies[1]).cursor)
        assertNull("the repair fetches everything", pullRequest(h.pullBodies[1]).lastSyncAt)

        h.service().pull()

        assertEquals(3, h.pullBodies.size)
        assertEquals("c1", pullRequest(h.pullBodies[2]).cursor)
    }

    @Test fun `a new device needs no repair`() = runBlocking {
        h.onPull = { SyncHarness.emptyPull().copy(nextCursor = "c1") }

        h.service().pull()

        assertEquals(1, h.pullBodies.size)
    }

    // MARK: - An older server

    @Test fun `an older server pages by offset and is never sent a full pull`() = runBlocking {
        h.syncRepo.updateLastSyncAt("2026-01-01T00:00:00.000Z")
        h.onPull = { body ->
            if (pullRequest(body).offset == 0) {
                SyncHarness.emptyPull(h.remoteNote(id(1), "ONE", "h1", null, listOf("h1"))).copy(hasMore = true)
            } else {
                SyncHarness.emptyPull(h.remoteNote(id(2), "TWO", "h2", null, listOf("h2")))
            }
        }

        h.service().pull()
        h.onPull = { SyncHarness.emptyPull() }
        h.service().pull()

        assertEquals(1, pullRequest(h.pullBodies[1]).offset)
        assertEquals("the watermark from before, not a restart",
            "2026-01-01T00:00:00.000Z", pullRequest(h.pullBodies[1]).lastSyncAt)
        assertEquals("the server's time", "2026-03-01T00:00:00.000Z", pullRequest(h.pullBodies[2]).lastSyncAt)
        assertNotNull(h.noteRepo.getRaw(id(1)))
        assertNotNull(h.noteRepo.getRaw(id(2)))
    }

    @Test fun `a server that loses the cursor falls back to the watermark`() = runBlocking {
        seedAdopted("c1")
        h.onPull = { SyncHarness.emptyPull() }

        h.service().pull()

        assertNull(h.syncRepo.getSyncMetadata()!!.pullCursor)
        assertEquals("2026-03-01T00:00:00.000Z", h.syncRepo.getSyncMetadata()!!.lastSyncAt)
    }

    @Test fun `a push response with no rejectedDeletions is an empty list`() = runBlocking {
        h.syncRepo.addDeletion(NoteDeletion(noteId, "2026-02-01T00:00:00.000Z"))
        h.onPush = { SyncPushResponse(emptyList(), emptyList()) }

        h.service().push()

        assertTrue(h.syncRepo.listDeletions().isEmpty())
    }

    @Test fun `a page too big for memory is asked for again in smaller pieces`() = runBlocking {
        // Real OutOfMemoryErrors can't be raised reliably, so this goes through the
        // limit logic instead: a heavy page shrinks the next one.
        val big = "A".repeat((SyncService.PULL_PAGE_BYTES + 1).toInt())
        h.onPull = { body ->
            if (pullRequest(body).cursor == "") {
                SyncHarness.emptyPull().copy(
                    attachments = listOf(SyncAttachment(id(9), big)),
                    hasMore = true,
                    nextCursor = "c1",
                )
            } else {
                SyncHarness.emptyPull().copy(nextCursor = "c2")
            }
        }

        h.service().pull()

        assertEquals(SyncService.PULL_PAGE_SIZE / 2, pullRequest(h.pullBodies[1]).limit)
    }

    // MARK: - Attachments

    @Test fun `a push says its attachment lists are complete`() = runBlocking {
        h.noteRepo.create("note", emptyList(), h.key)
        h.onPush = { accepted(pushRequest(it)) }

        h.service().push()

        assertTrue(pushRequest(h.pushBodies.first()).attachmentsAuthoritative)
    }

    @Test fun `a list that can't be read isn't called complete, because it goes out empty`() = runBlocking {
        val note = h.noteRepo.create("note", emptyList(), h.key)
        h.noteRepo.updateRaw(h.noteRepo.getRaw(note.id)!!.copy(attachments = "not json"))
        h.onPush = { accepted(pushRequest(it)) }

        h.service().push()

        assertFalse(pushRequest(h.pushBodies.first()).attachmentsAuthoritative)
    }

    @Test fun `a shorter list from the server drops the blob, even with an unchanged hash`() = runBlocking {
        val blobId = id(5)
        h.attachmentRepo.insertOrReplace(AttachmentRecord(blobId, "", "", 1, byteArrayOf(1)))
        val record = h.seedSynced(noteId, "BASE", "hash")
        h.noteRepo.updateRaw(record.copy(attachments = Json.encodeToString(listOf(ref(blobId)))))

        h.onPull = { SyncHarness.emptyPull(h.remoteNote(noteId, "BASE", "hash", null, listOf("hash"))) }
        h.service().pull()

        assertEquals("[]", h.noteRepo.getRaw(noteId)!!.attachments)
        assertNull(h.attachmentRepo.getBlob(blobId))
    }

    @Test fun `a blob a version still needs survives the note dropping it`() = runBlocking {
        val blobId = id(5)
        h.attachmentRepo.insertOrReplace(AttachmentRecord(blobId, "", "", 1, byteArrayOf(1)))
        val record = h.seedSynced(noteId, "BASE", "hash")
        val withRef = record.copy(attachments = Json.encodeToString(listOf(ref(blobId))))
        h.noteRepo.updateRaw(withRef)
        h.versionRepo.insertOrReplace(com.jottery.android.model.NoteVersionRecord.from(withRef, "sync"))

        h.onPull = { SyncHarness.emptyPull(h.remoteNote(noteId, "BASE", "hash", null, listOf("hash"))) }
        h.service().pull()

        assertNotNull(h.attachmentRepo.getBlob(blobId))
    }

    @Test fun `keeping the local side doesn't prune what another device added`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        val note = h.noteRepo.get(noteId, h.key)!!
        h.noteRepo.update(note.copy(content = "LOCAL"), h.key)
        val serverRef = ref(id(6))
        val remote = h.remoteNote(noteId, "REMOTE", "hash-r", "hash-base", listOf("hash-r", "hash-base"))
            .copy(attachments = listOf(serverRef))
        h.onPull = { SyncHarness.emptyPull(remote) }

        val service = h.service()
        val conflicts = mutableListOf<ConflictInfo>()
        service.onConflictDetected = { conflicts += it }
        service.pull()
        service.resolveConflict(conflicts.single(), ConflictResolutionStrategy.KEEP_LOCAL)

        assertTrue(h.noteRepo.getRaw(noteId)!!.attachments.contains(serverRef.id))
    }

    @Test fun `a blob the pull didn't send is fetched, and a 404 isn't asked for twice`() = runBlocking {
        val here = id(7)
        val gone = id(8)
        val data = Base64.getEncoder().encodeToString(byteArrayOf(9, 9))
        h.serverBlobs[here] = data
        val record = h.seedSynced(noteId, "BASE", "hash")
        h.noteRepo.updateRaw(record.copy(attachments = Json.encodeToString(listOf(ref(here), ref(gone)))))

        val service = h.service()
        service.pull()
        service.pull()

        assertEquals(2, h.attachmentRepo.getBlob(here)!!.sum() / 9)
        assertEquals(listOf(here, gone).sorted(), h.blobRequests.sorted())
    }

    @Test fun `a blob served for the wrong id isn't stored`() = runBlocking {
        val asked = id(7)
        h.serverBlobs[asked] = "not base64 !!"
        val record = h.seedSynced(noteId, "BASE", "hash")
        h.noteRepo.updateRaw(record.copy(attachments = Json.encodeToString(listOf(ref(asked)))))

        h.service().pull()

        assertNull(h.attachmentRepo.getBlob(asked))
    }

    // MARK: - Deletions

    private fun purger() = NotePurger(h.noteRepo, h.versionRepo, h.attachmentRepo, h.syncRepo) { block ->
        h.db.withTransaction { block() }
    }

    @Test fun `a permanent delete reaches the server`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash")

        purger().deletePermanently(listOf(noteId))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), rejectedDeletions = emptyList()) }
        h.service().push()

        assertEquals(listOf(noteId), pushRequest(h.pushBodies.single()).deletions.map { it.id })
        assertTrue("sent once", h.syncRepo.listDeletions().isEmpty())
    }

    private suspend fun importTheNote() {
        val file = Json.encodeToString(
            ExportService.ExportPayload(
                notes = listOf(
                    ExportService.ExportNote(
                        id = noteId,
                        createdAt = "2026-01-01T00:00:00.000Z",
                        modifiedAt = "2026-01-01T00:00:00.000Z",
                        content = "BACK",
                    )
                ),
            )
        )
        ImportService(h.noteRepo, h.attachmentRepo) { block -> h.db.withTransaction { block() } }
            .import(file, h.key, ImportStrategy.SKIP)
    }

    @Test fun `a queued deletion isn't sent for a note that came back`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash")
        purger().deletePermanently(listOf(noteId))
        importTheNote()
        h.onPush = { accepted(pushRequest(it)) }

        h.service().push()

        val requests = h.pushBodies.map(::pushRequest)
        assertEquals("the older copy would lose to it, on every device", emptyList<SyncDeletion>(), requests.flatMap { it.deletions })
        assertEquals(listOf(noteId), requests.flatMap { it.notes }.map { it.id })
    }

    @Test fun `a note restored over a queued deletion still takes edits from the server`() = runBlocking {
        // Restoring a backup writes the row back as it was
        val saved = h.seedSynced(noteId, "BASE", "hash")
        purger().deletePermanently(listOf(noteId))
        h.noteRepo.insertOrReplace(saved)
        val edit = h.remoteNote(noteId, "EDITED", "hash-edit", "hash", listOf("hash-edit", "hash"))
        h.onPull = { SyncHarness.emptyPull(edit) }

        h.service().pull()

        assertEquals("EDITED", h.noteRepo.get(noteId, h.key)?.content)
    }

    @Test fun `a note that came back and is deleted again is deleted everywhere`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash")
        purger().deletePermanently(listOf(noteId))
        importTheNote()
        purger().deletePermanently(listOf(noteId))
        h.onPush = { SyncPushResponse(emptyList(), emptyList()) }

        h.service().push()

        assertEquals(listOf(noteId), pushRequest(h.pushBodies.single()).deletions.map { it.id })
    }

    @Test fun `a permanent delete before the soft delete synced still reaches the server`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash")
        h.noteRepo.softDelete(noteId)

        purger().deletePermanently(listOf(noteId))
        h.onPush = { SyncPushResponse(emptyList(), emptyList()) }
        h.service().push()

        assertEquals(listOf(noteId), pushRequest(h.pushBodies.single()).deletions.map { it.id })
    }

    @Test fun `a refused deletion isn't sent again, and the note comes back through the pull`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash")
        purger().deletePermanently(listOf(noteId))
        h.onPush = { SyncPushResponse(emptyList(), emptyList(), rejectedDeletions = listOf(noteId)) }
        h.onPull = {
            SyncHarness.emptyPull(h.remoteNote(noteId, "NEWER EDIT", "h2", "hash", listOf("h2", "hash")))
        }

        h.service().fullSync()
        h.service().fullSync()

        assertEquals("sent once only", 1, h.pushBodies.size)
        assertEquals("NEWER EDIT", h.noteRepo.get(noteId, h.key)?.content)
    }

    @Test fun `a note deleted here isn't brought back by an older copy before the push lands`() = runBlocking {
        h.syncRepo.addDeletion(NoteDeletion(noteId, DateUtils.nowISO8601()))
        h.onPull = {
            SyncHarness.emptyPull(
                h.remoteNote(noteId, "OLD", "h", null, listOf("h"), modifiedAt = "2020-01-01T00:00:00.000Z")
            )
        }

        h.service().pull()

        assertNull(h.noteRepo.getRaw(noteId))
    }

    @Test fun `a permanent delete removes versions, the conflict and unshared blobs`() = runBlocking {
        val own = id(1)
        val shared = id(2)
        listOf(own, shared).forEach {
            h.attachmentRepo.insertOrReplace(AttachmentRecord(it, "", "", 1, byteArrayOf(1)))
        }
        val record = h.seedSynced(noteId, "BASE", "hash")
        val withRefs = record.copy(attachments = Json.encodeToString(listOf(ref(own), ref(shared))))
        h.noteRepo.updateRaw(withRefs)
        h.versionRepo.insertOrReplace(com.jottery.android.model.NoteVersionRecord.from(withRefs, "sync"))
        val other = h.seedSynced(id(3), "OTHER", "hash2")
        h.noteRepo.updateRaw(other.copy(attachments = Json.encodeToString(listOf(ref(shared)))))

        purger().deletePermanently(listOf(noteId))

        assertNull(h.noteRepo.getRaw(noteId))
        assertTrue(h.versionRepo.getVersions(noteId).isEmpty())
        assertNull(h.attachmentRepo.getBlob(own))
        assertNotNull("another note still uses it", h.attachmentRepo.getBlob(shared))
    }

    @Test fun `the bin is purged by this device's clock, not the date the server sent`() = runBlocking {
        // R2-069: a server sets an old deletedAt to skip the bin
        val remote = h.remoteNote(noteId, "TEXT", "h", null, listOf("h")).copy(
            deleted = true,
            deletedAt = "2020-01-01T00:00:00.000Z",
        )
        h.onPull = { SyncHarness.emptyPull(remote) }
        h.service().pull()

        assertEquals(0, purger().purgeExpired())
        assertNotNull(h.noteRepo.getRaw(noteId))

        val longAgo = DateUtils.formatISO8601(Instant.now().minus(Duration.ofDays(31)))
        h.noteRepo.updateRaw(h.noteRepo.getRaw(noteId)!!.copy(localDeletedAt = longAgo))

        assertEquals(1, purger().purgeExpired())
        assertNull(h.noteRepo.getRaw(noteId))
        assertEquals("2020-01-01T00:00:00.000Z", h.syncRepo.listDeletions().single().deletedAt)
    }

    @Test fun `a server listing every note leaves them all in the bin, and says so`() = runBlocking {
        val ids = (1..SyncService.NOTABLE_BIN_COUNT).map { id(it) }
        ids.forEach { h.seedSynced(it, "TEXT", "hash-$it") }
        h.onPull = {
            SyncHarness.emptyPull().copy(deletions = ids.map { SyncDeletion(it, "2026-02-01T00:00:00.000Z") })
        }
        var reported = 0
        val service = h.service()
        service.onManyNotesBinned = { reported = it }

        service.pull()

        ids.forEach { assertTrue(h.noteRepo.getRaw(it)!!.deleted) }
        assertEquals(ids.size, reported)
    }

    @Test fun `a deletion with an unreadable time keeps the note`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash")
        h.onPull = { SyncHarness.emptyPull().copy(deletions = listOf(SyncDeletion(noteId, "yesterday"))) }

        h.service().pull()

        assertFalse(h.noteRepo.getRaw(noteId)!!.deleted)
    }

    @Test fun `an edit older than the deletion goes to the bin, so it can't stay queued for ever`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash")
        val note = h.noteRepo.get(noteId, h.key)!!
        h.noteRepo.update(note.copy(content = "LOCAL"), h.key)
        val later = DateUtils.formatISO8601(Instant.now().plus(Duration.ofDays(1)))
        h.onPull = { SyncHarness.emptyPull().copy(deletions = listOf(SyncDeletion(noteId, later))) }

        h.service().pull()

        val row = h.noteRepo.getRaw(noteId)!!
        assertTrue(row.deleted)
        assertFalse(row.needsSync)
        assertEquals("the text is still there", "LOCAL", h.noteRepo.get(noteId, h.key)?.content)
    }

    @Test fun `a note in conflict waits for the user, whatever the server deletes`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        val note = h.noteRepo.get(noteId, h.key)!!
        h.noteRepo.update(note.copy(content = "LOCAL"), h.key)
        val remote = h.remoteNote(noteId, "REMOTE", "hash-r", "hash-base", listOf("hash-r", "hash-base"))
        h.onPull = { SyncHarness.emptyPull(remote) }
        h.service().pull()

        val later = DateUtils.formatISO8601(Instant.now().plus(Duration.ofDays(1)))
        h.onPull = { SyncHarness.emptyPull().copy(deletions = listOf(SyncDeletion(noteId, later))) }
        h.service().pull()

        assertFalse(h.noteRepo.getRaw(noteId)!!.deleted)
    }

    // MARK: - A full account

    /** The server's rule: anything that adds to the account fails. */
    private fun quota(body: String): SyncPushResponse {
        val request = pushRequest(body)
        val grows = request.attachments.isNotEmpty() || request.versions.isNotEmpty() ||
            request.notes.any { !it.deleted }
        if (grows) throw h.storageFull()
        return accepted(request)
    }

    @Test fun `a full account can still send what the user binned to make room`() = runBlocking {
        h.seedSynced(noteId, "OLD", "h0")
        h.versionRepo.insertOrReplace(
            com.jottery.android.model.NoteVersionRecord.from(h.noteRepo.getRaw(noteId)!!, "sync")
        )
        h.noteRepo.softDelete(noteId)
        h.noteRepo.create("a new note that won't fit", emptyList(), h.key)
        h.onPush = ::quota

        val failure = runCatching { h.service().push() }.exceptionOrNull()

        assertEquals(507, (failure as com.jottery.android.network.SyncClientException).statusCode)
        assertEquals("storage_quota_exceeded", failure.code)
        assertFalse("the binned note reached the server", h.noteRepo.getRaw(noteId)!!.needsSync)
        assertEquals(1, h.noteRepo.listNeedingSync().size)
    }

    @Test fun `a full account still sends permanent deletes`() = runBlocking {
        h.syncRepo.addDeletion(NoteDeletion(noteId, DateUtils.nowISO8601()))
        h.noteRepo.create("a new note that won't fit", emptyList(), h.key)
        h.onPush = ::quota

        runCatching { h.service().push() }

        assertTrue(h.syncRepo.listDeletions().isEmpty())
    }

    @Test fun `a note stays queued if its blob couldn't follow it`() = runBlocking {
        val note = h.noteRepo.create("note", emptyList(), h.key)
        val blobId = id(5)
        h.attachmentRepo.insertOrReplace(AttachmentRecord(blobId, "", "", 1, byteArrayOf(1)))
        h.noteRepo.addAttachment(note.id, ref(blobId), h.key)
        h.onPush = { body ->
            val request = pushRequest(body)
            if (request.attachments.isNotEmpty()) throw h.storageFull()
            accepted(request)
        }

        runCatching { h.service().push() }

        assertTrue("nothing else would ever send the blob", h.noteRepo.getRaw(note.id)!!.needsSync)

        h.onPush = { accepted(pushRequest(it)) }
        h.service().push()

        assertTrue(h.pushBodies.last().contains(blobId))
        assertFalse(h.noteRepo.getRaw(note.id)!!.needsSync)
    }

    // MARK: - Many saves between syncs

    /** The server's rule: its hash has to be the parent, or somewhere in the chain. */
    private fun serverWouldAccept(request: SyncPushRequest, serverHash: String) = request.notes.single().let {
        it.parentHash == serverHash || it.contentHash == serverHash || serverHash in it.hashChain.orEmpty()
    }

    @Test fun `any number of saves between syncs is still accepted`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        repeat(120) { n ->
            val note = h.noteRepo.get(noteId, h.key)!!
            h.noteRepo.update(note.copy(content = "save $n"), h.key)
        }
        h.onPush = { accepted(pushRequest(it)) }

        h.service().push()

        val pushed = pushRequest(h.pushBodies.first())
        assertTrue("rejected as a conflict on a note nothing else touched", serverWouldAccept(pushed, "hash-base"))
        assertTrue(pushed.notes.single().hashChain!!.size <= 50)
    }

    @Test fun `the same holds when the server's hash came from a push here`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        h.noteRepo.update(h.noteRepo.get(noteId, h.key)!!.copy(content = "first"), h.key)
        val onServer = h.noteRepo.getRaw(noteId)!!.contentHash!!
        h.onPush = { accepted(pushRequest(it)) }
        h.service().push()

        repeat(120) { n ->
            h.noteRepo.update(h.noteRepo.get(noteId, h.key)!!.copy(content = "save $n"), h.key)
        }
        h.pushBodies.clear()
        h.service().push()

        assertTrue(serverWouldAccept(pushRequest(h.pushBodies.first()), onServer))
    }

    @Test fun `the same holds when the server's hash came from a pull`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        val remote = h.remoteNote(noteId, "REMOTE", "hash-r", "hash-base", listOf("hash-r", "hash-base"))
        h.onPull = { SyncHarness.emptyPull(remote) }
        h.service().pull()

        repeat(120) { n ->
            h.noteRepo.update(h.noteRepo.get(noteId, h.key)!!.copy(content = "save $n"), h.key)
        }
        h.onPush = { accepted(pushRequest(it)) }
        h.service().push()

        assertTrue(serverWouldAccept(pushRequest(h.pushBodies.first()), "hash-r"))
    }

    // MARK: - Applying a pulled note

    @Test fun `an edit made between push and pull survives the echo`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        val note = h.noteRepo.get(noteId, h.key)!!
        h.noteRepo.update(note.copy(content = "PUSHED"), h.key)
        val pushed = h.noteRepo.getRaw(noteId)!!

        h.onPush = { accepted(pushRequest(it)) }
        h.onPull = { _ ->
            // The push is over and marked synced. The user types, then the pull answers.
            runBlocking {
                Thread.sleep(5)
                val current = h.noteRepo.get(noteId, h.key)!!
                h.noteRepo.update(current.copy(content = "TYPED AFTER THE PUSH"), h.key)
            }
            SyncHarness.emptyPull(
                h.remoteNote(noteId, "PUSHED", pushed.contentHash!!, pushed.parentHash, listOf(pushed.contentHash!!, "hash-base"))
            )
        }

        h.service().fullSync()

        assertEquals("TYPED AFTER THE PUSH", h.noteRepo.get(noteId, h.key)?.content)
        assertTrue(h.noteRepo.getRaw(noteId)!!.needsSync)
    }

    @Test fun `an attachment added here survives a remote edit pulled before the push`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        h.noteRepo.addAttachment(noteId, ref(id(4)), h.key)
        val remote = h.remoteNote(noteId, "REMOTE", "hash-r", "hash-base", listOf("hash-r", "hash-base"))
        h.onPull = { SyncHarness.emptyPull(remote) }

        val conflicts = mutableListOf<ConflictInfo>()
        val service = h.service()
        service.onConflictDetected = { conflicts += it }
        service.pull()

        assertEquals("it's a conflict, not a silent loss", 1, conflicts.size)
        assertTrue(h.noteRepo.getRaw(noteId)!!.attachments.contains(id(4)))
    }
}
