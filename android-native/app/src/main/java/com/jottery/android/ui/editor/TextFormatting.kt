package com.jottery.android.ui.editor

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue

/** Markdown edits, as pure changes to the text and selection. */
object TextFormatting {
    /** Replace the selection with [text], leaving the caret after it. */
    fun insert(value: TextFieldValue, text: String): TextFieldValue {
        val start = value.selection.min
        val end = value.selection.max
        return TextFieldValue(
            text = value.text.replaceRange(start, end, text),
            selection = TextRange(start + text.length),
        )
    }

    /**
     * Put [marker] either side of the selection, or take it away if it's already
     * there. With nothing selected the caret lands between the two, ready to type.
     */
    fun wrap(value: TextFieldValue, marker: String): TextFieldValue {
        val start = value.selection.min
        val end = value.selection.max
        val text = value.text
        val wrapped = start >= marker.length &&
            end + marker.length <= text.length &&
            text.substring(start - marker.length, start) == marker &&
            text.substring(end, end + marker.length) == marker
        if (wrapped) {
            return TextFieldValue(
                text = text.removeRange(end, end + marker.length).removeRange(start - marker.length, start),
                selection = TextRange(start - marker.length, end - marker.length),
            )
        }
        return TextFieldValue(
            text = text.replaceRange(start, end, marker + text.substring(start, end) + marker),
            selection = TextRange(start + marker.length, end + marker.length),
        )
    }

    /** Start each selected line with [prefix], or take it off if they all have it. */
    fun prefixLines(value: TextFieldValue, prefix: String): TextFieldValue {
        val text = value.text
        val first = text.lastIndexOf('\n', (value.selection.min - 1).coerceAtLeast(0))
            .let { if (value.selection.min == 0) 0 else it + 1 }
        val last = text.indexOf('\n', value.selection.max).let { if (it == -1) text.length else it }
        val lines = text.substring(first, last).split('\n')
        val removing = lines.all { it.startsWith(prefix) }
        val changed = lines.joinToString("\n") { if (removing) it.removePrefix(prefix) else prefix + it }
        val change = changed.length - (last - first)
        val caret = if (removing) (value.selection.max + change).coerceAtLeast(first) else value.selection.max + change
        return TextFieldValue(text = text.replaceRange(first, last, changed), selection = TextRange(caret))
    }

    /** Make the selection a link, with the address selected to type over. */
    fun link(value: TextFieldValue): TextFieldValue {
        val start = value.selection.min
        val end = value.selection.max
        val label = value.text.substring(start, end)
        val address = "https://"
        val linked = "[$label]($address)"
        // With no label yet, that's what comes next
        val selection = if (label.isEmpty()) {
            TextRange(start + 1)
        } else {
            TextRange(start + label.length + 3, start + label.length + 3 + address.length)
        }
        return TextFieldValue(text = value.text.replaceRange(start, end, linked), selection = selection)
    }
}
