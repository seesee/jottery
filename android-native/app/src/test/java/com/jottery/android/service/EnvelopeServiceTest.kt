package com.jottery.android.service

import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.EncryptionRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.database.repository.SavedSearchRepository
import com.jottery.android.database.repository.VersionRepository
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.NoteRecord
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.model.PutWrappedKeyRequest
import com.jottery.android.model.WrappedKeyResponse
import com.jottery.android.network.SyncClient
import com.jottery.android.network.SyncClientException
import androidx.room.withTransaction
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.UUID
import javax.crypto.spec.SecretKeySpec

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class EnvelopeServiceTest {
    private val userId = UUID.randomUUID().toString()
    private val notesPassword = "notes-${UUID.randomUUID()}"
    private val accountPassword = "account-${UUID.randomUUID()}"
    private val masterKey = CryptoService.generateSalt()

    private var getStatus = 404
    private var blob: String? = null
    private val puts = mutableListOf<String>()

    private val server = MockWebServer().apply {
        dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse = when (request.method) {
                "PUT" -> {
                    puts += request.body.readUtf8()
                    MockResponse().setBody("{}")
                }
                else -> blob?.let {
                    MockResponse().setBody(Json.encodeToString(WrappedKeyResponse(it, 1, 1_000_000)))
                } ?: MockResponse().setResponseCode(getStatus).setBody("{}")
            }
        }
        start()
    }
    private val client = SyncClient(server.url("/").toString().trimEnd('/'), "k", "c")

    @After fun tearDown() = server.shutdown()

    private fun wrappedWith(password: String, key: ByteArray = masterKey) =
        CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(key, CryptoService.deriveWrappingKey(password, userId))
        )

    private suspend fun reconcile(legacy: Boolean = false, canVouch: Boolean = true, opened: Boolean = false) =
        EnvelopeService.reconcile(client, userId, notesPassword, masterKey, legacy, opened) { canVouch }

    @Test fun `a blob wrapped with the account password is refused`() {
        // D1: the server receives the account password, so it could wrap a key
        // of its own choosing with it. Such a key must never be adopted.
        blob = wrappedWith(accountPassword)

        assertThrows(EnvelopeException.CannotUnwrap::class.java) {
            runBlocking { EnvelopeService.fetchServerKey(client, userId, notesPassword) }
        }
    }

    @Test fun `a blob wrapped with the notes password opens`() = runBlocking {
        blob = wrappedWith(notesPassword)

        assertArrayEquals(masterKey, EnvelopeService.fetchServerKey(client, userId, notesPassword))
    }

    @Test fun `a 404 means no key, and only a 404`() = runBlocking {
        getStatus = 404
        assertNull(EnvelopeService.fetchServerKey(client, userId, notesPassword))

        getStatus = 500
        val thrown = runCatching { EnvelopeService.fetchServerKey(client, userId, notesPassword) }.exceptionOrNull()
        assertTrue(thrown is SyncClientException)
    }

    @Test fun `a server failure during reconcile uploads nothing`() = runBlocking {
        getStatus = 503

        val thrown = runCatching { reconcile() }.exceptionOrNull()

        assertTrue(thrown is SyncClientException)
        assertTrue(puts.isEmpty())
    }

    @Test fun `with no key on the server, the device publishes`() = runBlocking {
        assertEquals(EnvelopeService.Reconciliation.PublishFirst, reconcile())
    }

    @Test fun `a blob that opens to the same key needs no upload`() = runBlocking {
        blob = wrappedWith(notesPassword)

        assertEquals(EnvelopeService.Reconciliation.InStep(finishMigration = false), reconcile())
        assertEquals(EnvelopeService.Reconciliation.InStep(finishMigration = true), reconcile(legacy = true))
    }

    @Test fun `a blob that opens to another key is never overwritten by an envelope vault`() = runBlocking {
        blob = wrappedWith(notesPassword, CryptoService.generateSalt())

        assertEquals(EnvelopeService.Reconciliation.Diverged, reconcile())
    }

    @Test fun `a legacy vault moves onto the account's key`() = runBlocking {
        val accountKey = CryptoService.generateSalt()
        blob = wrappedWith(notesPassword, accountKey)

        val action = reconcile(legacy = true) as EnvelopeService.Reconciliation.MoveToAccountKey

        assertArrayEquals(accountKey, action.accountKey)
    }

    @Test fun `a blob that won't open heals only if the device can vouch for its key`() = runBlocking {
        blob = wrappedWith(accountPassword)

        assertEquals(EnvelopeService.Reconciliation.Repair(null), reconcile(canVouch = true))
        assertEquals(EnvelopeService.Reconciliation.CannotHeal, reconcile(canVouch = false))
    }

    @Test fun `a device that has opened the blob before never heals over it`() = runBlocking {
        blob = wrappedWith("changed-${UUID.randomUUID()}")

        assertEquals(
            EnvelopeService.Reconciliation.PasswordChangedElsewhere,
            reconcile(canVouch = true, opened = true),
        )
    }

    @Test fun `what is published opens with the notes password and not the account password`() = runBlocking {
        EnvelopeService.publishKey(client, userId, notesPassword, masterKey, EnvelopeService.KeyWrite.FirstUpload)

        val sent = Json.decodeFromString<PutWrappedKeyRequest>(puts.single())
        val wrapped = CryptoService.parseEncryptedJSON(sent.blob)
        assertArrayEquals(
            masterKey,
            CryptoService.unwrapMasterKey(wrapped, CryptoService.deriveWrappingKey(notesPassword, userId)),
        )
        val wrongPassword = runCatching {
            CryptoService.unwrapMasterKey(wrapped, CryptoService.deriveWrappingKey(accountPassword, userId))
        }
        assertTrue(wrongPassword.isFailure)
    }

    // MARK: - Re-keying

    private class Vault {
        val db: JotteryDatabase = JotteryDatabase.buildInMemory(ApplicationProvider.getApplicationContext())
        val notes = NoteRepository(db.noteDao(), db.noteVersionDao())
        val versions = VersionRepository(db.noteVersionDao())
        val attachments = AttachmentRepository(db.attachmentDao())
        val searches = SavedSearchRepository(db.savedSearchDao())
        val encryption = EncryptionRepository(db.encryptionMetadataDao())
        val service = VaultReKeyService(notes, versions, attachments, searches, encryption) { block ->
            db.withTransaction { block() }
        }
    }

    private val oldKey = SecretKeySpec(ByteArray(32) { 1 }, "AES")
    private val newKey = SecretKeySpec(ByteArray(32) { 2 }, "AES")

    private fun enc(text: String, key: javax.crypto.SecretKey) =
        CryptoService.serializeEncryptedJSON(CryptoService.encryptText(text, key))

    private fun row(id: String, content: String, key: javax.crypto.SecretKey) = NoteRecord(
        id = id, createdAt = "t", modifiedAt = "t", content = content,
        tags = CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(listOf("t"), key)),
        needsSync = false,
    )

    private val newMetadata = EncryptionMetadata(createdAt = "t", verification = "v", envelopeVersion = 1)

    @Test fun `a re-key moves everything and leaves rows already on the new key`() = runBlocking {
        val vault = Vault()
        vault.encryption.store(EncryptionMetadata(createdAt = "t", salt = "s", iterations = 1))
        vault.notes.insertOrReplace(row("old", enc("on old key", oldKey), oldKey))
        val alreadyNew = row("new", enc("on new key", newKey), newKey)
        vault.notes.insertOrReplace(alreadyNew)
        val version = NoteVersionRecord.from(row("old", enc("old version", oldKey), oldKey), "sync")
        vault.versions.insertOrReplace(version)

        vault.service.reKey(oldKey, newKey, newMetadata)

        assertEquals("on old key", vault.notes.get("old", newKey)!!.content)
        assertTrue(vault.notes.getRaw("old")!!.needsSync)
        assertEquals("on new key", vault.notes.get("new", newKey)!!.content)
        assertEquals(alreadyNew.content, vault.notes.getRaw("new")!!.content)
        assertEquals(1, vault.encryption.get()!!.envelopeVersion)
        val stored = vault.versions.getVersions("old").single()
        assertEquals("old version", CryptoService.decryptText(CryptoService.parseEncryptedJSON(stored.content), newKey))
        vault.db.close()
    }

    @Test fun `a value under neither key rolls the whole re-key back`() = runBlocking {
        val vault = Vault()
        val before = EncryptionMetadata(createdAt = "t", salt = "s", iterations = 1)
        vault.encryption.store(before)
        vault.notes.insertOrReplace(row("a-good", enc("readable", oldKey), oldKey))
        val foreign = SecretKeySpec(ByteArray(32) { 3 }, "AES")
        vault.notes.insertOrReplace(row("z-bad", enc("unreadable", foreign), foreign))

        val thrown = runCatching { vault.service.reKey(oldKey, newKey, newMetadata) }.exceptionOrNull()

        assertTrue(thrown is VaultReKeyService.Unreadable)
        assertEquals("readable", vault.notes.get("a-good", oldKey)!!.content)
        assertEquals(before, vault.encryption.get())
        vault.db.close()
    }
}
