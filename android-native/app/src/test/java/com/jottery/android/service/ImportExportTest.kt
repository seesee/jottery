package com.jottery.android.service

import androidx.room.withTransaction
import com.jottery.android.crypto.CryptoService
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.testing.SyncHarness
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.ByteArrayOutputStream
import java.util.Base64

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class ImportExportTest {
    private lateinit var h: SyncHarness
    private val id = "66666666-6666-4666-8666-666666666666"

    private val file = """{"version":"1.0","exportDate":"2026-01-01T00:00:00.000Z","notes":[{
        "id":"$id","createdAt":"2025-03-01T10:00:00.000Z","modifiedAt":"2025-03-02T10:00:00.000Z",
        "content":"From the web","tags":["a","b"],"pinned":true,"archived":true,"locked":true,
        "wordWrap":false,"syntaxLanguage":"python","showPreview":true,"color":"red",
        "attachments":[{"filename":"secret-name.png","mimeType":"image/png",
          "data":"${Base64.getEncoder().encodeToString("pixels".toByteArray())}"}]}]}"""

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    private fun importer() = ImportService(h.noteRepo, h.attachmentRepo) { block -> h.db.withTransaction { block() } }

    @Test fun `import keeps attachments, dates, flags, language and colour`() = runBlocking {
        val result = importer().import(file, h.key, ImportStrategy.SKIP)

        assertEquals(1, result.imported)
        assertEquals(1, result.attachments)
        val note = h.noteRepo.get(id, h.key)!!
        assertEquals("From the web", note.content)
        assertEquals(listOf("a", "b"), note.tags)
        assertEquals("2025-03-01T10:00:00Z", note.createdAt.toString())
        assertEquals("2025-03-02T10:00:00Z", note.modifiedAt.toString())
        assertTrue(note.pinned)
        assertTrue(note.archived)
        assertTrue(note.locked)
        assertFalse(note.wordWrap)
        assertEquals("python", note.syntaxLanguage)
        assertEquals("red", note.color)

        val ref = note.attachments.single()
        assertEquals("secret-name.png", ref.filename)
        val blob = String(h.attachmentRepo.getBlob(ref.data)!!)
        assertEquals("pixels", String(CryptoService.decrypt(CryptoService.parseEncryptedJSON(blob), h.key)))
    }

    @Test fun `no filename reaches the database in plaintext`() = runBlocking {
        importer().import(file, h.key, ImportStrategy.SKIP)

        assertFalse(h.noteRepo.getRaw(id)!!.attachments.contains("secret-name"))
        val filenames = h.db.query("SELECT filename FROM attachments", null).use { cursor ->
            generateSequence { if (cursor.moveToNext()) cursor.getString(0) else null }.toList()
        }
        assertEquals(listOf(""), filenames)
    }

    @Test fun `skip leaves a note already here alone`() = runBlocking {
        h.seedSynced(id, "Mine", "h")

        val result = importer().import(file, h.key, ImportStrategy.SKIP)

        assertEquals(1, result.skipped)
        assertEquals(0, result.imported)
        assertEquals("Mine", h.noteRepo.get(id, h.key)!!.content)
    }

    @Test fun `importing twice doesn't duplicate`() = runBlocking {
        importer().import(file, h.key, ImportStrategy.SKIP)
        importer().import(file, h.key, ImportStrategy.SKIP)

        assertEquals(1, h.noteRepo.listAllRaw().size)
    }

    @Test fun `replace takes the imported copy`() = runBlocking {
        h.seedSynced(id, "Mine", "h")

        importer().import(file, h.key, ImportStrategy.REPLACE)

        assertEquals("From the web", h.noteRepo.get(id, h.key)!!.content)
    }

    @Test fun `keep both gives the copy a new id`() = runBlocking {
        h.seedSynced(id, "Mine", "h")

        importer().import(file, h.key, ImportStrategy.KEEP_BOTH)

        val all = h.noteRepo.listAllRaw()
        assertEquals(2, all.size)
        assertEquals("Mine", h.noteRepo.get(id, h.key)!!.content)
        assertNotEquals(id, all.first { it.id != id }.id)
    }

    @Test fun `a bad attachment is reported and the note still lands`() = runBlocking {
        val broken = file.replace(Regex("\"data\":\"[^\"]+\""), "\"data\":\"***not base64***\"")

        val result = importer().import(broken, h.key, ImportStrategy.SKIP)

        assertEquals(1, result.imported)
        assertEquals(1, result.attachmentsFailed)
        assertTrue(h.noteRepo.get(id, h.key)!!.attachments.isEmpty())
    }

    @Test fun `an export imports back with nothing lost`() = runBlocking {
        val blob = CryptoService.serializeEncryptedJSON(CryptoService.encrypt("file".toByteArray(), h.key)).toByteArray()
        h.attachmentRepo.insertOrReplace(AttachmentRecord("blob", "", "text/plain", blob.size, blob))
        val ref = AttachmentRef("blob", h.encrypt("f.txt"), "text/plain", 4, "blob")
        h.noteRepo.insertOrReplace(
            h.seedSynced(id, "Text", "h").copy(attachments = Json.encodeToString(listOf(ref)), color = "blue", archived = true)
        )
        val note = h.noteRepo.get(id, h.key)!!

        val out = ByteArrayOutputStream()
        val exported = ExportService(h.attachmentRepo).export(listOf(note), h.key, out)
        assertEquals(0, exported.skippedAttachments)

        val fresh = SyncHarness()
        val importer = ImportService(fresh.noteRepo, fresh.attachmentRepo) { block -> fresh.db.withTransaction { block() } }
        importer.import(out.toString(), fresh.key, ImportStrategy.SKIP)

        val copy = fresh.noteRepo.get(id, fresh.key)!!
        assertEquals("Text", copy.content)
        assertEquals("blue", copy.color)
        assertTrue(copy.archived)
        assertEquals("f.txt", copy.attachments.single().filename)
        fresh.close()
    }

    @Test fun `an attachment that can't be read is counted, not hidden`() = runBlocking {
        val ref = AttachmentRef("missing", h.encrypt("gone.txt"), "text/plain", 4, "missing")
        h.noteRepo.insertOrReplace(h.seedSynced(id, "Text", "h").copy(attachments = Json.encodeToString(listOf(ref))))

        val result = ExportService(h.attachmentRepo).export(
            listOf(h.noteRepo.get(id, h.key)!!), h.key, ByteArrayOutputStream(),
        )

        assertEquals(1, result.skippedAttachments)
    }
}
