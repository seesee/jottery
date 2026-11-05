package com.jottery.android.ui.editor

/** Find and replace in one note. Matching ignores case, as web's editor does. */
object EditorSearch {

    /** Every place [query] occurs, not overlapping. */
    fun matches(text: String, query: String): List<IntRange> {
        if (query.isEmpty()) return emptyList()
        val found = mutableListOf<IntRange>()
        var from = 0
        while (true) {
            val at = text.indexOf(query, from, ignoreCase = true)
            if (at < 0) return found
            found += at until at + query.length
            from = at + query.length
        }
    }

    /** The match after the caret, going round to the first. */
    fun next(matches: List<IntRange>, caret: Int): Int =
        matches.indexOfFirst { it.first >= caret }.let { if (it < 0) 0 else it }

    /** The match before the caret, going round to the last. */
    fun previous(matches: List<IntRange>, caret: Int): Int =
        matches.indexOfLast { it.last < caret }.let { if (it < 0) matches.lastIndex else it }

    fun replace(text: String, match: IntRange, replacement: String): String =
        text.replaceRange(match, replacement)

    /** Later matches first, so earlier positions stay right. */
    fun replaceAll(text: String, query: String, replacement: String): String =
        matches(text, query).asReversed().fold(text) { result, match -> result.replaceRange(match, replacement) }
}
