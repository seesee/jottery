package com.jottery.android.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class NoteCategoriesTest {
    private val settings = UserSettings()

    @Test fun `names and tag colours survive being stored`() {
        val stored = NoteCategories().named("red", "Urgent").withTag("Work", "blue").appliedTo(settings)

        val read = NoteCategories.from(stored)

        assertEquals("Urgent", read.names["red"])
        assertEquals("blue", read.colourOfTag("work"))
        assertEquals("blue", read.colourOfTag(" WORK "))
    }

    @Test fun `clearing a name or a tag's colour removes it`() {
        val categories = NoteCategories().named("red", "Urgent").withTag("work", "blue")

        val cleared = categories.named("red", "  ").withTag("work", null)

        assertEquals(emptyMap<String, String>(), cleared.names)
        assertNull(cleared.colourOfTag("work"))
    }

    @Test fun `nothing set stores nothing`() {
        val stored = NoteCategories().appliedTo(settings)

        assertNull(stored.colorPalette)
        assertNull(stored.tagColors)
    }

    @Test fun `a palette the web wrote still gives up its names`() {
        val web = settings.copy(colorPalette = """{"red":{"light":"#FFE5E5","dark":"#5C1A1A","displayName":"Urgent"},"blue":{"light":"#E5F0FF","dark":"#1A3A5C"}}""")

        assertEquals(mapOf("red" to "Urgent"), NoteCategories.from(web).names)
    }

    @Test fun `what can't be read costs a label, not the app`() {
        val broken = settings.copy(colorPalette = "not json", tagColors = "[1,2]")

        assertEquals(NoteCategories(), NoteCategories.from(broken))
    }

    @Test fun `other settings are left alone`() {
        val stored = NoteCategories().named("red", "Urgent").appliedTo(settings.copy(syncEndpoint = "https://example.test"))

        assertEquals("https://example.test", stored.syncEndpoint)
    }
}
