package com.jottery.android.util

/** What to offer while a search is being typed. */
object SearchSuggestions {
    const val MAX_RECENT = 8
    private const val MAX_TAGS = 8

    /** The word being typed, if it's a tag: what follows the last # in the last word. */
    private fun tagBeingTyped(query: String): String? {
        val word = query.substringAfterLast(' ')
        return if (word.startsWith("#")) word.drop(1) else null
    }

    /** Tags that would finish what's being typed, leaving out one already written in full. */
    fun tagCompletions(query: String, tags: List<String>): List<String> {
        val typed = tagBeingTyped(query) ?: return emptyList()
        return tags.filter { it.startsWith(typed, ignoreCase = true) && !it.equals(typed, ignoreCase = true) }
            .take(MAX_TAGS)
    }

    /** The query with the tag being typed replaced, ready for the next word. */
    fun complete(query: String, tag: String): String =
        query.substringBeforeLast(' ', "").let { start -> if (start.isEmpty()) "#$tag " else "$start #$tag " }

    /** [query] at the front of [recent], once, and only so many kept. */
    fun remember(recent: List<String>, query: String): List<String> {
        val trimmed = query.trim()
        if (trimmed.isEmpty()) return recent
        return (listOf(trimmed) + recent.filterNot { it == trimmed }).take(MAX_RECENT)
    }
}
