package com.jottery.android.database

import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.testing.SyncHarness
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Rows over the 2 MB CursorWindow can be written; they must also be readable. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class LargeRowTest {
    private lateinit var h: SyncHarness
    private val big = "11111111-1111-4111-8111-111111111111"
    private val small = "22222222-2222-4222-8222-222222222222"
    private val text = "line of a large log file\n".repeat(150_000) // 3.75 MB

    @Before fun setUp() { h = SyncHarness() }
    @After fun tearDown() { h.close() }

    @Test fun `one oversized note doesn't stop the vault loading`() = runBlocking {
        h.seedSynced(small, "small", "h1")
        h.seedSynced(big, text, "h2")

        val result = h.noteRepo.listActiveWithStats(h.key)

        assertEquals(2, result.notes.size)
        assertEquals(0, result.failedCount)
        assertEquals(text, result.notes.single { it.id == big }.content)
    }

    @Test fun `every other read of an oversized note returns all of it`() = runBlocking {
        h.seedSynced(big, text, "h2")
        val stored = h.noteRepo.getRaw(big)!!

        assertEquals(text, h.noteRepo.get(big, h.key)!!.content)
        assertEquals(stored.content, h.noteRepo.listAllRaw().single().content)
        h.noteRepo.insertOrReplace(stored.copy(needsSync = true))
        assertEquals(stored.content, h.noteRepo.listNeedingSync().single().content)
    }

    @Test fun `an oversized version reads back whole`() = runBlocking {
        val record = h.seedSynced(big, text, "h2")
        h.versionRepo.insertOrReplace(NoteVersionRecord.from(record, "sync"))

        assertEquals(record.content, h.versionRepo.getVersions(big).single().content)
    }

    @Test fun `an oversized attachment reads back whole`() = runBlocking {
        val blob = ByteArray(9 * 1024 * 1024 + 3) { (it % 251).toByte() }
        h.attachmentRepo.insertOrReplace(AttachmentRecord("a", "", "image/png", blob.size, blob))

        assertArrayEquals(blob, h.attachmentRepo.getBlob("a"))
    }

    @Test fun `a missing blob is null and an empty one is empty`() = runBlocking {
        assertNull(h.attachmentRepo.getBlob("missing"))

        h.attachmentRepo.insertOrReplace(AttachmentRecord("e", "", "", 0, ByteArray(0)))

        assertEquals(0, h.attachmentRepo.getBlob("e")!!.size)
    }
}
