package com.jottery.android.service

import com.jottery.android.model.AttachmentWarning
import com.jottery.android.model.SyncPushResponse
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * JVM unit tests for the server's attachmentWarnings push-response field and
 * the client reaction that re-uploads blobs this device still holds.
 */
class AttachmentWarningTest {

    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun `push response decodes with warnings`() {
        val response = json.decodeFromString<SyncPushResponse>(
            """{"accepted":[],"rejected":[],"attachmentWarnings":[{"noteId":"n1","attachmentIds":["a1","a2"]}]}"""
        )
        val warnings = response.attachmentWarnings
        assertEquals(1, warnings?.size)
        assertEquals("n1", warnings?.get(0)?.noteId)
        assertEquals(listOf("a1", "a2"), warnings?.get(0)?.attachmentIds)
    }

    @Test
    fun `push response decodes without warnings`() {
        val response = json.decodeFromString<SyncPushResponse>(
            """{"accepted":[],"rejected":[]}"""
        )
        assertNull(response.attachmentWarnings)
    }

    @Test
    fun `collectRepairAttachments returns held blobs only, deduplicated`() {
        val warnings = listOf(
            AttachmentWarning(noteId = "n1", attachmentIds = listOf("held", "ghost")),
            AttachmentWarning(noteId = "n2", attachmentIds = listOf("held")),
        )
        val blob = "hello".toByteArray()

        val repairs = SyncService.collectRepairAttachments(warnings) { id ->
            if (id == "held") blob else null
        }

        assertEquals(1, repairs.size)
        assertEquals("held", repairs[0].id)
        assertEquals(java.util.Base64.getEncoder().encodeToString(blob), repairs[0].data)
    }

    @Test
    fun `collectRepairAttachments empty when nothing held`() {
        val warnings = listOf(AttachmentWarning(noteId = "n1", attachmentIds = listOf("ghost")))
        val repairs = SyncService.collectRepairAttachments(warnings) { null }
        assertTrue(repairs.isEmpty())
    }
}
