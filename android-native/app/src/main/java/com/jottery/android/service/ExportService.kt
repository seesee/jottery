package com.jottery.android.service

import android.util.Base64
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.model.DecryptedNote
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import javax.crypto.SecretKey

/**
 * Export notes as decrypted JSON.
 *
 * Format matches the specification in CLAUDE.md:
 * ```json
 * {
 *   "version": "1.0",
 *   "exportDate": "...",
 *   "notes": [...]
 * }
 * ```
 */
class ExportService(
    private val noteRepo: NoteRepository,
    private val attachmentRepo: AttachmentRepository,
) {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true; prettyPrint = true }

    @Serializable
    data class ExportPayload(
        val version: String = "1.0",
        val exportDate: String,
        val notes: List<ExportNote>,
    )

    @Serializable
    data class ExportNote(
        val id: String,
        val createdAt: String,
        val modifiedAt: String,
        val content: String,
        val tags: List<String>,
        val attachments: List<ExportAttachment> = emptyList(),
        val pinned: Boolean,
    )

    @Serializable
    data class ExportAttachment(
        val filename: String,
        val mimeType: String,
        val data: String, // Base64-encoded decrypted data
    )

    /**
     * Export the given notes as a decrypted JSON string.
     */
    suspend fun export(notes: List<DecryptedNote>, key: SecretKey): String {
        val exportNotes = notes.map { note ->
            val attachments = note.attachments.mapNotNull { ref ->
                try {
                    val blob = attachmentRepo.getBlob(ref.data) ?: return@mapNotNull null
                    val blobStr = String(blob, Charsets.UTF_8)
                    val encrypted = com.jottery.android.crypto.CryptoService.parseEncryptedJSON(blobStr)
                    val decryptedData = com.jottery.android.crypto.CryptoService.decrypt(encrypted, key)
                    ExportAttachment(
                        filename = ref.filename,
                        mimeType = ref.mimeType,
                        data = Base64.encodeToString(decryptedData, Base64.NO_WRAP),
                    )
                } catch (_: Exception) { null }
            }

            ExportNote(
                id = note.id,
                createdAt = note.createdAt.toString(),
                modifiedAt = note.modifiedAt.toString(),
                content = note.content,
                tags = note.tags,
                attachments = attachments,
                pinned = note.pinned,
            )
        }

        val payload = ExportPayload(
            exportDate = com.jottery.android.util.DateUtils.nowISO8601(),
            notes = exportNotes,
        )

        return json.encodeToString(payload)
    }
}
