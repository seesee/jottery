package com.jottery.android.service

import com.jottery.android.model.ConflictInfo
import com.jottery.android.model.ConflictResolutionStrategy
import com.jottery.android.model.SyncAccepted
import com.jottery.android.model.SyncDeletion
import com.jottery.android.model.SyncPushRequest
import com.jottery.android.model.SyncPushResponse
import com.jottery.android.testing.SyncHarness
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
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

/**
 * Runs the real sync cycle against a stub server: pull must never destroy a
 * local edit, whatever order the server's answers arrive in.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class SyncConflictTest {
    private lateinit var h: SyncHarness
    private val noteId = "11111111-1111-4111-8111-111111111111"
    private val parser = Json { ignoreUnknownKeys = true }

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    private suspend fun localText(id: String = noteId): String? =
        h.noteRepo.get(id, h.key)?.content

    private suspend fun editLocally(text: String) {
        val note = h.noteRepo.get(noteId, h.key)!!
        h.noteRepo.update(note.copy(content = text), h.key)
    }

    @Test fun `a rejected note keeps its local text and raises a conflict showing both sides`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL EDIT MADE OFFLINE")

        val remote = h.remoteNote(noteId, "LAPTOP TEXT", "hash-laptop", "hash-base", listOf("hash-laptop", "hash-base"))
        h.onPush = { SyncPushResponse(accepted = emptyList(), rejected = listOf(h.rejection(remote))) }
        h.onPull = { SyncHarness.emptyPull(remote) }

        val conflicts = mutableListOf<ConflictInfo>()
        val service = h.service()
        service.onConflictDetected = { conflicts += it }
        service.fullSync()

        assertEquals("LOCAL EDIT MADE OFFLINE", localText())
        assertTrue("the edit must stay queued", h.noteRepo.getRaw(noteId)!!.needsSync)
        assertEquals(1, conflicts.size)
        assertEquals("LOCAL EDIT MADE OFFLINE", conflicts[0].localContent)
        assertEquals("LAPTOP TEXT", conflicts[0].serverContent)
    }

    @Test fun `an edit saved during the push stays queued and survives the pull echo`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("PUSHED TEXT")
        val pushed = h.noteRepo.getRaw(noteId)!!

        h.onPush = { body ->
            // The autosave lands while the request is in flight
            runBlocking {
                Thread.sleep(5) // modified_at has millisecond resolution
                editLocally("TYPED DURING PUSH")
            }
            val request = parser.decodeFromString<SyncPushRequest>(body)
            SyncPushResponse(accepted = request.notes.map { SyncAccepted(it.id, 2, "2026-02-01T00:00:00.000Z") }, rejected = emptyList())
        }
        val echo = h.remoteNote(
            noteId, "PUSHED TEXT", pushed.contentHash!!, pushed.parentHash,
            listOf(pushed.contentHash!!, "hash-base"),
        )
        h.onPull = { SyncHarness.emptyPull(echo) }

        h.service().fullSync()

        assertEquals("TYPED DURING PUSH", localText())
        assertTrue("the later edit has not been sent", h.noteRepo.getRaw(noteId)!!.needsSync)
    }

    @Test fun `a pull with no rejection still raises a conflict rather than overwrite a pending edit`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")

        // Push is accepted for nothing: the server answers as if it never saw it
        val remote = h.remoteNote(noteId, "REMOTE", "hash-remote", "hash-other", listOf("hash-remote", "hash-other"))
        h.onPull = { SyncHarness.emptyPull(remote) }

        val conflicts = mutableListOf<ConflictInfo>()
        val service = h.service()
        service.onConflictDetected = { conflicts += it }
        service.pull()

        assertEquals("LOCAL", localText())
        assertEquals(listOf("REMOTE"), conflicts.map { it.serverContent })
    }

    @Test fun `a remote deletion does not remove a note with unsynced edits`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")
        h.onPull = {
            SyncHarness.emptyPull().copy(deletions = listOf(SyncDeletion(noteId, "2026-02-01T00:00:00.000Z")))
        }

        h.service().pull()

        assertEquals("LOCAL", localText())
    }

    @Test fun `a remote deletion moves a clean note to the bin, with nothing left to push`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        h.onPull = {
            SyncHarness.emptyPull().copy(deletions = listOf(SyncDeletion(noteId, "2026-02-01T00:00:00.000Z")))
        }

        h.service().pull()

        val note = h.noteRepo.getRaw(noteId)!!
        assertTrue(note.deleted)
        assertFalse(note.needsSync)
        assertNotNull(note.localDeletedAt)
        assertEquals("BASE", localText())
    }

    @Test fun `a clean note fast-forwards to the remote version`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        val remote = h.remoteNote(noteId, "NEWER", "hash-new", "hash-base", listOf("hash-new", "hash-base"))
        h.onPull = { SyncHarness.emptyPull(remote) }

        h.service().pull()

        assertEquals("NEWER", localText())
        assertFalse(h.noteRepo.getRaw(noteId)!!.needsSync)
    }

    private suspend fun raiseConflict(remote: com.jottery.android.model.SyncNote): ConflictInfo {
        h.onPush = { SyncPushResponse(accepted = emptyList(), rejected = listOf(h.rejection(remote))) }
        h.onPull = { SyncHarness.emptyPull(remote) }
        h.service().fullSync()
        return h.service().loadConflicts().single()
    }

    @Test fun `a conflict survives a new session and no plaintext reaches the database`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")
        val remote = h.remoteNote(noteId, "SERVER SECRET", "hash-r", "hash-base", listOf("hash-r", "hash-base"))
        raiseConflict(remote)

        // A fresh service is what a later unlock gets
        val reloaded = h.service().loadConflicts().single()

        assertEquals("LOCAL", reloaded.localContent)
        assertEquals("SERVER SECRET", reloaded.serverContent)
        val row = h.db.noteSyncMetadataDao().getByNoteId(noteId)!!
        assertFalse(row.conflictData!!.contains("SERVER SECRET"))
    }

    @Test fun `a note in conflict is not pushed again until it is resolved`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")
        raiseConflict(h.remoteNote(noteId, "R", "hash-r", "hash-base", listOf("hash-r", "hash-base")))
        val pushes = h.pushBodies.size

        h.service().fullSync()

        assertEquals(pushes, h.pushBodies.size)
    }

    @Test fun `keep local rebases with the newest hashes first`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")
        val localHash = h.noteRepo.getRaw(noteId)!!.contentHash!!
        val conflict = raiseConflict(
            h.remoteNote(noteId, "R", "hash-r", "hash-base", listOf("hash-r", "hash-base"))
        )

        h.service().resolveConflict(conflict, ConflictResolutionStrategy.KEEP_LOCAL)

        val row = h.noteRepo.getRaw(noteId)!!
        val chain = parser.decodeFromString<List<String>>(row.hashChain!!)
        assertEquals(listOf(localHash, "hash-r", "hash-base"), chain)
        assertEquals("hash-r", row.parentHash)
        assertTrue(row.needsSync)
        assertEquals("LOCAL", localText())
        assertTrue(h.service().loadConflicts().isEmpty())
    }

    @Test fun `keep server adopts every server field and snapshots the local text`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")
        val rejected = h.rejection(
            h.remoteNote(noteId, "R", "hash-r", "hash-base", listOf("hash-r", "hash-base"))
        ).copy(serverArchived = true, serverLocked = true, serverColor = "red")
        h.onPush = { SyncPushResponse(accepted = emptyList(), rejected = listOf(rejected)) }
        h.service().fullSync()
        val conflict = h.service().loadConflicts().single()

        h.service().resolveConflict(conflict, ConflictResolutionStrategy.KEEP_SERVER)

        val row = h.noteRepo.getRaw(noteId)!!
        assertTrue(row.archived)
        assertTrue(row.locked)
        assertEquals("red", row.color)
        assertFalse(row.needsSync)
        assertEquals("R", localText())
        val snapshot = h.versionRepo.getVersions(noteId).first { it.reason == "pre-conflict-resolution" }
        assertEquals(
            "LOCAL",
            com.jottery.android.crypto.CryptoService.decryptText(
                com.jottery.android.crypto.CryptoService.parseEncryptedJSON(snapshot.content), h.key,
            ),
        )
    }

    @Test fun `keep both keeps the local note's language and colour on the copy`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        h.noteRepo.update(
            h.noteRepo.get(noteId, h.key)!!.copy(content = "LOCAL", syntaxLanguage = "python", color = "blue"),
            h.key,
        )
        val conflict = raiseConflict(
            h.remoteNote(noteId, "R", "hash-r", "hash-base", listOf("hash-r", "hash-base"))
        )

        h.service().resolveConflict(conflict, ConflictResolutionStrategy.KEEP_BOTH)

        val copy = h.noteRepo.listAllRaw().single { it.id != noteId }
        assertEquals("python", copy.syntaxLanguage)
        assertEquals("blue", copy.color)
        assertEquals("LOCAL", localText(copy.id))
        assertEquals("R", localText())
    }

    @Test fun `an undecryptable server version is never shown raw and can't replace the note`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")
        val foreign = javax.crypto.spec.SecretKeySpec(ByteArray(32) { 9 }, "AES")
        val raw = com.jottery.android.crypto.CryptoService.serializeEncryptedJSON(
            com.jottery.android.crypto.CryptoService.encryptText("FOREIGN", foreign)
        )
        val remote = h.remoteNote(noteId, "x", "hash-r", "hash-base", listOf("hash-r")).copy(content = raw)
        val conflict = raiseConflict(remote)

        assertNull(conflict.serverContent)
        val failed = runCatching {
            h.service().resolveConflict(conflict, ConflictResolutionStrategy.KEEP_SERVER)
        }
        assertTrue(failed.isFailure)
        assertEquals("LOCAL", localText())
        assertEquals(1, h.service().loadConflicts().size)
    }

    @Test fun `resolving uses the server's latest version, not the one first shown`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")
        val shown = raiseConflict(h.remoteNote(noteId, "R1", "hash-r1", "hash-base", listOf("hash-r1", "hash-base")))
        h.onPull = {
            SyncHarness.emptyPull(
                h.remoteNote(noteId, "R2", "hash-r2", "hash-r1", listOf("hash-r2", "hash-r1", "hash-base"))
            )
        }
        h.service().pull()

        h.service().resolveConflict(shown, ConflictResolutionStrategy.KEEP_SERVER)

        assertEquals("R2", localText())
        assertEquals("hash-r2", h.noteRepo.getRaw(noteId)!!.contentHash)
    }

    @Test fun `keep both copies the local text as it is now`() = runBlocking {
        h.seedSynced(noteId, "BASE", "hash-base")
        editLocally("LOCAL")
        val shown = raiseConflict(h.remoteNote(noteId, "R", "hash-r", "hash-base", listOf("hash-r", "hash-base")))
        editLocally("LOCAL, EDITED SINCE")

        h.service().resolveConflict(shown, ConflictResolutionStrategy.KEEP_BOTH)

        val copy = h.noteRepo.listAllRaw().single { it.id != noteId }
        assertEquals("LOCAL, EDITED SINCE", localText(copy.id))
    }

    @Test fun `a new remote note is inserted`() = runBlocking {
        val remote = h.remoteNote(noteId, "NEW", "hash-new", null, listOf("hash-new"))
        h.onPull = { SyncHarness.emptyPull(remote) }

        assertEquals(1, h.service().pull())

        assertNotNull(h.noteRepo.getRaw(noteId))
        assertEquals("NEW", localText())
    }
}
