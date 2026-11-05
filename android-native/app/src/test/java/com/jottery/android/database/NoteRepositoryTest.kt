package com.jottery.android.database

import com.jottery.android.crypto.CryptoService
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.DecryptedNote
import com.jottery.android.testing.SyncHarness
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class NoteRepositoryTest {
    private lateinit var h: SyncHarness
    private val id = "55555555-5555-4555-8555-555555555555"

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    @Test fun `a save from an old copy does not undo a delete`() = runBlocking {
        h.seedSynced(id, "BASE", "h0")
        val stale = h.noteRepo.get(id, h.key)!!
        h.noteRepo.softDelete(id)

        val saved = h.noteRepo.update(stale.copy(content = "typed"), h.key)

        assertFalse(saved)
        assertTrue(h.noteRepo.getRaw(id)!!.deleted)
    }

    @Test fun `a save from an old copy keeps archive, lock and pin`() = runBlocking {
        h.seedSynced(id, "BASE", "h0")
        val stale = h.noteRepo.get(id, h.key)!!
        h.noteRepo.archive(id)
        h.noteRepo.togglePin(id)
        h.noteRepo.toggleLock(id)

        h.noteRepo.update(stale.copy(content = "typed"), h.key)

        val row = h.noteRepo.getRaw(id)!!
        assertTrue(row.archived)
        assertTrue(row.pinned)
        assertTrue(row.locked)
        assertEquals("typed", h.noteRepo.get(id, h.key)!!.content)
    }

    @Test fun `an edit leaves attachment filenames encrypted`() = runBlocking {
        val filename = h.encrypt("passport-scan.jpg")
        val ref = AttachmentRef("a1", filename, "image/jpeg", 10, "a1")
        h.noteRepo.insertOrReplace(
            h.seedSynced(id, "BASE", "h0").copy(attachments = Json.encodeToString(listOf(ref)))
        )
        val note = h.noteRepo.get(id, h.key)!!
        assertEquals("passport-scan.jpg", note.attachments.single().filename)

        h.noteRepo.update(note.copy(content = "edited"), h.key)

        val stored = Json.decodeFromString<List<AttachmentRef>>(h.noteRepo.getRaw(id)!!.attachments).single()
        assertFalse(stored.filename.contains("passport"))
        assertEquals(
            "passport-scan.jpg",
            CryptoService.decryptText(CryptoService.parseEncryptedJSON(stored.filename), h.key),
        )
    }

    @Test fun `a save from an old copy keeps attachments added elsewhere`() = runBlocking {
        h.seedSynced(id, "BASE", "h0")
        val stale = h.noteRepo.get(id, h.key)!!
        h.noteRepo.addAttachment(id, AttachmentRef("a1", h.encrypt("new.png"), "image/png", 1, "a1"), h.key)

        h.noteRepo.update(stale.copy(content = "typed"), h.key)

        assertEquals(listOf("a1"), h.noteRepo.get(id, h.key)!!.attachments.map { it.id })
    }

    @Test fun `every text edit advances the hash and the chain`() = runBlocking {
        h.seedSynced(id, "BASE", "h0")
        h.noteRepo.update(h.noteRepo.get(id, h.key)!!.copy(content = "one"), h.key)
        val row = h.noteRepo.getRaw(id)!!

        assertEquals("h0", row.parentHash)
        assertEquals(listOf(row.contentHash, "h0"), Json.decodeFromString<List<String>>(row.hashChain!!))
        assertTrue(row.needsSync)
    }

    @Test fun `plaintext filenames left by older builds are encrypted and queued`() = runBlocking {
        val ref = AttachmentRef("a1", "passport-scan.jpg", "image/jpeg", 10, "a1")
        h.noteRepo.insertOrReplace(
            h.seedSynced(id, "BASE", "h0").copy(attachments = Json.encodeToString(listOf(ref)))
        )

        assertEquals(1, h.noteRepo.repairPlaintextFilenames(h.key))

        val row = h.noteRepo.getRaw(id)!!
        assertFalse(row.attachments.contains("passport"))
        assertTrue("the server's copy must be replaced too", row.needsSync)
        assertEquals("passport-scan.jpg", h.noteRepo.get(id, h.key)!!.attachments.single().filename)
        assertEquals("a second pass finds nothing", 0, h.noteRepo.repairPlaintextFilenames(h.key))
    }

    @Test fun `an unchanged note isn't decrypted again, and a changed one is`() = runBlocking {
        h.seedSynced(id, "BASE", "h0")
        val first = h.noteRepo.listActiveWithStats(h.key).notes.single()

        val again = h.noteRepo.listActiveWithStats(h.key, mapOf(id to first)).notes.single()
        assertTrue("the same object means no second decryption", again === first)

        h.noteRepo.update(first.copy(content = "changed"), h.key)
        val changed = h.noteRepo.listActiveWithStats(h.key, mapOf(id to first)).notes.single()
        assertEquals("changed", changed.content)

        h.noteRepo.togglePin(id)
        val pinned = h.noteRepo.listActiveWithStats(h.key, mapOf(id to changed)).notes.single()
        assertTrue(pinned.pinned)
    }

    private fun note(pinned: Boolean = false, locked: Boolean = false, attachments: List<AttachmentRef> = emptyList()) =
        DecryptedNote(
            id = "a", createdAt = java.time.Instant.EPOCH, modifiedAt = java.time.Instant.EPOCH,
            content = "c", tags = emptyList(), attachments = attachments, pinned = pinned, archived = false,
            locked = locked, deleted = false, version = 1, wordWrap = true, syntaxLanguage = "markdown",
            showPreview = false, needsSync = false,
        )

    @Test fun `a list that only changes a pin or lock is still published`() {
        val flow = MutableStateFlow(listOf(note()))

        flow.value = listOf(note(pinned = true, locked = true))

        assertTrue(flow.value[0].pinned)
        assertTrue(flow.value[0].locked)
    }
}
