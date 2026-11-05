package com.jottery.android.service

import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.DecryptedNote
import com.jottery.android.model.NoteCategories
import com.jottery.android.model.SortOrder
import org.junit.Assert.*
import org.junit.Test
import java.time.Instant

/**
 * JVM unit tests for SearchService query parsing and filtering.
 */
class SearchServiceTest {

    // -- Query Parsing --

    @Test
    fun `empty query matches all notes`() {
        val notes = listOf(makeNote("Hello"), makeNote("World"))
        val result = SearchService.filter(notes, "")
        assertEquals(2, result.matchCount)
        assertEquals(2, result.totalCount)
    }

    @Test
    fun `simple text search filters by content`() {
        val notes = listOf(makeNote("Hello world"), makeNote("Goodbye world"), makeNote("Hi there"))
        val result = SearchService.filter(notes, "hello")
        assertEquals(1, result.matchCount)
        assertEquals(3, result.totalCount)
    }

    @Test
    fun `tag search filters by tag`() {
        val notes = listOf(
            makeNote("Note 1", tags = listOf("animals", "dogs")),
            makeNote("Note 2", tags = listOf("animals", "cats")),
            makeNote("Note 3", tags = listOf("plants")),
        )
        val result = SearchService.filter(notes, "#animals")
        assertEquals(2, result.matchCount)
    }

    @Test
    fun `multiple tags require all tags (AND)`() {
        val notes = listOf(
            makeNote("Note 1", tags = listOf("animals", "dogs")),
            makeNote("Note 2", tags = listOf("animals", "cats")),
        )
        val result = SearchService.filter(notes, "#animals #dogs")
        assertEquals(1, result.matchCount)
    }

    @Test
    fun `OR tags match any tag`() {
        val notes = listOf(
            makeNote("Note 1", tags = listOf("dogs")),
            makeNote("Note 2", tags = listOf("cats")),
            makeNote("Note 3", tags = listOf("fish")),
        )
        val result = SearchService.filter(notes, "#dogs | #cats")
        assertEquals(2, result.matchCount)
    }

    @Test
    fun `negation excludes matching notes`() {
        val notes = listOf(
            makeNote("I like dogs and cats"),
            makeNote("I like dogs only"),
            makeNote("I like fish"),
        )
        val result = SearchService.filter(notes, "dogs -cats")
        assertEquals(1, result.matchCount)
    }

    @Test
    fun `tag negation excludes notes with tag`() {
        val notes = listOf(
            makeNote("Note 1", tags = listOf("animals", "dogs")),
            makeNote("Note 2", tags = listOf("animals", "cats")),
        )
        val result = SearchService.filter(notes, "#animals -#dogs")
        assertEquals(1, result.matchCount)
    }

    @Test
    fun `exact phrase search`() {
        val notes = listOf(
            makeNote("Hello beautiful world"),
            makeNote("Hello world"),
            makeNote("world hello"),
        )
        val result = SearchService.filter(notes, "\"hello world\"")
        assertEquals(1, result.matchCount) // Only "Hello world" matches exactly
    }

    @Test
    fun `prefix wildcard search`() {
        val notes = listOf(
            makeNote("The doghouse is big"),
            makeNote("The cat is small"),
            makeNote("I like dogs"),
        )
        val result = SearchService.filter(notes, "dog*")
        assertEquals(2, result.matchCount)
    }

    @Test
    fun `suffix wildcard search`() {
        val notes = listOf(
            makeNote("The hotdog is tasty"),
            makeNote("I have a dog"),
            makeNote("No match here"),
        )
        val result = SearchService.filter(notes, "*dog")
        assertEquals(2, result.matchCount)
    }

    @Test
    fun `has attachment modifier`() {
        val notes = listOf(
            makeNote("With attachment", attachments = listOf(
                AttachmentRef(id = "1", filename = "test.png", mimeType = "image/png", size = 100, data = "blob1")
            )),
            makeNote("Without attachment"),
        )
        val result = SearchService.filter(notes, "has:attachment")
        assertEquals(1, result.matchCount)
    }

    @Test
    fun `word count filter`() {
        val notes = listOf(
            makeNote("one two three four five six seven eight nine ten"), // 10 words
            makeNote("one two three"), // 3 words
            makeNote("one"), // 1 word
        )
        val result = SearchService.filter(notes, "words:>5")
        assertEquals(1, result.matchCount)
    }

    @Test
    fun `color filter`() {
        val notes = listOf(
            makeNote("Red note", color = "red"),
            makeNote("Blue note", color = "blue"),
            makeNote("No color"),
        )
        val result = SearchService.filter(notes, "color:red")
        assertEquals(1, result.matchCount)
    }

    @Test
    fun `OR text search`() {
        val notes = listOf(
            makeNote("I have a dog"),
            makeNote("I have a cat"),
            makeNote("I have a fish"),
        )
        val result = SearchService.filter(notes, "dog | cat")
        assertEquals(2, result.matchCount)
    }

    @Test
    fun `combined tag and text search`() {
        val notes = listOf(
            makeNote("Dogs are great", tags = listOf("animals")),
            makeNote("Cats are great", tags = listOf("animals")),
            makeNote("Dogs are OK", tags = listOf("other")),
        )
        val result = SearchService.filter(notes, "#animals dogs")
        assertEquals(1, result.matchCount)
    }

    // -- Sorting --

