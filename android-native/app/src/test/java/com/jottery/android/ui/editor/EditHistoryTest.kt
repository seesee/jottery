package com.jottery.android.ui.editor

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class EditHistoryTest {
    private var now = 10_000L
    private val history = EditHistory(TextFieldValue("start")) { now }

    private fun type(text: String) = history.record(TextFieldValue(text))

    @Test fun `a burst of typing undoes in one step`() {
        type("start a")
        now += 100
        type("start ab")
        now += 100
        type("start abc")

        assertEquals("start", history.undo()?.text)
        assertFalse(history.canUndo)
    }

    @Test fun `a pause starts a new step`() {
        type("one")
        now += 5_000
        type("one two")

        assertEquals("one", history.undo()?.text)
        assertEquals("start", history.undo()?.text)
    }

    @Test fun `redo returns what undo took, and a new edit clears it`() {
        type("edited")
        history.undo()
        assertEquals("edited", history.redo()?.text)

        history.undo()
        now += 5_000
        type("different")

        assertFalse(history.canRedo)
        assertNull(history.redo())
    }

    @Test fun `moving the caret isn't an edit, but undo goes back to where it was`() {
        history.record(TextFieldValue("start", TextRange(2)))
        assertFalse(history.canUndo)

        now += 5_000
        type("start!")

        assertEquals(TextRange(2), history.undo()?.selection)
    }

    @Test fun `text from elsewhere ends the history`() {
        type("edited")

        history.reset(TextFieldValue("from another device"))

        assertFalse(history.canUndo)
        assertFalse(history.canRedo)
    }

    @Test fun `old steps fall away`() {
        repeat(150) {
            now += 5_000
            type("edit $it")
        }
        var steps = 0
        while (history.undo() != null) steps++

        assertEquals(100, steps)
        assertTrue(steps < 150)
    }
}
