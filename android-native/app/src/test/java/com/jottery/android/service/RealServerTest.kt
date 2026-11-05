package com.jottery.android.service

import androidx.room.withTransaction
import com.jottery.android.crypto.CryptoService
import com.jottery.android.model.AccountCredentials
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.PutWrappedKeyRequest
import com.jottery.android.model.RegisterDeviceRequest
import com.jottery.android.model.SyncPullRequest
import com.jottery.android.model.SyncPushRequest
import com.jottery.android.network.SyncClient
import com.jottery.android.network.SyncClientException
import com.jottery.android.testing.SyncHarness
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.time.Instant
import java.time.temporal.ChronoUnit
import java.util.UUID

/**
 * The real sync service against a real server, as two devices. Every other sync
 * test uses a stub written from this client's own reading of the protocol, so only
 * this can catch the two disagreeing.
 *
 * Skipped unless JOTTERY_TEST_SERVER, JOTTERY_TEST_EMAIL and JOTTERY_TEST_PASSWORD
 * name an approved account on a throwaway server. It writes to that account.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class RealServerTest {
    private val endpoint = System.getenv("JOTTERY_TEST_SERVER").orEmpty()
    private val email = System.getenv("JOTTERY_TEST_EMAIL").orEmpty()
    private val password = System.getenv("JOTTERY_TEST_PASSWORD").orEmpty()

    private lateinit var laptop: SyncHarness
    private lateinit var phone: SyncHarness

    private companion object {
        /**
         * Registration is limited to a burst of 20 calls from one address, then one
         * every three seconds. A device per test goes over that, and the 429 in setUp
         * stops the test before it has tried anything, so each is registered only once.
         */
        val registered = mutableMapOf<String, SyncHarness.RealServer>()
    }

    private fun register(name: String): SyncHarness.RealServer = runBlocking {
        val response = SyncClient(endpoint, "", "").registerDevice(RegisterDeviceRequest(email, password, name))
        SyncHarness.RealServer(endpoint, response.apiKey, response.clientId)
    }

    /** A device with an empty database. Its registration lasts the whole run: see [registered]. */
    private fun device(name: String) = SyncHarness(registered.getOrPut(name) { register(name) })

    @Before fun setUp() {
        assumeTrue("needs a real server", endpoint.isNotEmpty())
        laptop = device("laptop")
        phone = device("phone")
    }

    @After fun tearDown() {
        if (::laptop.isInitialized) laptop.close()
        if (::phone.isInitialized) phone.close()
    }

    private fun purger(h: SyncHarness) = NotePurger(h.noteRepo, h.versionRepo, h.attachmentRepo, h.syncRepo) { block ->
        h.db.withTransaction { block() }
    }

    private fun blob(h: SyncHarness): ByteArray = CryptoService.serializeEncryptedJSON(
        CryptoService.encrypt("attachment bytes".toByteArray(), h.key)
    ).toByteArray()

    private fun ref(h: SyncHarness, id: String) =
        AttachmentRef(id = id, filename = h.encrypt("file.txt"), mimeType = "text/plain", size = 16, data = id)

    @Test fun `a note, its edit, its tags and its version reach the other device`() = runBlocking {
        val note = laptop.noteRepo.create("From the laptop", listOf("one", "two"), laptop.key)
        laptop.service().fullSync()
        phone.service().fullSync()

        val pulled = phone.noteRepo.get(note.id, phone.key)!!
        assertEquals("From the laptop", pulled.content)
        assertEquals(listOf("one", "two"), pulled.tags)
        assertFalse(phone.noteRepo.getRaw(note.id)!!.needsSync)

        phone.noteRepo.update(pulled.copy(content = "Edited on the phone"), phone.key)
        phone.service().fullSync()
        laptop.service().fullSync()

        assertEquals("Edited on the phone", laptop.noteRepo.get(note.id, laptop.key)?.content)
        assertTrue("history came too", laptop.versionRepo.getVersions(note.id).isNotEmpty())
    }

    @Test fun `this server speaks the cursor, and it saves work`() = runBlocking {
        laptop.noteRepo.create("Cursor note", emptyList(), laptop.key)
        laptop.service().fullSync()

        phone.service().pull()
        val cursor = phone.syncRepo.getSyncMetadata()!!.pullCursor
        assertNotNull("the server offered no cursor", cursor)
        assertNotNull(phone.syncRepo.getSyncMetadata()!!.lastSyncAt)

        val page = phone.client.pull(
            SyncPullRequest(cursor = cursor, knownNoteIds = emptyList(), knownAttachmentIds = emptyList(), limit = 100, offset = 0)
        )
        assertTrue("nothing has changed since", page.notes.isEmpty())
        assertEquals(cursor, page.nextCursor)
    }

    @Test fun `an edit between devices conflicts, and is resolved`() = runBlocking {
        val note = laptop.noteRepo.create("Base", emptyList(), laptop.key)
        laptop.service().fullSync()
        phone.service().fullSync()

        val onLaptop = laptop.noteRepo.get(note.id, laptop.key)!!
        laptop.noteRepo.update(onLaptop.copy(content = "Laptop wins the race"), laptop.key)
        laptop.service().fullSync()

        val onPhone = phone.noteRepo.get(note.id, phone.key)!!
        phone.noteRepo.update(onPhone.copy(content = "Phone was offline"), phone.key)
        val service = phone.service()
        val conflicts = mutableListOf<com.jottery.android.model.ConflictInfo>()
        service.onConflictDetected = { conflicts += it }
        service.fullSync()

        assertEquals("Phone was offline", phone.noteRepo.get(note.id, phone.key)?.content)
        assertEquals("Laptop wins the race", conflicts.last().serverContent)

        service.resolveConflict(conflicts.last(), com.jottery.android.model.ConflictResolutionStrategy.KEEP_LOCAL)
        service.fullSync()
        laptop.service().fullSync()

        assertEquals("Phone was offline", laptop.noteRepo.get(note.id, laptop.key)?.content)
    }

    @Test fun `any number of saves between syncs is accepted`() = runBlocking {
        val note = laptop.noteRepo.create("Base", emptyList(), laptop.key)
        laptop.service().fullSync()

        repeat(120) { n ->
            val current = laptop.noteRepo.get(note.id, laptop.key)!!
            laptop.noteRepo.update(current.copy(content = "Save $n"), laptop.key)
        }
        val conflicts = mutableListOf<com.jottery.android.model.ConflictInfo>()
        val service = laptop.service()
        service.onConflictDetected = { conflicts += it }
        service.fullSync()

        assertTrue("rejected on a note nothing else touched", conflicts.isEmpty())
        assertFalse(laptop.noteRepo.getRaw(note.id)!!.needsSync)
    }

    @Test fun `a blob follows its note, and removing it reaches the other device`() = runBlocking {
        val note = laptop.noteRepo.create("With a file", emptyList(), laptop.key)
        val id = UUID.randomUUID().toString()
        val bytes = blob(laptop)
        laptop.attachmentRepo.insertOrReplace(AttachmentRecord(id, "", "text/plain", bytes.size, bytes))
        laptop.noteRepo.addAttachment(note.id, ref(laptop, id), laptop.key)
        laptop.service().fullSync()

        phone.service().fullSync()
        assertEquals(id, phone.noteRepo.get(note.id, phone.key)!!.attachments.single().id)
        assertNotNull("the blob came with it", phone.attachmentRepo.getBlob(id))

        laptop.noteRepo.removeAttachment(note.id, id, laptop.key)
        laptop.service().fullSync()
        phone.service().fullSync()

        assertTrue(phone.noteRepo.get(note.id, phone.key)!!.attachments.isEmpty())
    }

    @Test fun `a permanent delete reaches the other device's bin`() = runBlocking {
        val note = laptop.noteRepo.create("Delete me", emptyList(), laptop.key)
        laptop.service().fullSync()
        phone.service().fullSync()
        assertNotNull(phone.noteRepo.getRaw(note.id))

        purger(laptop).deletePermanently(listOf(note.id))
        laptop.service().fullSync()
        assertTrue("sent once", laptop.syncRepo.listDeletions().isEmpty())

        phone.service().fullSync()

        val row = phone.noteRepo.getRaw(note.id)!!
        assertTrue(row.deleted)
        assertNotNull(row.localDeletedAt)
    }

    @Test fun `a deletion older than an edit is refused, and the note comes back`() = runBlocking {
        val note = laptop.noteRepo.create("Keep me", emptyList(), laptop.key)
        laptop.service().fullSync()
        phone.service().fullSync()

        // Permanently deleted on the phone, and only then edited on the laptop
        purger(phone).deletePermanently(listOf(note.id))
        Thread.sleep(20)
        val current = laptop.noteRepo.get(note.id, laptop.key)!!
        laptop.noteRepo.update(current.copy(content = "Edited since"), laptop.key)
        laptop.service().fullSync()

        phone.service().fullSync()

        assertTrue(phone.syncRepo.listDeletions().isEmpty())
        assertEquals("Edited since", phone.noteRepo.get(note.id, phone.key)?.content)
        assertEquals("Edited since", laptop.noteRepo.get(note.id, laptop.key)?.content)
    }

    @Test fun `an edit older than a deletion is skipped, and the device stops sending it`() = runBlocking {
        val note = laptop.noteRepo.create("Delete me later", emptyList(), laptop.key)
        laptop.service().fullSync()
        phone.service().fullSync()

        // Edited on the laptop, and only then permanently deleted on the phone
        val current = laptop.noteRepo.get(note.id, laptop.key)!!
        laptop.noteRepo.update(current.copy(content = "Too late"), laptop.key)
        Thread.sleep(20)
        purger(phone).deletePermanently(listOf(note.id))
        phone.service().fullSync()

        laptop.service().fullSync()

        val row = laptop.noteRepo.getRaw(note.id)!!
        assertTrue("in the bin, not gone", row.deleted)
        assertFalse("not queued to be sent again", row.needsSync)
    }

    @Test fun `a note dated days ahead is skipped, kept, and the clock is mentioned`() = runBlocking {
        val note = laptop.noteRepo.create("From the future", emptyList(), laptop.key)
        val row = laptop.noteRepo.getRaw(note.id)!!
        laptop.noteRepo.insertOrReplace(row.copy(modifiedAt = Instant.now().plus(3, ChronoUnit.DAYS).toString()))
        var told = false
        val service = laptop.service()
        service.onClockLooksWrong = { told = true }

        service.fullSync()

        assertTrue("the user should hear about it", told)
        assertTrue("it goes when the clock is right", laptop.noteRepo.getRaw(note.id)!!.needsSync)
        phone.service().fullSync()
        assertNull(phone.noteRepo.getRaw(note.id))
    }

    @Test fun `a conflict with a note in the server's bin says so`() = runBlocking {
        val note = laptop.noteRepo.create("Base", emptyList(), laptop.key)
        laptop.service().fullSync()
        phone.service().fullSync()

        // Binning alone doesn't move the note's hash, so it takes an edit as well to diverge
        val onLaptop = laptop.noteRepo.get(note.id, laptop.key)!!
        laptop.noteRepo.update(onLaptop.copy(content = "Laptop edit"), laptop.key)
        laptop.noteRepo.softDelete(note.id)
        laptop.service().fullSync()

        val onPhone = phone.noteRepo.get(note.id, phone.key)!!
        phone.noteRepo.update(onPhone.copy(content = "Phone was offline"), phone.key)
        val service = phone.service()
        val conflicts = mutableListOf<com.jottery.android.model.ConflictInfo>()
        service.onConflictDetected = { conflicts += it }
        service.fullSync()

        val conflict = conflicts.last { it.id == note.id }
        assertTrue(phone.syncRepo.listStoredConflicts().single { it.id == note.id }.serverDeleted)
        service.resolveConflict(conflict, com.jottery.android.model.ConflictResolutionStrategy.KEEP_SERVER)

        assertTrue(phone.noteRepo.getRaw(note.id)!!.deleted)
    }

    @Test fun `saved searches travel both ways`() = runBlocking {
        val search = laptop.savedSearchRepo.create("Work", "#work", laptop.key)
        laptop.service().fullSync()
        phone.service().fullSync()

        assertEquals("#work", phone.savedSearchRepo.listActive(phone.key).single { it.id == search.id }.query)
        assertTrue(laptop.savedSearchRepo.listNeedingSync().isEmpty())

        phone.savedSearchRepo.delete(search.id)
        phone.service().fullSync()
        laptop.service().fullSync()

        assertTrue(laptop.savedSearchRepo.listActive(laptop.key).none { it.id == search.id })
    }

    @Test fun `key writes mean what the protocol says`() = runBlocking {
        val client = laptop.client
        fun request(replace: Boolean? = null) =
            PutWrappedKeyRequest(blob = "blob-${UUID.randomUUID()}", kdfVersion = 1, kdfIterations = 1_000_000, replace = replace)

        suspend fun status(block: suspend () -> Unit): Int? = try {
            block()
            null
        } catch (e: SyncClientException) {
            e.statusCode
        }

        // Force a known state, so the order the tests run in doesn't matter
        client.putWrappedKey(request(replace = true))
        val first = client.getWrappedKey()!!
        assertNotNull("this server has conditional writes", first.updatedAt)

        assertEquals("a first upload over a key", 409, status { client.putWrappedKey(request(replace = false)) })

        assertNull(status { client.putWrappedKey(request(), ifMatch = first.updatedAt) })
        val second = client.getWrappedKey()!!
        assertEquals("a repair of what has since changed", 409, status { client.putWrappedKey(request(), ifMatch = first.updatedAt) })

        assertNull("a password change always lands", status { client.putWrappedKey(request(replace = true)) })
        assertTrue(client.getWrappedKey()!!.updatedAt != second.updatedAt)
    }

    @Test fun `the account calls work with a session`() = runBlocking {
        val anonymous = SyncClient(endpoint, "", "")
        val credentials = AccountCredentials(email, password)

        assertTrue(anonymous.userStatus(credentials).isApproved)

        val session = anonymous.logIn(credentials).sessionId!!
        val info = anonymous.accountInfo(session)
        assertEquals(email, info.email)
        assertTrue(info.storageQuotaMb > 0)

        val token = anonymous.generateInboxToken(session).token
        assertTrue(token.isNotEmpty())
        assertTrue(anonymous.accountInfo(session).inbox.hasToken)
        anonymous.revokeInboxToken(session)
        assertFalse(anonymous.accountInfo(session).inbox.hasToken)
        anonymous.logOut(session)
    }

    @Test fun `a revoked device is told so`() = runBlocking {
        val spare = SyncHarness(register("spare"))
        spare.client.revokeThisDevice()

        val failure = runCatching { spare.service().pull() }.exceptionOrNull()

        assertEquals(401, (failure as SyncClientException).statusCode)
        spare.close()
    }

    @Test fun `the request the server is sent really is what the protocol documents`() {
        val json = Json { encodeDefaults = true }
        val body = json.encodeToString(
            SyncPushRequest(notes = emptyList(), attachments = emptyList(), versions = emptyList(), attachmentsAuthoritative = true)
        )
        assertTrue(body.contains("\"attachmentsAuthoritative\":true"))
    }
}
