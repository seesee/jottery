package com.jottery.android.util

import org.junit.Assert.assertEquals
import org.junit.Test

class SearchSuggestionsTest {
    private val tags = listOf("work", "Worship", "home")

    @Test fun `a tag being typed is finished, whatever the case`() {
        assertEquals(listOf("work", "Worship"), SearchSuggestions.tagCompletions("#wo", tags))
        assertEquals(listOf("work", "Worship"), SearchSuggestions.tagCompletions("dog #WO", tags))
    }

    @Test fun `a bare hash offers everything`() {
        assertEquals(tags, SearchSuggestions.tagCompletions("#", tags))
    }

    @Test fun `plain words and finished tags offer nothing`() {
        assertEquals(emptyList<String>(), SearchSuggestions.tagCompletions("wo", tags))
        assertEquals(emptyList<String>(), SearchSuggestions.tagCompletions("#work", tags))
        assertEquals(emptyList<String>(), SearchSuggestions.tagCompletions("#wo ", tags))
    }

    @Test fun `completing keeps the rest of the query, and leaves room for more`() {
        assertEquals("#work ", SearchSuggestions.complete("#wo", "work"))
        assertEquals("dog cat #work ", SearchSuggestions.complete("dog cat #wo", "work"))
    }

    @Test fun `the newest search comes first, once, and the list stays short`() {
        var recent = emptyList<String>()
        for (n in 1..12) recent = SearchSuggestions.remember(recent, "query $n")
        recent = SearchSuggestions.remember(recent, "query 11")

        assertEquals("query 11", recent.first())
        assertEquals(SearchSuggestions.MAX_RECENT, recent.size)
        assertEquals(1, recent.count { it == "query 11" })
    }

    @Test fun `an empty search isn't worth remembering`() {
        assertEquals(listOf("kept"), SearchSuggestions.remember(listOf("kept"), "   "))
    }
}
