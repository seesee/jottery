package com.jottery.android.service

import com.jottery.android.model.DecryptedNote
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.Instant

/**
 * The search cases every client runs, so a saved search means the same thing
 * everywhere. A copy of docs/search-query-fixture.json.
 */
class SearchFixtureTest {
    @Test fun `the shared query fixture passes`() {
        val text = javaClass.classLoader!!.getResourceAsStream("search-query-fixture.json")!!
            .use { it.readBytes().decodeToString() }
        val fixture = Json.parseToJsonElement(text).jsonObject

        val notes = fixture.getValue("notes").jsonArray.map { element ->
            val note = element.jsonObject
            DecryptedNote(
                id = note.getValue("id").jsonPrimitive.content,
                createdAt = Instant.EPOCH,
                modifiedAt = Instant.EPOCH,
                content = note.getValue("content").jsonPrimitive.content,
                tags = note.getValue("tags").jsonArray.map { it.jsonPrimitive.content },
                attachments = emptyList(),
                pinned = false,
                archived = false,
                locked = false,
                deleted = false,
                version = 1,
                wordWrap = true,
                syntaxLanguage = "markdown",
                showPreview = false,
                needsSync = false,
            )
        }

        val failures = fixture.getValue("cases").jsonArray.mapNotNull { element ->
            val case = element.jsonObject
            val query = case.getValue("query").jsonPrimitive.content
            val expected = case.getValue("expected").jsonArray.map { it.jsonPrimitive.content }.sorted()
            val actual = SearchService.filter(notes, query).notes.map { it.id }.sorted()
            if (expected == actual) null else "$query: expected $expected, got $actual"
        }

        assertEquals(failures.joinToString("\n"), 0, failures.size)
    }
}
