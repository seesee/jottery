package com.jottery.android.service

import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.AttachmentWarning
import com.jottery.android.model.SyncAccepted
import com.jottery.android.model.SyncAttachment
import com.jottery.android.model.SyncPullRequest
import com.jottery.android.model.SyncPushRequest
import com.jottery.android.model.SyncPushResponse
import com.jottery.android.model.SyncSavedSearch
import com.jottery.android.network.SyncClientException
import com.jottery.android.testing.SyncHarness
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.UUID

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class SyncPushTest {
    private lateinit var h: SyncHarness
    private val parser = Json { ignoreUnknownKeys = true }

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    private fun id() = UUID.randomUUID().toString()

    private suspend fun queuedNote(text: String, attachments: List<AttachmentRef> = emptyList()): String {
        val id = id()
        val record = h.seedSynced(id, text, "hash-$id").copy(
            needsSync = true,
            attachments = Json.encodeToString(attachments),
        )
        h.noteRepo.insertOrReplace(record)
        return id
    }

    private suspend fun blob(size: Int): AttachmentRef {
        val id = id()
        h.attachmentRepo.insertOrReplace(AttachmentRecord(id, "", "image/png", size, ByteArray(size) { 7 }))
        return AttachmentRef(id, h.encrypt("name"), "image/png", size, id)
    }

    private fun acceptAll(): (String) -> SyncPushResponse = { body ->
        val request = parser.decodeFromString<SyncPushRequest>(body)
        SyncPushResponse(
            accepted = request.notes.map { SyncAccepted(it.id, 5, "2026-02-01T00:00:00.000Z") },
            rejected = emptyList(),
        )
    }

    private fun requests() = h.pushBodies.map { parser.decodeFromString<SyncPushRequest>(it) }

    @Test fun `a large vault goes in several requests, each within the limit`() = runBlocking {
        repeat(4) { queuedNote("x".repeat(1_500_000)) }
        h.onPush = acceptAll()

        h.service().push()

        assertTrue("expected several chunks, got ${h.pushBodies.size}", h.pushBodies.size > 1)
        assertEquals(4, requests().sumOf { it.notes.size })
        assertTrue(h.pushBodies.all { it.length < 5 * 1024 * 1024 })
        assertTrue(h.noteRepo.listNeedingSync().isEmpty())
    }

    @Test fun `many small notes are split by count`() = runBlocking {
        repeat(SyncService.PUSH_CHUNK_NOTES + 1) { queuedNote("n") }
        h.onPush = acceptAll()

        h.service().push()

        assertEquals(2, requests().count { it.notes.isNotEmpty() })
    }

    @Test fun `a push failure doesn't stop the pull`() = runBlocking {
        queuedNote("local")
        h.server.dispatcher = failingPushes(500)
        val remote = h.remoteNote(id(), "from elsewhere", "h", null, listOf("h"))
        h.onPull = { SyncHarness.emptyPull(remote) }

        val thrown = runCatching { h.service().fullSync() }.exceptionOrNull()

        assertTrue(thrown is SyncClientException)
        assertEquals("from elsewhere", h.noteRepo.get(remote.id, h.key)!!.content)
    }

    @Test fun `a note too large for the server doesn't hold up the others`() = runBlocking {
        val big = queuedNote("x".repeat(4_000_000))
        val small = queuedNote("small")
        h.server.dispatcher = refuseBodiesOver(3_000_000)
        h.onPush = acceptAll()

        val thrown = runCatching { h.service().push() }.exceptionOrNull()

        assertEquals(413, (thrown as SyncClientException).statusCode)
        assertTrue("still queued", h.noteRepo.getRaw(big)!!.needsSync)
        assertFalse(h.noteRepo.getRaw(small)!!.needsSync)
    }

    @Test fun `a blob follows its note, once`() = runBlocking {
        val ref = blob(100)
        val id = queuedNote("with file", listOf(ref))
        h.onPush = acceptAll()

        h.service().push()

        val first = requests()
        assertTrue("the note's request carries no blob", first[0].attachments.isEmpty())
        assertEquals(listOf(ref.id), first.flatMap { it.attachments }.map { it.id })

        // Another edit must not send it again
        h.noteRepo.insertOrReplace(h.noteRepo.getRaw(id)!!.copy(needsSync = true))
        h.pushBodies.clear()
        h.service().push()

        assertTrue(requests().flatMap { it.attachments }.isEmpty())
    }

    @Test fun `a blob the server reports missing is sent again`() = runBlocking {
        val ref = blob(100)
        h.attachmentRepo.markUploaded(ref.id)
        val id = queuedNote("with file", listOf(ref))
        h.onPush = { body ->
            acceptAll()(body).copy(attachmentWarnings = listOf(AttachmentWarning(id, listOf(ref.id))))
        }

        h.service().push()

        assertEquals(listOf(ref.id), requests().flatMap { it.attachments }.map { it.id }.distinct())
    }

    @Test fun `a saved search is pushed and marked synced`() = runBlocking {
        val search = h.savedSearchRepo.create("Mine", "#mine", h.key)
        h.onPush = { SyncPushResponse(emptyList(), emptyList()) }

        h.service().push()

        assertEquals(listOf(search.id), requests().flatMap { it.savedSearches }.map { it.id })
        assertTrue(h.savedSearchRepo.listNeedingSync().isEmpty())
    }

    @Test fun `a pulled saved search is stored`() = runBlocking {
        val remote = SyncSavedSearch(
            id = id(), name = h.encrypt("Remote"), query = h.encrypt("#r"), order = 2,
            createdAt = "2026-01-01T00:00:00.000Z", modifiedAt = "2026-01-02T00:00:00.000Z",
            deleted = false, version = 3,
        )
        h.onPull = { SyncHarness.emptyPull().copy(savedSearches = listOf(remote)) }

        h.service().pull()

        assertEquals(listOf("Remote"), h.savedSearchRepo.listActive(h.key).map { it.name })
    }

    @Test fun `the watermark is the server's time from the first page, set only after the last`() = runBlocking {
        var page = 0
        h.onPull = {
            page++
            if (page == 1) {
                SyncHarness.emptyPull(h.remoteNote(id(), "a", "h1", null, listOf("h1")))
                    .copy(syncedAt = "2026-05-01T00:00:00.000Z", hasMore = true)
            } else {
                SyncHarness.emptyPull(h.remoteNote(id(), "b", "h2", null, listOf("h2")))
                    .copy(syncedAt = "2026-05-02T00:00:00.000Z", hasMore = false)
            }
        }

        h.service().pull()

        assertEquals("2026-05-01T00:00:00.000Z", h.syncRepo.getSyncMetadata()!!.lastSyncAt)
    }

    @Test fun `a pull that fails partway leaves the watermark alone`() = runBlocking {
        h.syncRepo.updateLastSyncAt("2026-01-01T00:00:00.000Z")
        var page = 0
        h.onPull = {
            page++
            if (page == 1) {
                SyncHarness.emptyPull(h.remoteNote(id(), "a", "h1", null, listOf("h1"))).copy(hasMore = true)
            } else {
                throw IllegalStateException("connection dropped")
            }
        }

        runCatching { h.service().pull() }

        assertEquals("2026-01-01T00:00:00.000Z", h.syncRepo.getSyncMetadata()!!.lastSyncAt)
    }

    @Test fun `a full pull ignores the watermark`() = runBlocking {
        h.syncRepo.updateLastSyncAt("2026-01-01T00:00:00.000Z")

        h.service().fullSync(fullPull = true)

        assertNull(parser.decodeFromString<SyncPullRequest>(h.pullBodies.single()).lastSyncAt)
    }

    @Test fun `known blobs are the ones held, not the ones referenced`() = runBlocking {
        val held = blob(10)
        val missing = AttachmentRef(id(), h.encrypt("n"), "image/png", 1, id())
        queuedNote("n", listOf(held, missing))

        h.service().pull()

        val known = parser.decodeFromString<SyncPullRequest>(h.pullBodies.single()).knownAttachmentIds
        assertEquals(listOf(held.data), known)
    }

    @Test fun `what fails validation is dropped and the rest still lands`() = runBlocking {
        val good = h.remoteNote(id(), "good", "h1", null, listOf("h1"))
        val badId = h.remoteNote("../../etc/passwd", "bad", "h2", null, listOf("h2"))
        val badDate = h.remoteNote(id(), "bad", "h3", null, listOf("h3"), modifiedAt = "yesterday")
        val plaintext = h.remoteNote(id(), "x", "h4", null, listOf("h4")).copy(content = "not encrypted at all")
        val badRef = h.remoteNote(id(), "x", "h5", null, listOf("h5")).copy(
            attachments = listOf(AttachmentRef("nope", "f", "../evil", -1, "nope"))
        )
        h.onPull = {
            SyncHarness.emptyPull(good, badId, badDate, plaintext, badRef).copy(
                attachments = listOf(SyncAttachment("not-a-uuid", "AAAA"), SyncAttachment(id(), "!!!not base64")),
            )
        }

        h.service().pull()

        assertEquals(listOf(good.id), h.noteRepo.listAllRaw().map { it.id })
        assertTrue(h.attachmentRepo.listBlobIds().isEmpty())
    }

    @Test fun `a note that won't decrypt under this key is quarantined`() = runBlocking {
        val foreign = javax.crypto.spec.SecretKeySpec(ByteArray(32) { 9 }, "AES")
        val content = com.jottery.android.crypto.CryptoService.serializeEncryptedJSON(
            com.jottery.android.crypto.CryptoService.encryptText("foreign", foreign)
        )
        val remote = h.remoteNote(id(), "x", "h", null, listOf("h")).copy(content = content)
        h.onPull = { SyncHarness.emptyPull(remote) }

        h.service().pull()

        assertTrue(h.noteRepo.listAllRaw().isEmpty())
    }

    @Test fun `quarantined notes are reported`() = runBlocking {
        val foreign = javax.crypto.spec.SecretKeySpec(ByteArray(32) { 9 }, "AES")
        val content = com.jottery.android.crypto.CryptoService.serializeEncryptedJSON(
            com.jottery.android.crypto.CryptoService.encryptText("foreign", foreign)
        )
        val unreadable = h.remoteNote(id(), "x", "h1", null, listOf("h1")).copy(content = content)
        val malformed = h.remoteNote("not-a-uuid", "x", "h2", null, listOf("h2"))
        h.onPull = { SyncHarness.emptyPull(unreadable, malformed) }
        var reported = 0
        val service = h.service()
        service.onNotesQuarantined = { reported = it }

        service.pull()

        assertEquals(2, reported)
    }

    @Test fun `a mime type with parameters is accepted`() {
        val ref = AttachmentRef(id(), "x", "text/plain; charset=utf-8", 1, id())
        val note = h.remoteNote(id(), "x", "h", null, listOf("h")).copy(attachments = listOf(ref.copy(data = ref.id)))

        assertTrue(SyncValidator.isValidNote(note))
    }

    @Test fun `tags in every historic shape survive a pull`() = runBlocking {
        val asString = h.encryptTag("one")
        val asArray = h.encrypt("[\"two\",\"three\"]")
        val asPlain = h.encrypt("four")
        val remote = h.remoteNote(id(), "x", "h", null, listOf("h")).copy(tags = listOf(asString, asArray, asPlain))
        h.onPull = { SyncHarness.emptyPull(remote) }

        h.service().pull()

        assertEquals(listOf("one", "two", "three", "four"), h.noteRepo.get(remote.id, h.key)!!.tags)
    }

    private fun failingPushes(status: Int) = object : okhttp3.mockwebserver.Dispatcher() {
        val original = h.server.dispatcher
        override fun dispatch(request: okhttp3.mockwebserver.RecordedRequest) =
            if (request.path == "/api/v1/sync/push") {
                okhttp3.mockwebserver.MockResponse().setResponseCode(status).setBody("{}")
            } else {
                original.dispatch(request)
            }
    }

    private fun refuseBodiesOver(limit: Long) = object : okhttp3.mockwebserver.Dispatcher() {
        val original = h.server.dispatcher
        override fun dispatch(request: okhttp3.mockwebserver.RecordedRequest) =
            if (request.path == "/api/v1/sync/push" && request.bodySize > limit) {
                okhttp3.mockwebserver.MockResponse().setResponseCode(413).setBody("{}")
            } else {
                original.dispatch(request)
            }
    }
}
