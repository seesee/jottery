package com.jottery.android.ui.editor

import androidx.compose.ui.text.input.TextFieldValue

/**
 * Undo and redo for one editor session. A burst of typing is one step, so undo
 * doesn't take a character at a time. Nothing outlives the session: the history is
 * decrypted text.
 */
class EditHistory(
    initial: TextFieldValue,
    private val clock: () -> Long = System::currentTimeMillis,
) {
    private val undoStack = ArrayDeque<TextFieldValue>()
    private val redoStack = ArrayDeque<TextFieldValue>()
    private var current = initial
    private var lastEditAt = 0L

    val canUndo: Boolean get() = undoStack.isNotEmpty()
    val canRedo: Boolean get() = redoStack.isNotEmpty()

    /** The editor's value changed. A move of the caret alone isn't an edit. */
    fun record(value: TextFieldValue) {
        if (value.text == current.text) {
            current = value
            return
        }
        val now = clock()
        if (undoStack.isEmpty() || now - lastEditAt > BURST_MILLIS) {
            undoStack.addLast(current)
            if (undoStack.size > MAX_STEPS) undoStack.removeFirst()
        }
        lastEditAt = now
        current = value
        redoStack.clear()
    }

    fun undo(): TextFieldValue? {
        val previous = undoStack.removeLastOrNull() ?: return null
        redoStack.addLast(current)
        current = previous
        lastEditAt = 0
        return previous
    }

    fun redo(): TextFieldValue? {
        val next = redoStack.removeLastOrNull() ?: return null
        undoStack.addLast(current)
        current = next
        lastEditAt = 0
        return next
    }

    /** The text changed from outside, so older steps no longer apply to it. */
    fun reset(value: TextFieldValue) {
        undoStack.clear()
        redoStack.clear()
        current = value
        lastEditAt = 0
    }

    private companion object {
        const val BURST_MILLIS = 1000L
        const val MAX_STEPS = 100
    }
}