    @Test
    fun `sort by recent puts newest first`() {
        val notes = listOf(
            makeNote("Old", modifiedAt = Instant.parse("2024-01-01T00:00:00Z")),
            makeNote("New", modifiedAt = Instant.parse("2024-06-01T00:00:00Z")),
        )
        val sorted = SearchService.sort(notes, SortOrder.RECENT)
        assertEquals("New", sorted[0].content)
    }

    @Test
    fun `sort puts pinned notes first`() {
        val notes = listOf(
            makeNote("Unpinned", pinned = false, modifiedAt = Instant.parse("2024-06-01T00:00:00Z")),
            makeNote("Pinned", pinned = true, modifiedAt = Instant.parse("2024-01-01T00:00:00Z")),
        )
        val sorted = SearchService.sort(notes, SortOrder.RECENT)
        assertEquals("Pinned", sorted[0].content)
    }

    @Test
    fun `sort alphabetical`() {
        val notes = listOf(
            makeNote("Banana"),
            makeNote("Apple"),
            makeNote("Cherry"),
        )
        val sorted = SearchService.sort(notes, SortOrder.ALPHA)
        assertEquals("Apple", sorted[0].content)
        assertEquals("Banana", sorted[1].content)
        assertEquals("Cherry", sorted[2].content)
    }

    // Categories

    private val categories = NoteCategories(
        names = mapOf("red" to "Urgent", "blue" to "Very Important"),
        tagColours = mapOf("work" to "red"),
    )

    private fun search(query: String, vararg notes: DecryptedNote) =
        SearchService.filter(notes.toList(), query, categories).notes.map { it.content }

    @Test fun `a colour's name finds its notes`() {
        val notes = arrayOf(makeNote("One", color = "red"), makeNote("Two", color = "green"))

        assertEquals(listOf("One"), search("category:urgent", *notes))
    }

    @Test fun `a name of two words is one name, and the search goes on after it`() {
        val notes = arrayOf(makeNote("dog", color = "blue"), makeNote("cat", color = "blue"), makeNote("dog"))

        assertEquals(listOf("dog"), search("category:very important dog", *notes))
    }

    @Test fun `a colour still answers to its own name`() {
        assertEquals(listOf("One"), search("category:red", makeNote("One", color = "red"), makeNote("Two")))
    }

    @Test fun `a colour finds tags of that colour too`() {
        val notes = arrayOf(makeNote("Tagged", tags = listOf("Work")), makeNote("Coloured", color = "red"), makeNote("Neither"))

        assertEquals(listOf("Tagged", "Coloured"), search("color:red", *notes))
    }

    @Test fun `tag and note narrow a colour to that kind`() {
        val notes = arrayOf(makeNote("Tagged", tags = listOf("work")), makeNote("Coloured", color = "red"))

        assertEquals(listOf("Tagged"), search("color:red tag", *notes))
        assertEquals(listOf("Coloured"), search("color:red note", *notes))
        assertEquals(listOf("Tagged"), search("category:urgent tag", *notes))
    }

    @Test fun `with no categories, colours behave as they always did`() {
        val notes = listOf(makeNote("Tagged", tags = listOf("work")), makeNote("Coloured", color = "red"))

        assertEquals(listOf("Coloured"), SearchService.filter(notes, "color:red").notes.map { it.content })
        assertEquals(emptyList<String>(), SearchService.filter(notes, "color:red tag").notes.map { it.content })
    }

    // -- Helpers --

    private fun makeNote(
        content: String,
        tags: List<String> = emptyList(),
        pinned: Boolean = false,
        attachments: List<AttachmentRef> = emptyList(),
        color: String? = null,
        modifiedAt: Instant = Instant.now(),
        createdAt: Instant = Instant.now(),
    ): DecryptedNote = DecryptedNote(
        id = java.util.UUID.randomUUID().toString(),
        createdAt = createdAt,
        modifiedAt = modifiedAt,
        content = content,
        tags = tags,
        attachments = attachments,
        pinned = pinned,
        archived = false,
        locked = false,
        deleted = false,
        version = 1,
        wordWrap = true,
        syntaxLanguage = "markdown",
        showPreview = false,
        color = color,
        needsSync = false,
    )

    @Test fun `a before date includes that day`() {
        val zone = java.time.ZoneId.systemDefault()
        val lastThing = java.time.LocalDate.parse("2024-06-30").plusDays(1).atStartOfDay(zone).toInstant().minusSeconds(1)
        val nextDay = lastThing.plusSeconds(2)
        val onTheDay = makeNote(content = "a", createdAt = lastThing)
        val after = makeNote(content = "b", createdAt = nextDay)

        assertEquals(listOf(onTheDay), SearchService.filter(listOf(onTheDay, after), "created:<2024-06-30").notes)
    }

    @Test fun `archived finds only archived notes, in either list`() {
        val note = makeNote(content = "plain")
        val archived = makeNote(content = "kept").copy(archived = true)

        assertEquals(listOf(archived), SearchService.filter(listOf(note, archived), "archived").notes)
        assertEquals(listOf(archived), SearchService.filter(listOf(note, archived), "archived:true").notes)
        assertTrue(SearchService.searchesArchive("kept archived"))
        assertFalse(SearchService.searchesArchive("the archived-box"))
    }

    @Test fun `colour qualifiers aren't searched for as text`() {
        val red = makeNote(content = "no such word", color = "red")

        assertEquals(listOf(red), SearchService.filter(listOf(red), "color:red note").notes)
        assertTrue("this client has no tag colours", SearchService.filter(listOf(red), "color:red tag").notes.isEmpty())
    }
}
