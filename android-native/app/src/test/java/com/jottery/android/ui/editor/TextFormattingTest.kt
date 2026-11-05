package com.jottery.android.ui.editor

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDateTime

class TextFormattingTest {
    private fun value(text: String, start: Int, end: Int = start) = TextFieldValue(text, TextRange(start, end))

    @Test fun `wrapping marks the selection and keeps it selected`() {
        val result = TextFormatting.wrap(value("a word here", 2, 6), "**")

        assertEquals("a **word** here", result.text)
        assertEquals(TextRange(4, 8), result.selection)
    }

    @Test fun `wrapping with nothing selected leaves the caret ready to type`() {
        val result = TextFormatting.wrap(value("ab", 1), "*")

        assertEquals("a**b", result.text)
        assertEquals(TextRange(2), result.selection)
    }

    @Test fun `wrapping again takes the marks away`() {
        val once = TextFormatting.wrap(value("a word here", 2, 6), "**")

        val twice = TextFormatting.wrap(once, "**")

        assertEquals("a word here", twice.text)
        assertEquals(TextRange(2, 6), twice.selection)
    }

    @Test fun `a list prefix goes on the line the caret is in`() {
        val result = TextFormatting.prefixLines(value("one\ntwo\nthree", 5), "- ")

        assertEquals("one\n- two\nthree", result.text)
        assertEquals(TextRange(7), result.selection)
    }

    @Test fun `a list prefix goes on every selected line`() {
        val result = TextFormatting.prefixLines(value("one\ntwo\nthree", 1, 6), "- ")

        assertEquals("- one\n- two\nthree", result.text)
    }

    @Test fun `the first line of a note works too, even when it's empty`() {
        assertEquals("- one", TextFormatting.prefixLines(value("one", 0), "- ").text)
        assertEquals("- ", TextFormatting.prefixLines(value("", 0), "- ").text)
    }

    @Test fun `a caret at the end of a line stays with that line`() {
        assertEquals("one\n- two", TextFormatting.prefixLines(value("one\ntwo", 7), "- ").text)
    }

    @Test fun `prefixing lines that all have it takes it off`() {
        assertEquals("one\ntwo", TextFormatting.prefixLines(value("- one\n- two", 0, 11), "- ").text)
    }

    @Test fun `a link around a selection selects the address`() {
        val result = TextFormatting.link(value("see this page", 4, 8))

        assertEquals("see [this](https://) page", result.text)
        assertEquals("https://", result.text.substring(result.selection.min, result.selection.max))
    }

    @Test fun `a link with no text waits for it`() {
        val result = TextFormatting.link(value("", 0))

        assertEquals("[](https://)", result.text)
        assertEquals(TextRange(1), result.selection)
    }

    @Test fun `inserting replaces the selection`() {
        val result = TextFormatting.insert(value("a old b", 2, 5), "new")

        assertEquals("a new b", result.text)
        assertEquals(TextRange(5), result.selection)
    }

    // Quick commands

    private val time = LocalDateTime.of(2026, 3, 9, 7, 5, 2)

    private fun typed(before: String, char: Char) =
        QuickCommand.expand(value(before, before.length), value(before + char, before.length + 1), time)

    @Test fun `the formats are the web's`() {
        assertEquals("2026-03-09 07:05:02", QuickCommand.NOW.generate(time))
        assertEquals("2026-03-09", QuickCommand.DATE.generate(time))
        assertEquals("07:05:02", QuickCommand.TIME.generate(time))
        assertTrue(Regex("[0-9a-f-]{36}").matches(QuickCommand.UUID_V4.generate(time)))
    }

    @Test fun `a command expands when a space or new line finishes it`() {
        assertEquals("Stamp 2026-03-09 ", typed("Stamp /date", ' ')!!.text)
        assertEquals("07:05:02\n", typed("/time", '\n')!!.text)
    }

    @Test fun `the caret ends up after what was typed`() {
        val result = typed("/date", ' ')!!

        assertEquals(TextRange(result.text.length), result.selection)
    }

    @Test fun `anything else is left alone`() {
        assertNull("not a command", typed("/nope", ' '))
        assertNull("not on its own", typed("and/now", ' '))
        assertNull("not finished", typed("/now", 'x'))
    }

    @Test fun `only a keystroke counts, not a paste`() {
        assertNull(QuickCommand.expand(value("", 0), value("/now ", 5), time))
    }
}
