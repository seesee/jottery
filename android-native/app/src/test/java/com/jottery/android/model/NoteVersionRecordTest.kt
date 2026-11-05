package com.jottery.android.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class NoteVersionRecordTest {
    private fun note(hash: String?, version: Int = 5) = NoteRecord(
        id = "1b4e28ba-2fa1-11d2-883f-b9a761bde3fb",
        createdAt = "2026-01-01T00:00:00.000Z",
        modifiedAt = "2026-01-01T00:00:00.000Z",
        content = "c",
        tags = "t",
        version = version,
        contentHash = hash,
    )

    private fun key(hash: String?, version: Int = 5) = NoteVersionRecord.from(note(hash, version), "sync").versionKey

    @Test fun `two devices' different text at one version number can't collide`() {
        assertNotEquals(key("aaaaaaaaaaaaaaaa0000"), key("bbbbbbbbbbbbbbbb0000"))
    }

    @Test fun `the same text at the same version is one version, wherever it was made`() {
        assertEquals(key("aaaaaaaaaaaaaaaa0000"), key("aaaaaaaaaaaaaaaa0000"))
    }

    @Test fun `a note with no hash still gets a key other clients can store`() {
        assertEquals("1b4e28ba-2fa1-11d2-883f-b9a761bde3fb:5", key(null))
    }

    @Test fun `a safety snapshot never takes a real version's key`() {
        val real = NoteVersionRecord.from(note("aaaaaaaaaaaaaaaa"), "sync").versionKey
        assertNotEquals(real, NoteVersionRecord.safetySnapshot(note("aaaaaaaaaaaaaaaa"), "pre-sync-overwrite").versionKey)
    }
}
