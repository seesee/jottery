package com.jottery.android.model

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.Instant

class TagCountsTest {
    private fun note(vararg tags: String) = DecryptedNote(
        id = "id",
        createdAt = Instant.EPOCH,
        modifiedAt = Instant.EPOCH,
        content = "",
        tags = tags.toList(),
        attachments = emptyList(),
        pinned = false,
        archived = false,
        locked = false,
        deleted = false,
        version = 1,
        wordWrap = true,
        syntaxLanguage = "markdown",
        showPreview = false,
        needsSync = false,
    )

    @Test fun `most used first, then by name, whatever the case`() {
        val counts = listOf(note("work", "home"), note("work"), note("Alpha"), note("beta")).tagCounts()

        assertEquals(listOf("work" to 2, "Alpha" to 1, "beta" to 1, "home" to 1), counts)
    }

    @Test fun `a tag repeated on one note counts once`() {
        assertEquals(listOf("work" to 1), listOf(note("work", "work")).tagCounts())
    }

    @Test fun `no notes means no tags`() {
        assertEquals(emptyList<Pair<String, Int>>(), emptyList<DecryptedNote>().tagCounts())
    }
}
