package com.jottery.android.service

import android.util.Base64
import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.model.DecryptedNote
import com.jottery.android.util.DateUtils
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.OutputStream
import javax.crypto.SecretKey

/**
 * Export notes as decrypted JSON, in the format every client reads:
 * ```json
 * { "version": "1.0", "exportDate": "...", "notes": [...] }
 * ```
 */
class ExportService(
    private val attachmentRepo: AttachmentRepository,
) {
    private companion object {
        const val TAG = "ExportService"
    }

    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    @Serializable
    data class ExportPayload(
        val version: String = "1.0",
        val exportDate: String = "",
        val notes: List<ExportNote>,
    )

    @Serializable
    data class ExportNote(
        val id: String,
        val createdAt: String,
        val modifiedAt: String,
        val content: String,
        val tags: List<String> = emptyList(),
        val attachments: List<ExportAttachment> = emptyList(),
        val pinned: Boolean = false,
        val archived: Boolean = false,
        val locked: Boolean = false,
        val wordWrap: Boolean = true,
        val syntaxLanguage: String = "markdown",
        val showPreview: Boolean = false,
        val color: String? = null,
    )

    @Serializable
    data class ExportAttachment(
        val filename: String,
        val mimeType: String,
        val data: String, // Base64-encoded decrypted data
    )

    data class Result(val notes: Int, val skippedAttachments: Int)

    /** Stream the notes as decrypted JSON, a note at a time. */
    suspend fun export(notes: List<DecryptedNote>, key: SecretKey, out: OutputStream): Result {
        val writer = out.bufferedWriter(Charsets.UTF_8)
        var skipped = 0

        writer.write("{\"version\":\"1.0\",\"exportDate\":\"${DateUtils.nowISO8601()}\",\"notes\":[")
        notes.forEachIndexed { index, note ->
            val attachments = note.attachments.mapNotNull { ref ->
                try {
                    val blob = attachmentRepo.getBlob(ref.data) ?: throw IllegalStateException("no local blob")
                    val encrypted = CryptoService.parseEncryptedJSON(String(blob, Charsets.UTF_8))
                    ExportAttachment(
                        filename = ref.filename,
                        mimeType = ref.mimeType,
                        data = Base64.encodeToString(CryptoService.decrypt(encrypted, key), Base64.NO_WRAP),
                    )
                } catch (e: Exception) {
                    Log.w(TAG, "Skipping attachment ${ref.id}: ${e.javaClass.simpleName}")
                    skipped++
                    null
                }
            }
            if (index > 0) writer.write(",")
            writer.write(
                json.encodeToString(
                    ExportNote(
                        id = note.id,
                        createdAt = DateUtils.formatISO8601(note.createdAt),
                        modifiedAt = DateUtils.formatISO8601(note.modifiedAt),
                        content = note.content,
                        tags = note.tags,
                        attachments = attachments,
                        pinned = note.pinned,
                        archived = note.archived,
                        locked = note.locked,
                        wordWrap = note.wordWrap,
                        syntaxLanguage = note.syntaxLanguage,
                        showPreview = note.showPreview,
                        color = note.color,
                    )
                )
            )
        }
        writer.write("]}")
        writer.flush()
        return Result(notes.size, skipped)
    }
}
