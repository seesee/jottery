package com.jottery.android.viewmodel

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.model.EncryptionMetadata
import com.jottery.android.model.NoteRecord
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withTimeout
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
import javax.crypto.SecretKey

/**
 * A vault from an older build, with no verification token. Nothing but a note can
 * prove a password here, and storing a token under the wrong key locks out the right one.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class LegacyUnlockTest {
    private lateinit var appState: AppState
    private lateinit var db: JotteryDatabase
    private val password = "right-${UUID.randomUUID()}"
    private val wrong = "wrong-${UUID.randomUUID()}"
    private val salt = CryptoService.generateSalt()
    private val key: SecretKey by lazy { CryptoService.deriveKey(password, salt, CryptoService.MIN_ITERATIONS) }

    @Before fun setUp() {
        Dispatchers.setMain(Dispatchers.Unconfined)
        val context = ApplicationProvider.getApplicationContext<Application>()
        appState = AppState(context)
        runBlocking { withTimeout(30_000) { while (!appState.isInitialised.value) delay(5) } }
        db = JotteryDatabase.getInstance(context)
    }

    @After fun tearDown() {
        JotteryDatabase.destroyInstance()
        Dispatchers.resetMain()
    }

    private suspend fun seedVault() {
        db.encryptionMetadataDao().insertOrReplace(
            EncryptionMetadata.new(salt, CryptoService.MIN_ITERATIONS, "unused").copy(verification = null)
        )
    }

    private suspend fun seedNote(content: String) = db.noteDao().insertOrReplace(
        NoteRecord.new(
            encryptedContent = content,
            encryptedTags = CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(emptyList(), key)),
        )
    )

    private fun encrypted(text: String) = CryptoService.serializeEncryptedJSON(CryptoService.encryptText(text, key))

    private suspend fun token() = db.encryptionMetadataDao().get()?.verification

    private suspend fun attempt(password: String) = runCatching { appState.unlock(password) }.exceptionOrNull()

    @Test fun `a wrong password is refused, and leaves no token behind`() = runBlocking {
        seedVault()
        seedNote(encrypted("Intact"))

        assertEquals(AppStateError.WrongPassword, attempt(wrong))

        assertNull(token())
        assertTrue(appState.isLocked.value)
    }

    @Test fun `the right password still opens after a wrong one`() = runBlocking {
        seedVault()
        seedNote(encrypted("Intact"))
        attempt(wrong)

        assertNull(attempt(password))

        assertEquals("Intact", appState.notes.value.single().content)
    }

    @Test fun `the right password earns a token, and that refuses the wrong one`() = runBlocking {
        seedVault()
        seedNote(encrypted("Intact"))

        attempt(password)
        withTimeout(5_000) { while (token() == null) delay(5) }
        appState.lock()

        assertEquals(AppStateError.WrongPassword, attempt(wrong))
        assertNull(attempt(password))
    }

    @Test fun `one damaged note doesn't lock out the right password`() = runBlocking {
        seedVault()
        seedNote("not ciphertext at all")
        seedNote(encrypted("Intact"))

        assertNull(attempt(password))
    }

    @Test fun `an empty vault proves nothing, so it opens without storing a token`() = runBlocking {
        seedVault()

        assertNull(attempt(wrong))
        delay(200)

        assertNull(token())
        assertNotNull(db.encryptionMetadataDao().get())
    }
}
