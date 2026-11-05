package com.jottery.android.viewmodel

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.JotteryDatabase
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.model.NoteCategories
import com.jottery.android.util.KeyboardCommand
import com.jottery.android.util.QuickAction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withTimeout
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
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

    @Test fun `the new note shortcut waits for the vault to open`() = runBlocking {
        appState.lock()

        appState.receiveQuickAction(QuickAction.NEW_NOTE)
        assertNull(appState.selectedNoteId.value)

        appState.unlock(password)
        withTimeout(5_000) { while (appState.selectedNoteId.value == null) delay(5) }
        assertEquals(1, appState.notes.value.size)
    }

    @Test fun `the new note shortcut needs no waiting when the vault is open`() = runBlocking {
        appState.receiveQuickAction(QuickAction.NEW_NOTE)

        withTimeout(5_000) { while (appState.selectedNoteId.value == null) delay(5) }
    }

    @Test fun `the search shortcut leaves the note, and waits for the screen to take it up`() = runBlocking {
        val note = appState.createNote("Open")!!
        assertEquals(note.id, appState.selectedNoteId.value)
        appState.lock()

        appState.receiveQuickAction(QuickAction.SEARCH)
        appState.unlock(password)

        withTimeout(5_000) { while (!appState.searchRequested.value) delay(5) }
        assertNull(appState.selectedNoteId.value)
        appState.searchRequestHandled()
        assertFalse(appState.searchRequested.value)
    }

    @Test fun `a duplicate keeps what makes the note what it is`() = runBlocking {
        val note = appState.createNote("1 + 1", listOf("sums"))!!
        appState.saveNote(
            note.copy(syntaxLanguage = "calc", color = "red", wordWrap = false, showPreview = true, pinned = true)
        )
        val attachmentId = appState.addAttachment(note.id, "bytes".toByteArray(), "sums.txt", "text/plain")!!

        appState.duplicateNote(note.id)

        val copy = appState.notes.value.first { it.id == appState.selectedNoteId.value }
        assertNotEquals(note.id, copy.id)
        assertEquals("1 + 1", copy.content)
        assertEquals(listOf("sums"), copy.tags)
        assertEquals("calc", copy.syntaxLanguage)
        assertEquals("red", copy.color)
        assertEquals(false, copy.wordWrap)
        assertEquals(true, copy.showPreview)
        assertEquals(attachmentId, copy.attachments.single().id)
        assertEquals("sums.txt", copy.attachments.single().filename)
        assertFalse("a copy doesn't jump the queue", copy.pinned)
    }

    @Test fun `a duplicate takes the text on screen, not what was last saved`() = runBlocking {
        val note = appState.createNote("Saved")!!
        appState.setPendingEditorNote(note.copy(content = "Still being typed"))

        appState.duplicateNote(note.id)

        val copy = appState.notes.value.first { it.id == appState.selectedNoteId.value }
        assertEquals("Still being typed", copy.content)
    }

    private fun server(vararg responses: MockResponse) = MockWebServer().apply {
        responses.forEach(::enqueue)
        start()
    }

    private suspend fun connectStateSettles() =
        withTimeout(5_000) { while (appState.connectState.value is AppState.ConnectState.Working) delay(5) }

    @Test fun `registering carries on in the view model, and hands over the connection`() = runBlocking {
        val server = server(MockResponse().setBody("""{"clientId":"c","apiKey":"k","userId":"u","deviceName":"Phone"}"""))
        var handedOver: AppState.PendingConnection? = null

        appState.beginRegistration(server.url("/").toString().trimEnd('/'), "a@example.test", "p", "Phone") { handedOver = it }
        connectStateSettles()
        withTimeout(5_000) { while (handedOver == null) delay(5) }

        assertEquals("k", handedOver!!.apiKey)
        assertEquals(handedOver, appState.pendingConnection.value)
        assertEquals(AppState.ConnectState.Idle, appState.connectState.value)
        server.shutdown()
    }

    @Test fun `a refused registration says why, with no connection left behind`() = runBlocking {
        val server = server(MockResponse().setResponseCode(401).setBody("""{"error":"no"}"""))

        appState.beginRegistration(server.url("/").toString().trimEnd('/'), "a@example.test", "p", "Phone")
        connectStateSettles()

        assertTrue(appState.connectState.value is AppState.ConnectState.Failed)
        assertNull(appState.pendingConnection.value)
        server.shutdown()
    }

    @Test fun `a new note takes the default type, and that's remembered`() = runBlocking {
        assertEquals("markdown", appState.createNote("Before")!!.syntaxLanguage)

        appState.setDefaultNoteType("calc")

        assertEquals("calc", appState.createNote("After")!!.syntaxLanguage)
        val reopened = AppState(ApplicationProvider.getApplicationContext<Application>())
        assertEquals("calc", reopened.defaultNoteType.value)
        appState.setDefaultNoteType("markdown")
    }

    @Test fun `what's shared in stays text, whatever the default`() = runBlocking {
        appState.setDefaultNoteType("calc")

        appState.receiveShare("A link from another app", emptyList())

        withTimeout(5_000) { while (appState.notes.value.isEmpty()) delay(5) }
        assertEquals("markdown", appState.notes.value.single().syntaxLanguage)
        appState.setDefaultNoteType("markdown")
    }

    @Test fun `naming a colour reaches search, and survives a restart of the app's state`() = runBlocking {
        val note = appState.createNote("Coloured")!!
        appState.saveNote(note.copy(color = "red"))
        appState.createNote("Plain")
        appState.setCategoryName("red", "Urgent")

        appState.setSearchQuery("category:urgent")

        withTimeout(5_000) { while (appState.filteredNotes.value.size != 1) delay(5) }
        assertEquals("Coloured", appState.filteredNotes.value.single().content)
        assertEquals("Urgent", NoteCategories.from(appState.settings.value).names["red"])
    }

    @Test fun `a tag's colour reaches search`() = runBlocking {
        appState.createNote("Tagged", listOf("work"))
        appState.createNote("Plain")
        appState.setTagColour("work", "blue")

        appState.setSearchQuery("color:blue tag")

        withTimeout(5_000) { while (appState.filteredNotes.value.size != 1) delay(5) }
        assertEquals("Tagged", appState.filteredNotes.value.single().content)
    }

    @Test fun `a search says how many match in the view not showing`() = runBlocking {
        val archived = appState.createNote("Hidden treasure")!!
        appState.createNote("Something else")
        appState.archiveNote(archived.id)

        appState.setSearchQuery("treasure")

        withTimeout(5_000) { while (appState.otherViewMatches.value != 1) delay(5) }
        assertTrue(appState.filteredNotes.value.isEmpty())

        appState.setSearchQuery("nothing matches this")
        withTimeout(5_000) { while (appState.otherViewMatches.value != 0) delay(5) }
    }

    @Test fun `recent searches are gone once the vault is locked`() = runBlocking {
        appState.rememberSearch("something private")
        assertEquals(listOf("something private"), appState.recentSearches.value)

        appState.lock()

        assertTrue(appState.recentSearches.value.isEmpty())
    }

    @Test fun `a keyboard makes a note, and saves what's being typed`() = runBlocking {
        assertTrue(appState.perform(KeyboardCommand.NEW_NOTE))
        withTimeout(5_000) { while (appState.selectedNoteId.value == null) delay(5) }
        val note = appState.notes.value.single()

        appState.setPendingEditorNote(note.copy(content = "Typed"))
        assertTrue(appState.perform(KeyboardCommand.SAVE))

        withTimeout(5_000) { while (appState.notes.value.single().content != "Typed") delay(5) }
    }

    @Test fun `search and find ask the screen for what they need`() = runBlocking {
        assertFalse("nothing to find in yet", appState.perform(KeyboardCommand.FIND))

        appState.createNote("Open")
        assertTrue(appState.perform(KeyboardCommand.FIND))
        assertTrue(appState.findRequested.value)
        appState.findRequestHandled()

        assertTrue(appState.perform(KeyboardCommand.SEARCH))
        withTimeout(5_000) { while (!appState.searchRequested.value) delay(5) }
    }

    @Test fun `a keyboard locks the vault, and does nothing else to a locked one`() = runBlocking {
        assertTrue(appState.perform(KeyboardCommand.LOCK))
        assertTrue(appState.isLocked.value)

        assertFalse(appState.perform(KeyboardCommand.NEW_NOTE))
        appState.unlock(password)
        assertTrue("nothing was held back for later", appState.notes.value.isEmpty())
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

    // MARK: - Selection

    @Test fun `an open note stays open when it stops matching the search`() = runBlocking {
        val note = appState.createNote("todo: buy milk")!!
        appState.setSearchQuery("todo")
        shown(note.id)
        appState.setSelectedNoteId(note.id)

        appState.saveNote(note.copy(content = "buy milk"))
        withTimeout(5_000) { while (appState.filteredNotes.value.isNotEmpty()) delay(5) }

        assertEquals(note.id, appState.selectedNote?.id)
    }

    @Test fun `a new note opens during a search, though it matches nothing yet`() = runBlocking {
        appState.setSearchQuery("anything")

        val note = appState.createNote()!!

        assertEquals(note.id, appState.selectedNote?.id)
    }

    @Test fun `an archived note can stay selected`() = runBlocking {
        val note = appState.createNote("keep me")!!
        appState.archiveNote(note.id)
        appState.setSelectedNoteId(note.id)
        withTimeout(5_000) { while (appState.selectedNote == null) delay(5) }
    }

    // MARK: - Protected notes

    @Test fun `a pinned or locked note can't be swiped away, and others can be undone`() = runBlocking {
        val pinned = appState.createNote("pinned")!!
        appState.togglePin(pinned.id)
        val locked = appState.createNote("locked")!!
        appState.toggleLock(locked.id)
        val plain = appState.createNote("plain")!!

        fun current(id: String) = appState.notes.value.first { it.id == id }

        assertFalse(appState.deleteNoteWithUndo(current(pinned.id)))
        assertFalse(appState.deleteNoteWithUndo(current(locked.id)))
        assertEquals(3, appState.notes.value.size)

        assertTrue(appState.deleteNoteWithUndo(current(plain.id)))
        assertEquals(plain.id, appState.undoableDelete.value)
        assertEquals(2, appState.notes.value.size)

        appState.restoreNote(plain.id)
        withTimeout(5_000) { while (appState.notes.value.size != 3) delay(5) }
    }

    // MARK: - Bulk

    @Test fun `a bulk delete leaves pinned and locked notes and says how many`() = runBlocking {
        val pinned = appState.createNote("pinned")!!
        appState.togglePin(pinned.id)
        val locked = appState.createNote("locked")!!
        appState.toggleLock(locked.id)
        val plain = appState.createNote("plain")!!

        val skipped = appState.deleteNotes(setOf(pinned.id, locked.id, plain.id))

        assertEquals(2, skipped)
        withTimeout(5_000) { while (appState.notes.value.size != 2) delay(5) }
        assertEquals(1, appState.loadDeletedNotes().size)
    }

    @Test fun `colour and archive reach every selected note`() = runBlocking {
        val a = appState.createNote("a")!!
        val b = appState.createNote("b")!!
        val locked = appState.createNote("locked")!!
        appState.toggleLock(locked.id)
        val ids = setOf(a.id, b.id, locked.id)

        appState.setColourOnNotes(ids, "green")
        withTimeout(5_000) { while (appState.notes.value.count { it.color == "green" } != 2) delay(5) }

        assertEquals(1, appState.archiveNotes(ids))
        withTimeout(5_000) { while (appState.archivedNotes.value.size != 2) delay(5) }
        assertEquals(listOf(locked.id), appState.notes.value.map { it.id })
    }

    @Test fun `notes combine oldest first, keeping tags and attachments`() = runBlocking {
        val first = appState.createNote("first", listOf("a", "shared"))!!
        delay(5)
        val second = appState.createNote("second", listOf("shared", "b"))!!
        appState.addAttachment(second.id, byteArrayOf(1, 2, 3), "file.bin", "application/octet-stream")

        val combined = appState.combineNotes(setOf(second.id, first.id))!!

        withTimeout(5_000) { while (appState.notes.value.size != 1) delay(5) }
        val note = appState.notes.value.single()
        assertEquals(combined.id, note.id)
        assertEquals("first\n\n---\n\nsecond", note.content)
        assertEquals(listOf("a", "shared", "b"), note.tags)
        assertEquals("file.bin", note.attachments.single().filename)
        assertEquals(byteArrayOf(1, 2, 3).toList(), appState.attachmentData(note.attachments.single())!!.toList())
        assertEquals(2, appState.loadDeletedNotes().size)
    }

    @Test fun `locked notes take no part in a combine`() = runBlocking {
        val locked = appState.createNote("locked")!!
        appState.toggleLock(locked.id)
        val plain = appState.createNote("plain")!!

        assertEquals(null, appState.combineNotes(setOf(locked.id, plain.id)))
        assertEquals(2, appState.notes.value.size)
    }

    // MARK: - Saved searches

    @Test fun `saved searches keep their order, and a rename reaches the list`() = runBlocking {
        appState.saveSearch("One", "#one")
        appState.saveSearch("Two", "#two")
        val two = appState.savedSearches.value.last()

        appState.moveSavedSearch(two.id, -1)
        appState.renameSavedSearch(two.id, "Second")

        assertEquals(listOf("Second", "One"), appState.savedSearches.value.map { it.name })

        appState.lock()
        appState.unlock(password)
        withTimeout(5_000) { while (appState.savedSearches.value.size != 2) delay(5) }
        assertEquals(listOf("Second", "One"), appState.savedSearches.value.map { it.name })
    }

    // MARK: - Sharing in

    @Test fun `something shared while locked becomes a note on unlock, and not before`() = runBlocking {
        appState.lock()

        appState.receiveShare("Shared text", listOf(
            com.jottery.android.util.AttachmentFiles.Picked("photo.png", "image/png", byteArrayOf(1, 2)),
        ))
        delay(50)
        assertTrue(appState.notes.value.isEmpty())

        appState.unlock(password)

        withTimeout(5_000) { while (appState.notes.value.none { it.content == "Shared text" }) delay(5) }
        withTimeout(5_000) { while (appState.notes.value.first().attachments.isEmpty()) delay(5) }
        assertEquals("photo.png", appState.notes.value.first().attachments.single().filename)
    }

    @Test fun `something shared while unlocked is saved at once, and only once`() = runBlocking {
        appState.receiveShare("Now", emptyList())

        withTimeout(5_000) { while (appState.notes.value.size != 1) delay(5) }
        appState.lock()
        appState.unlock(password)
        withTimeout(5_000) { while (appState.notes.value.isEmpty()) delay(5) }
        assertEquals(1, appState.notes.value.size)
    }

    // MARK: - Leaving an account

    @Test fun `disconnecting keeps the notes and forgets the connection`() = runBlocking {
        val note = appState.createNote("mine")!!
        val db = JotteryDatabase.getInstance(ApplicationProvider.getApplicationContext())
        db.syncMetadataDao().insertOrReplace(
            com.jottery.android.model.SyncMetadata(
                userId = "user", lastSyncAt = "2026-01-01T00:00:00.000Z", pullCursor = "c1",
                cursorAdopted = true, wrappedKeyOpened = true, pendingWrappedKey = "blob",
            )
        )
        appState.updateSettings(appState.settings.value.copy(syncEnabled = true, syncEndpoint = "http://localhost:1"))

        appState.disconnectSync()

        val meta = db.syncMetadataDao().get()!!
        assertEquals(null, meta.userId)
        assertEquals(null, meta.lastSyncAt)
        assertEquals(null, meta.pullCursor)
        assertEquals(null, meta.pendingWrappedKey)
        assertFalse(meta.cursorAdopted)
        assertFalse(meta.wrappedKeyOpened)
        assertFalse(appState.settings.value.syncEnabled)
        assertEquals(null, appState.settings.value.syncEndpoint)
        assertEquals("mine", appState.notes.value.single { it.id == note.id }.content)
    }
}
