package com.jottery.android.viewmodel

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import com.jottery.android.R
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.model.RegisterDeviceResponse
import com.jottery.android.model.WrappedKeyResponse
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.UUID

/** A failed join must leave nothing behind: no vault, no sync, no key on a new device. */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class ConnectFlowTest {
    private val userId = UUID.randomUUID().toString()
    private val notesPassword = "notes-${UUID.randomUUID()}"
    private val accountPassword = "account-${UUID.randomUUID()}"

    private var keyStatus = 404
    private var blob: String? = null
    private var revokes = 0

    private val server = MockWebServer().apply {
        dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse = when {
                request.path == "/api/v1/auth/register-device" -> MockResponse().setBody(
                    Json.encodeToString(RegisterDeviceResponse("client", "api-key", userId, "device"))
                )
                request.path == "/api/v1/auth/wrapped-key" -> blob?.let {
                    MockResponse().setBody(Json.encodeToString(WrappedKeyResponse(it, 1, 1_000_000)))
                } ?: MockResponse().setResponseCode(keyStatus).setBody("{}")
                request.path == "/api/v1/sync/device" -> {
                    revokes++
                    MockResponse().setBody("{}")
                }
                else -> MockResponse().setResponseCode(404).setBody("{}")
            }
        }
        start()
    }

    private lateinit var appState: AppState
    private val endpoint get() = server.url("/").toString().trimEnd('/')

    @Before fun setUp() {
        Dispatchers.setMain(Dispatchers.Unconfined)
        appState = AppState(ApplicationProvider.getApplicationContext<Application>())
    }

    @After fun tearDown() {
        JotteryDatabase.destroyInstance()
        Dispatchers.resetMain()
        server.shutdown()
    }

    private suspend fun register() {
        appState.registerDevice(endpoint, "user@example.com", accountPassword, "Test").getOrThrow()
    }

    private suspend fun settled(): AppState.ConnectState = withTimeout(60_000) {
        appState.connectState.first {
            it !is AppState.ConnectState.Working && it !is AppState.ConnectState.Idle
        }
    }

    private fun vault() = runBlocking {
        JotteryDatabase.getInstance(ApplicationProvider.getApplicationContext()).encryptionMetadataDao().get()
    }

    private fun assertNothingCreated() {
        assertNull(vault())
        assertTrue(appState.isFirstLaunch.value)
        assertTrue(appState.isLocked.value)
        assertEquals(false, appState.settings.value.syncEnabled)
        assertNotNull("the user stays on the screen and can try again", appState.pendingConnection.value)
    }

    @Test fun `the wrong notes password creates no vault`() = runBlocking {
        blob = CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(
                CryptoService.generateSalt(),
                CryptoService.deriveWrappingKey("the-real-password-${UUID.randomUUID()}", userId),
            )
        )
        register()

        appState.connectNewVault(notesPassword)

        assertTrue(settled() is AppState.ConnectState.Failed)
        assertNothingCreated()
    }

    @Test fun `a network failure on the key creates no vault`() = runBlocking {
        keyStatus = 503
        register()

        appState.connectNewVault(notesPassword)

        assertTrue(settled() is AppState.ConnectState.Failed)
        assertNothingCreated()
    }

    @Test fun `matching passwords are refused`() = runBlocking {
        register()

        appState.connectNewVault(accountPassword)

        assertTrue(settled() is AppState.ConnectState.Failed)
        assertNothingCreated()
    }

    @Test fun `a matching password that already opens the account's key is only warned about`() = runBlocking {
        // D2: an existing password can't be refused, or these users can't add a device
        blob = CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(
                CryptoService.generateSalt(),
                CryptoService.deriveWrappingKey(accountPassword, userId),
            )
        )
        register()

        appState.connectNewVault(accountPassword)

        withTimeout(60_000) { appState.userMessage.first { it != null } }
        val app = ApplicationProvider.getApplicationContext<Application>()
        assertEquals(app.getString(R.string.password_matches_account_warning), appState.userMessage.value)
        withTimeout(60_000) { while (vault() == null) kotlinx.coroutines.delay(10) }
    }

    @Test fun `a short password is refused for a new account key`() = runBlocking {
        register()

        appState.connectNewVault("short")

        assertTrue(settled() is AppState.ConnectState.Failed)
        assertNothingCreated()
    }

    @Test fun `a new password has to be typed twice before anything is created`() = runBlocking {
        register()

        appState.connectNewVault(notesPassword)
        assertEquals(AppState.ConnectState.ConfirmNewPassword, settled())
        assertNothingCreated()

        appState.clearConnectState()
        appState.connectNewVault(notesPassword, "${notesPassword}typo")
        assertTrue(settled() is AppState.ConnectState.Failed)
        assertNothingCreated()
    }

    @Test fun `a registered device isn't connected until it holds a key`() = runBlocking {
        register()

        assertEquals(false, appState.settings.value.syncEnabled)
        assertEquals(false, appState.syncEnabled.value)
    }

    @Test fun `abandoning a registration revokes the device`() = runBlocking {
        register()

        appState.abandonPendingConnection()

        withTimeout(10_000) { while (revokes == 0) kotlinx.coroutines.delay(10) }
        assertNull(appState.pendingConnection.value)
    }
}
