package com.jottery.android.ui.editor

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import java.util.UUID

/** The web's default commands, in its formats, so a stamp reads the same on either. */
enum class QuickCommand(val trigger: String) {
    NOW("/now"),
    DATE("/date"),
    TIME("/time"),
    UUID_V4("/uuid");

    fun generate(now: LocalDateTime = LocalDateTime.now()): String = when (this) {
        NOW -> now.format(DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss"))
        DATE -> now.format(DateTimeFormatter.ofPattern("yyyy-MM-dd"))
        TIME -> now.format(DateTimeFormatter.ofPattern("HH:mm:ss"))
        UUID_V4 -> UUID.randomUUID().toString()
    }

    companion object {
        /**
         * Expand a command just finished with a space or a new line, or null if that
         * isn't what this edit did. It has to stand alone, so "and/or now" stays as it is.
         */
        fun expand(before: TextFieldValue, after: TextFieldValue, now: LocalDateTime = LocalDateTime.now()): TextFieldValue? {
            val caret = after.selection.max
            if (after.text.length != before.text.length + 1 || !after.selection.collapsed || caret == 0) return null
            val typed = after.text[caret - 1]
            if (typed != ' ' && typed != '\n') return null

            val wordEnd = caret - 1
            val wordStart = after.text.substring(0, wordEnd).indexOfLast { it.isWhitespace() } + 1
            val word = after.text.substring(wordStart, wordEnd)
            val command = entries.firstOrNull { it.trigger.equals(word, ignoreCase = true) } ?: return null

            val content = command.generate(now)
            return TextFieldValue(
                text = after.text.replaceRange(wordStart, wordEnd, content),
                selection = TextRange(wordStart + content.length + 1),
            )
        }
    }
}
