package com.jottery.android.service

import androidx.test.core.app.ApplicationProvider
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.SyncRepository
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.PutWrappedKeyRequest
import com.jottery.android.model.SyncMetadata
import com.jottery.android.model.WrappedKeyResponse
import com.jottery.android.network.SyncClient
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.UUID

/**
 * D1 with its wave 2 amendment: several devices, each with a real database, sharing
 * one server's blob. Self-healing must not be able to undo a password change.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class AccountKeyServiceTest {
    private val userId = UUID.randomUUID().toString()
    private val oldPassword = "old-${UUID.randomUUID()}"
    private val newPassword = "new-${UUID.randomUUID()}"
    private val accountPassword = "account-${UUID.randomUUID()}"
    private val masterKey = CryptoService.generateSalt()

    /** The server's one copy, and what changes with every write to it. */
    private var blob: String? = null
    private var updatedAt = 0
    private var putStatus = 200
    private var puts = 0
    private var refusals = 0
    private val ifMatches = mutableListOf<String?>()
    private val replaces = mutableListOf<Boolean?>()

    /** An older server sends no updatedAt and writes whatever it's given. */
    private var conditional = true

    /** Runs as a PUT arrives, to put another device's write ahead of it. */
    private var beforePut: (() -> Unit)? = null

    private fun store(value: String) {
        blob = value
        updatedAt++
    }

    private val server = MockWebServer().apply {
        dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse = when (request.method) {
                "PUT" -> put(request)
                else -> blob?.let {
                    MockResponse().setBody(
                        Json.encodeToString(
                            WrappedKeyResponse(it, 1, 1_000_000, updatedAt.toString().takeIf { conditional })
                        )
                    )
                } ?: MockResponse().setResponseCode(404).setBody("{}")
            }
        }
        start()
    }

    private fun put(request: RecordedRequest): MockResponse {
        beforePut?.invoke()
        beforePut = null
        if (putStatus != 200) return MockResponse().setResponseCode(putStatus).setBody("{}")

        val body = Json.decodeFromString<PutWrappedKeyRequest>(request.body.readUtf8())
        val ifMatch = request.getHeader("If-Match")
        ifMatches += ifMatch
        replaces += body.replace

        val refused = conditional && when {
            ifMatch != null -> blob == null || ifMatch != updatedAt.toString()
            body.replace == false -> blob != null
            else -> false
        }
        if (refused) {
            refusals++
            return MockResponse().setResponseCode(409).setBody("{}")
        }
        puts++
        store(body.blob)
        return MockResponse().setBody("{}")
    }

    private inner class Device(opened: Boolean, lastSyncAt: String? = "2026-01-01T00:00:00.000Z") {
        val db: JotteryDatabase = JotteryDatabase.buildInMemory(ApplicationProvider.getApplicationContext())
        val syncRepo = SyncRepository(db.syncMetadataDao(), db.noteSyncMetadataDao(), db.noteDeletionDao())
        val encryptionRepo = EncryptionRepository(db.encryptionMetadataDao())
        val client = SyncClient(server.url("/").toString().trimEnd('/'), "key", "client")
        val service = AccountKeyService(client, syncRepo, encryptionRepo)

        init {
            runBlocking {
                syncRepo.saveSyncMetadata(
                    SyncMetadata(userId = userId, lastSyncAt = lastSyncAt, syncEnabled = true, wrappedKeyOpened = opened)
                )
                encryptionRepo.store(
                    EncryptionMetadata(createdAt = "t", verification = "v", envelopeVersion = 1)
                )
            }
        }

        suspend fun unlock(password: String, canVouch: Boolean = true) =
            service.settle(password, masterKey) { canVouch }

        /** What AppState.changePassword does to the account's copy. */
        suspend fun changePassword(password: String) {
            syncRepo.setPendingWrappedKey(EnvelopeService.wrapForAccount(userId, password, masterKey))
            service.uploadPending()
        }

        suspend fun flag() = syncRepo.getSyncMetadata()!!.wrappedKeyOpened
    }

    private val devices = mutableListOf<Device>()
    private fun device(opened: Boolean = true) = Device(opened).also { devices += it }

    @After fun tearDown() {
        devices.forEach { it.db.close() }
        server.shutdown()
    }

    private fun wrapped(password: String, key: ByteArray = masterKey) =
        CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(key, CryptoService.deriveWrappingKey(password, userId))
        )

    private fun blobOpensWith(password: String): Boolean = runCatching {
        CryptoService.unwrapMasterKey(
            CryptoService.parseEncryptedJSON(blob!!),
            CryptoService.deriveWrappingKey(password, userId),
        )
    }.getOrNull()?.contentEquals(masterKey) == true

    @Test fun `a password change on one device isn't undone by another that still has the old one`() = runBlocking {
        store(wrapped(oldPassword))
        val a = device()
        val b = device()
        assertEquals(AccountKeyService.Outcome.InStep, a.unlock(oldPassword))
        assertEquals(AccountKeyService.Outcome.InStep, b.unlock(oldPassword))

        a.changePassword(newPassword)
        assertTrue(blobOpensWith(newPassword))
        val putsAfterChange = puts

        // Without the flag this is the ping-pong: B can't open it and re-wraps it
        assertEquals(AccountKeyService.Outcome.PasswordChangedElsewhere, b.unlock(oldPassword))
        assertEquals(AccountKeyService.Outcome.PasswordChangedElsewhere, b.unlock(oldPassword))

        assertEquals(putsAfterChange, puts)
        assertTrue("the new password still opens the account's key", blobOpensWith(newPassword))
        assertFalse(blobOpensWith(oldPassword))
    }

    @Test fun `a device that has only ever seen the blob open is covered by the flag`() = runBlocking {
        store(wrapped(oldPassword))
        val b = device(opened = false)

        b.unlock(oldPassword)

        assertTrue("opening the blob sets the flag", b.flag())
        store(wrapped(newPassword))
        assertEquals(AccountKeyService.Outcome.PasswordChangedElsewhere, b.unlock(oldPassword))
        assertEquals(0, puts)
    }

    @Test fun `an old-format blob is healed by a device that never opened it, once`() = runBlocking {
        store(wrapped(accountPassword))
        val device = device(opened = false)

        assertEquals(AccountKeyService.Outcome.InStep, device.unlock(oldPassword))

        assertEquals(1, puts)
        assertTrue(blobOpensWith(oldPassword))
        assertTrue(device.flag())

        // Having healed once, a later failure means a change elsewhere
        store(wrapped(newPassword))
        assertEquals(AccountKeyService.Outcome.PasswordChangedElsewhere, device.unlock(oldPassword))
        assertEquals(1, puts)
    }

    @Test fun `a device that can't vouch for its key leaves an old-format blob alone`() = runBlocking {
        store(wrapped(accountPassword))
        val device = device(opened = false)

        assertEquals(AccountKeyService.Outcome.CannotHeal, device.unlock(oldPassword, canVouch = false))

        assertEquals(0, puts)
        assertFalse(device.flag())
    }

    @Test fun `a blob that opens to another key is never overwritten`() = runBlocking {
        val other = wrapped(oldPassword, CryptoService.generateSalt())
        store(other)

        assertEquals(AccountKeyService.Outcome.Diverged, device(opened = false).unlock(oldPassword))

        assertEquals(other, blob)
        assertEquals(0, puts)
    }

    @Test fun `publishing to an empty account sets the flag`() = runBlocking {
        val device = device(opened = false)

        device.unlock(oldPassword)

        assertTrue(blobOpensWith(oldPassword))
        assertTrue(device.flag())
    }

    @Test fun `a password change the server missed is retried, and isn't mistaken for one made elsewhere`() = runBlocking {
        store(wrapped(oldPassword))
        val device = device()

        putStatus = 503
        device.changePassword(newPassword)

        assertNotNull("the rewrap is kept for later", device.syncRepo.getSyncMetadata()!!.pendingWrappedKey)
        assertTrue(blobOpensWith(oldPassword))

        // Still down at the next unlock: nothing is lost and nothing is claimed
        val whileDown = device.unlock(newPassword)
        assertEquals(AccountKeyService.Outcome.InStep, whileDown)
        assertNotNull(device.syncRepo.getSyncMetadata()!!.pendingWrappedKey)

        putStatus = 200
        assertEquals(AccountKeyService.Outcome.InStep, device.unlock(newPassword))

        assertNull(device.syncRepo.getSyncMetadata()!!.pendingWrappedKey)
        assertTrue(blobOpensWith(newPassword))
    }

    @Test fun `the retry that runs before a sync sends a waiting rewrap`() = runBlocking {
        store(wrapped(oldPassword))
        val device = device()
        putStatus = 503
        device.changePassword(newPassword)
        putStatus = 200

        assertTrue(device.service.uploadPending())

        assertTrue(blobOpensWith(newPassword))
        assertNull(device.syncRepo.getSyncMetadata()!!.pendingWrappedKey)
    }

    // MARK: - Conditional writes

    @Test fun `each purpose says what it expects to find`() = runBlocking {
        val first = device(opened = false)
        first.unlock(oldPassword)
        assertEquals("a first upload", false, replaces.last())
        assertNull(ifMatches.last())

        first.changePassword(newPassword)
        assertEquals("a password change", true, replaces.last())
        assertNull(ifMatches.last())

        store(wrapped(accountPassword))
        val tag = updatedAt.toString()
        device(opened = false).unlock(oldPassword)
        assertEquals("a repair names what it read", tag, ifMatches.last())
    }

    @Test fun `a password change between a repair's read and its write isn't overwritten`() = runBlocking {
        // The race the flag can't close on its own: this device has never opened the
        // blob, so it may heal, and then another device changes the password
        store(wrapped(accountPassword))
        val device = device(opened = false)
        beforePut = { store(wrapped(newPassword)) }

        val outcome = device.unlock(oldPassword)

        assertEquals(1, refusals)
        assertEquals(0, puts)
        assertTrue("the other device's password still opens it", blobOpensWith(newPassword))
        assertEquals(AccountKeyService.Outcome.PasswordChangedElsewhere, outcome)
    }

    @Test fun `a refused repair isn't tried again at the next unlock either`() = runBlocking {
        store(wrapped(accountPassword))
        val device = device(opened = false)
        beforePut = { store(wrapped(newPassword)) }
        device.unlock(oldPassword)

        assertEquals(AccountKeyService.Outcome.PasswordChangedElsewhere, device.unlock(oldPassword))

        assertEquals(0, puts)
        assertTrue(blobOpensWith(newPassword))
    }

    @Test fun `losing the race for a first upload adopts nothing and overwrites nothing`() = runBlocking {
        val device = device(opened = false)
        val theirs = wrapped(oldPassword, CryptoService.generateSalt())
        beforePut = { store(theirs) }

        val outcome = device.unlock(oldPassword)

        assertEquals(1, refusals)
        assertEquals(theirs, blob)
        assertEquals(AccountKeyService.Outcome.Diverged, outcome)
    }

    @Test fun `a first upload that loses to the same key is simply in step`() = runBlocking {
        val device = device(opened = false)
        beforePut = { store(wrapped(oldPassword)) }

        assertEquals(AccountKeyService.Outcome.InStep, device.unlock(oldPassword))

        assertEquals(0, puts)
        assertTrue(device.flag())
    }

    @Test fun `a password change writes whatever is there`() = runBlocking {
        store(wrapped(oldPassword))
        val device = device()
        beforePut = { store(wrapped("elsewhere-${UUID.randomUUID()}")) }

        device.changePassword(newPassword)

        assertTrue(blobOpensWith(newPassword))
        assertNull(device.syncRepo.getSyncMetadata()!!.pendingWrappedKey)
    }

    @Test fun `an older server gets no If-Match, and a repair still heals`() = runBlocking {
        conditional = false
        store(wrapped(accountPassword))
        val device = device(opened = false)

        assertEquals(AccountKeyService.Outcome.InStep, device.unlock(oldPassword))

        assertNull(ifMatches.single())
        assertEquals(true, replaces.single())
        assertTrue(blobOpensWith(oldPassword))
    }

    @Test fun `a server's updatedAt can't break out of the header`() = runBlocking {
        val response = WrappedKeyResponse(wrapped(accountPassword), 1, 1_000_000, "x\r\nEvil: yes")
        val hostile = MockWebServer().apply {
            dispatcher = object : Dispatcher() {
                override fun dispatch(request: RecordedRequest): MockResponse = when (request.method) {
                    "PUT" -> {
                        ifMatches += request.getHeader("If-Match")
                        MockResponse().setBody("{}")
                    }
                    else -> MockResponse().setBody(Json.encodeToString(response))
                }
            }
            start()
        }
        val device = device(opened = false)
        val client = SyncClient(hostile.url("/").toString().trimEnd('/'), "key", "client")

        AccountKeyService(client, device.syncRepo, device.encryptionRepo).settle(oldPassword, masterKey) { true }

        assertNull(ifMatches.single())
        hostile.shutdown()
    }

    @Test fun `an account with no user id changes nothing`() = runBlocking {
        val device = device()
        device.syncRepo.saveSyncMetadata(SyncMetadata())

        assertEquals(AccountKeyService.Outcome.InStep, device.unlock(oldPassword))
        assertEquals(0, puts)
        assertNull(blob)
    }
}
