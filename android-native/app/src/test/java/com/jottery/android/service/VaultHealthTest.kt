package com.jottery.android.service

import com.jottery.android.crypto.CryptoService
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.testing.SyncHarness
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
import java.util.Base64
import java.util.UUID
import javax.crypto.spec.SecretKeySpec

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class VaultHealthTest {
    private lateinit var h: SyncHarness
    private val otherKey = SecretKeySpec(ByteArray(32) { 9 }, "AES")

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    private suspend fun scan() = VaultHealthService(h.noteRepo, h.attachmentRepo).scan(h.key)

    private fun ref(id: String) =
        AttachmentRef(id = id, filename = h.encrypt("file.txt"), mimeType = "text/plain", size = 3, data = id)

    private suspend fun noteWithoutItsBlob(): Pair<String, String> {
        val note = h.noteRepo.create("Has a file", emptyList(), h.key)
        val id = UUID.randomUUID().toString()
        h.noteRepo.addAttachment(note.id, ref(id), h.key)
        return note.id to id
    }

    @Test fun `a sound vault is healthy`() = runBlocking {
        val note = h.noteRepo.create("Fine", emptyList(), h.key)
        val id = UUID.randomUUID().toString()
        h.attachmentRepo.insertOrReplace(AttachmentRecord(id, "", "text/plain", 3, byteArrayOf(1, 2, 3)))
        h.noteRepo.addAttachment(note.id, ref(id), h.key)

        assertTrue(scan().isHealthy)
    }

    @Test fun `a note this key can't open is reported, and nothing is changed`() = runBlocking {
        val note = h.noteRepo.create("Under another key", emptyList(), otherKey)

        val report = scan()

        assertEquals(listOf(note.id), report.undecryptable.map { it.id })
        assertNotNull("a scan only looks", h.noteRepo.getRaw(note.id))
    }

    @Test fun `an attachment with no blob is reported with its note's title`() = runBlocking {
        val (noteId, blobId) = noteWithoutItsBlob()

        val missing = scan().missing.single()

        assertEquals(noteId, missing.noteId)
        assertEquals(blobId, missing.blobId)
        assertEquals("Has a file", missing.noteTitle)
    }

    @Test fun `the bin isn't anyone's concern`() = runBlocking {
        val (noteId, _) = noteWithoutItsBlob()
        h.noteRepo.softDelete(noteId)

        assertTrue(scan().isHealthy)
    }

    @Test fun `an unreadable note's missing attachment has no title to show`() = runBlocking {
        val note = h.noteRepo.create("Under another key", emptyList(), otherKey)
        val id = UUID.randomUUID().toString()
        h.noteRepo.addAttachment(note.id, ref(id), otherKey)

        assertNull(scan().missing.single().noteTitle)
    }

    // Repair

    @Test fun `a blob the server has comes back, and clears the report`() = runBlocking {
        val (_, blobId) = noteWithoutItsBlob()
        h.serverBlobs[blobId] = Base64.getEncoder().encodeToString("bytes".toByteArray())

        assertEquals(BlobRepair.REPAIRED, h.service().repairBlob(blobId))

        assertTrue(scan().missing.isEmpty())
        assertEquals("bytes", String(h.attachmentRepo.getBlob(blobId)!!))
    }

    @Test fun `a blob the server doesn't have says so`() = runBlocking {
        val (_, blobId) = noteWithoutItsBlob()

        assertEquals(BlobRepair.NOT_ON_SERVER, h.service().repairBlob(blobId))
        assertFalse(scan().missing.isEmpty())
    }

    @Test fun `a server that sends another blob isn't believed`() = runBlocking {
        assertEquals(BlobRepair.NOT_ON_SERVER, h.service().repairBlob("not-a-uuid"))
        assertTrue(h.blobRequests.isEmpty())
    }

    @Test fun `an unreachable server is a failure, not a loss`() = runBlocking {
        val (_, blobId) = noteWithoutItsBlob()
        h.server.shutdown()

        assertEquals(BlobRepair.FAILED, h.service().repairBlob(blobId))
    }
}
