package com.jottery.android.viewmodel

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.model.NoteVersionRecord
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.UUID

/** The real AppState against a real database, without sync. */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class AppStateNotesTest {
    private lateinit var appState: AppState
    private val password = "vault-${UUID.randomUUID()}"

    @Before fun setUp() {
        Dispatchers.setMain(Dispatchers.Unconfined)
        appState = AppState(ApplicationProvider.getApplicationContext<Application>())
        runBlocking { withTimeout(30_000) { while (!appState.isInitialised.value) delay(5) } }
        runBlocking { appState.createVault(password) }
    }

    @After fun tearDown() {
        JotteryDatabase.destroyInstance()
        Dispatchers.resetMain()
    }

    private suspend fun shown(id: String) =
        withTimeout(5_000) {
            while (appState.filteredNotes.value.none { it.id == id }) delay(5)
            appState.filteredNotes.value.first { it.id == id }
        }

    @Test fun `restoring a version updates the list the editor reads`() = runBlocking {
        val note = appState.createNote("original")!!
        appState.saveNote(note.copy(content = "first edit"))
        val version = appState.getVersionsForNote(note.id).single { it.version == 0 }

        assertTrue(appState.restoreVersion(note.id, version))

        withTimeout(5_000) { while (shown(note.id).content != "original") delay(5) }
        assertEquals("original", appState.notes.value.single().content)
    }

    @Test fun `restoring twice keeps both snapshots`() = runBlocking {
        val note = appState.createNote("original")!!
        appState.saveNote(note.copy(content = "first edit"))
        val version = appState.getVersionsForNote(note.id).single { it.version == 0 }

        appState.restoreVersion(note.id, version)
        Thread.sleep(5)
        appState.restoreVersion(note.id, version)

        assertEquals(2, appState.getVersionsForNote(note.id).count { it.reason == "pre-restore" })
    }

    @Test fun `a version from an older build with no tags restores without crashing`() = runBlocking {
        val note = appState.createNote("original")!!
        val key = appState.keyManager.masterKey!!
        val old = NoteVersionRecord(
            versionKey = "${note.id}:9", noteId = note.id, version = 9, createdAt = "t", syncedAt = "t",
            content = CryptoService.serializeEncryptedJSON(CryptoService.encryptText("old text", key)),
            tags = "[]", attachments = "[]", reason = "sync",
        )

        assertTrue(appState.restoreVersion(note.id, old))
        assertEquals("old text", appState.notes.value.single().content)
    }

    @Test fun `a version that can't be read changes nothing and doesn't crash`() = runBlocking {
        val note = appState.createNote("original")!!
        val broken = NoteVersionRecord(
            versionKey = "${note.id}:9", noteId = note.id, version = 9, createdAt = "t", syncedAt = "t",
            content = "not encrypted", tags = "[]", attachments = "[]", reason = "sync",
        )

        assertFalse(appState.restoreVersion(note.id, broken))
        assertEquals("original", appState.notes.value.single().content)
    }

    @Test fun `pinning and locking reach the list`() = runBlocking {
        val note = appState.createNote("text")!!

        appState.togglePin(note.id)
        appState.toggleLock(note.id)

        assertTrue(appState.notes.value.single().pinned)
        assertTrue(appState.notes.value.single().locked)
    }

    @Test fun `a save from the editor's copy doesn't undo a delete`() = runBlocking {
        val note = appState.createNote("text")!!
        appState.deleteNote(note.id)

        appState.saveNote(note.copy(content = "typed before the delete"))

        assertTrue(appState.loadDeletedNotes().any { it.id == note.id })
        assertTrue(appState.notes.value.isEmpty())
    }

    @Test fun `changing the password keeps the key and opens with the new password only`() = runBlocking {
        val note = appState.createNote("text")!!
        val keyBefore = appState.keyManager.masterKey!!.encoded.toList()
        val newPassword = "new-${UUID.randomUUID()}"

        appState.changePassword(password, newPassword)
        appState.lock()

        val wrong = runCatching { appState.unlock(password) }.exceptionOrNull()
        assertTrue(wrong is AppStateError.WrongPassword)
        assertTrue(appState.unlock(newPassword))
        assertEquals(keyBefore, appState.keyManager.masterKey!!.encoded.toList())
        assertEquals("text", appState.notes.value.single { it.id == note.id }.content)
    }

    @Test fun `the wrong current password changes nothing`() = runBlocking {
        val thrown = runCatching { appState.changePassword("wrong", "whatever-long") }.exceptionOrNull()

        assertTrue(thrown is AppStateError.WrongPassword)
        appState.lock()
        assertTrue(appState.unlock(password))
    }

    @Test fun `typing not yet saved survives a lock`() = runBlocking {
        val note = appState.createNote("saved")!!
        appState.setPendingEditorNote(note.copy(content = "typed a moment before the lock"))

        appState.lock()
        appState.unlock(password)

        withTimeout(5_000) { while (shown(note.id).content != "typed a moment before the lock") delay(5) }
    }

    @Test fun `a lock leaves nothing decrypted behind`() = runBlocking {
        appState.createNote("private")
        appState.saveSearch("Work", "#work")
        appState.setSearchQuery("private")
        val key = appState.keyManager.masterKey!!

        appState.lock()

        withTimeout(5_000) { while (!key.isDestroyed) delay(5) }
        assertTrue(appState.savedSearches.value.isEmpty())
        assertEquals("", appState.searchQuery.value)
        assertTrue(appState.notes.value.isEmpty())
        assertTrue(appState.filteredNotes.value.isEmpty())
    }

    @Test fun `a search from before a lock can't republish notes after it`() = runBlocking {
        appState.createNote("private")
        appState.setSearchQuery("private")

        appState.lock()
        delay(300) // Longer than the search debounce

        assertTrue(appState.filteredNotes.value.isEmpty())
    }

    @Test fun `a failed setup after the key is proved isn't reported as a wrong password`() = runBlocking {
        // syncEnabled with no credentials: the keystore can't be reached under
        // Robolectric, so setup throws where a real device's Keystore might.
        val settings = appState.settings.value
        appState.updateSettings(settings.copy(syncEnabled = true, syncEndpoint = "http://localhost:1"))
        appState.lock()

        assertTrue(appState.unlock(password))

        assertFalse("the vault opened", appState.isLocked.value)
        assertTrue(appState.syncError.value != null)
    }
}
