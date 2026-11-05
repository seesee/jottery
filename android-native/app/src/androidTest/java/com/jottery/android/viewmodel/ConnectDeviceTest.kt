package com.jottery.android.viewmodel

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.model.PutWrappedKeyRequest
import com.jottery.android.model.RegisterDeviceResponse
import com.jottery.android.model.SyncAccepted
import com.jottery.android.model.SyncNote
import com.jottery.android.model.SyncPullResponse
import com.jottery.android.model.SyncPushRequest
import com.jottery.android.model.SyncPushResponse
import com.jottery.android.model.WrappedKeyResponse
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.util.UUID
import java.util.concurrent.CopyOnWriteArrayList
import javax.crypto.spec.SecretKeySpec

/**
 * Joining an account from start to finish, on a device: the real Keystore, the
 * real database and real HTTP to a stub server. Robolectric has no Keystore, so
 * this is the only place the success path runs.
 */
@RunWith(AndroidJUnit4::class)
class ConnectDeviceTest {
    private val userId = UUID.randomUUID().toString()
    private val notesPassword = "notes-${UUID.randomUUID()}"
    private val accountPassword = "account-${UUID.randomUUID()}"
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    private var blob: String? = null
    private val puts = CopyOnWriteArrayList<String>()
    private val pushed = CopyOnWriteArrayList<SyncPushRequest>()
    private var pullNotes: List<SyncNote> = emptyList()

    private val server = MockWebServer().apply {
        dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse = when {
                request.path == "/api/v1/auth/register-device" -> MockResponse().setBody(
                    json.encodeToString(RegisterDeviceResponse("client-a", "api-key-a", userId, "device"))
                )
                request.path == "/api/v1/auth/wrapped-key" && request.method == "PUT" -> {
                    puts += request.body.readUtf8()
                    blob = json.decodeFromString<PutWrappedKeyRequest>(puts.last()).blob
                    MockResponse().setBody("{}")
                }
                request.path == "/api/v1/auth/wrapped-key" -> blob?.let {
                    MockResponse().setBody(json.encodeToString(WrappedKeyResponse(it, 1, 1_000_000)))
                } ?: MockResponse().setResponseCode(404).setBody("{}")
                request.path == "/api/v1/sync/push" -> {
                    val push = json.decodeFromString<SyncPushRequest>(request.body.readUtf8())
                    pushed += push
                    MockResponse().setBody(
                        json.encodeToString(
                            SyncPushResponse(
                                accepted = push.notes.map { SyncAccepted(it.id, 2, "2026-02-01T00:00:00+00:00") },
                                rejected = emptyList(),
                            )
                        )
                    )
                }
                request.path == "/api/v1/sync/pull" -> MockResponse().setBody(
                    json.encodeToString(
                        SyncPullResponse(
                            notes = pullNotes,
                            attachments = emptyList(),
                            versions = emptyList(),
                            syncedAt = "2026-03-01T00:00:00+00:00",
                            hasMore = false,
                        )
                    )
                )
                else -> MockResponse().setResponseCode(404).setBody("[]")
            }
        }
        start()
    }

    private val endpoint get() = server.url("/").toString().trimEnd('/')
    private val application = ApplicationProvider.getApplicationContext<Application>()
    private lateinit var appState: AppState

    @Before fun setUp() {
        JotteryDatabase.destroyInstance()
        application.deleteDatabase("jottery.db")
        InstrumentationRegistry.getInstrumentation().runOnMainSync { appState = AppState(application) }
        runBlocking { withTimeout(30_000) { appState.isInitialised.first { it } } }
    }

    @After fun tearDown() {
        InstrumentationRegistry.getInstrumentation().runOnMainSync { appState.wipeAllData() }
        JotteryDatabase.destroyInstance()
        application.deleteDatabase("jottery.db")
        server.shutdown()
    }

    private suspend fun settled(): AppState.ConnectState = withTimeout(120_000) {
        appState.connectState.first {
            it !is AppState.ConnectState.Working && it !is AppState.ConnectState.Idle
        }
    }

    private suspend fun register() {
        appState.registerDevice(endpoint, "me@example.com", accountPassword, "Test").getOrThrow()
    }

    private fun onMain(block: () -> Unit) =
        InstrumentationRegistry.getInstrumentation().runOnMainSync(block)

    @Test fun theFirstDevicePublishesItsKeyBeforeSyncStarts() = runBlocking {
        register()
        onMain { appState.connectNewVault(notesPassword) }
        assertEquals(AppState.ConnectState.ConfirmNewPassword, settled())

        onMain { appState.clearConnectState() }
        onMain { appState.connectNewVault(notesPassword, notesPassword) }

        assertEquals(AppState.ConnectState.Done, settled())
        assertFalse(appState.isFirstLaunch.value)
        assertFalse(appState.isLocked.value)
        assertTrue(appState.settings.value.syncEnabled)
        assertTrue(appState.syncEnabled.value)

        val wrapped = CryptoService.parseEncryptedJSON(blob!!)
        val master = CryptoService.unwrapMasterKey(wrapped, CryptoService.deriveWrappingKey(notesPassword, userId))
        assertArrayEquals(master, appState.keyManager.masterKey!!.encoded)

        // The vault really is on that key, behind the same password
        appState.lock()
        assertTrue(appState.unlock(notesPassword))
        assertArrayEquals(master, appState.keyManager.masterKey!!.encoded)
    }

    @Test fun aSecondDeviceJoinsOnTheAccountKeyAndReadsTheAccountsNotes() = runBlocking {
        val master = CryptoService.generateSalt()
        val key = SecretKeySpec(master, "AES")
        blob = CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(master, CryptoService.deriveWrappingKey(notesPassword, userId))
        )
        val noteId = UUID.randomUUID().toString()
        pullNotes = listOf(
            SyncNote(
                id = noteId,
                createdAt = "2026-01-01T00:00:00.000Z",
                modifiedAt = "2026-01-01T00:00:00.000Z",
                content = CryptoService.serializeEncryptedJSON(CryptoService.encryptText("From my laptop", key)),
                tags = emptyList(),
                attachments = emptyList(),
                pinned = false,
                deleted = false,
                version = 1,
                contentHash = "hash",
                hashChain = listOf("hash"),
            )
        )
        register()

        onMain { appState.connectNewVault(notesPassword) }

        assertEquals(AppState.ConnectState.Done, settled())
        assertTrue("no key may be uploaded over the account's", puts.isEmpty())
        assertArrayEquals(master, appState.keyManager.masterKey!!.encoded)
        withTimeout(30_000) { while (appState.notes.value.none { it.id == noteId }) delay(50) }
        assertEquals("From my laptop", appState.notes.value.single { it.id == noteId }.content)
        assertNotNull(appState.lastSyncAt.value)
    }

    @Test fun aNewNoteIsPushedUnderTheAccountKey() = runBlocking {
        val master = CryptoService.generateSalt()
        val key = SecretKeySpec(master, "AES")
        blob = CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(master, CryptoService.deriveWrappingKey(notesPassword, userId))
        )
        register()
        onMain { appState.connectNewVault(notesPassword) }
        assertEquals(AppState.ConnectState.Done, settled())

        val note = appState.createNote("Written here")!!
        appState.triggerSync()
        withTimeout(30_000) { while (pushed.none { p -> p.notes.any { it.id == note.id } }) delay(50) }

        val sent = pushed.flatMap { it.notes }.first { it.id == note.id }
        val text = CryptoService.decryptText(CryptoService.parseEncryptedJSON(sent.content), key)
        assertEquals("Written here", text)
    }
}
