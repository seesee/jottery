package com.jottery.android.service

import android.util.Base64
import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.util.DateUtils
import kotlinx.serialization.json.Json
import javax.crypto.SecretKey

/** What to do when an imported note's id already exists here. */
enum class ImportStrategy {
    /** Leave the note here as it is. */
    SKIP,

    /** Replace it with the imported copy. */
    REPLACE,

    /** Keep both: the imported copy gets a new id. */
    KEEP_BOTH,
}

/**
 * Import notes from the decrypted JSON export format, keeping everything the file
 * carries: attachments, dates, pin and archive state, language and colour. Each
 * note goes in its own transaction with its attachments.
 */
class ImportService(
    private val noteRepo: NoteRepository,
    private val attachmentRepo: AttachmentRepository,
    private val transaction: suspend (suspend () -> Unit) -> Unit,
) {
    companion object {
        private const val TAG = "ImportService"
    }

    data class Result(
        val imported: Int,
        val skipped: Int,
        val failed: Int,
        val attachments: Int,
        val attachmentsFailed: Int,
    )

    private val json = Json { ignoreUnknownKeys = true }

    suspend fun import(jsonString: String, key: SecretKey, strategy: ImportStrategy): Result {
        val payload: ExportService.ExportPayload = json.decodeFromString(jsonString)

        var imported = 0
        var skipped = 0
        var failed = 0
        var attachments = 0
        var attachmentsFailed = 0

        for (note in payload.notes) {
            val exists = SyncValidator.isUuid(note.id) && noteRepo.getRaw(note.id) != null
            if (exists && strategy == ImportStrategy.SKIP) {
                skipped++
                continue
            }
            // A new id for a kept copy, and for anything that isn't a valid id
            val id = if (SyncValidator.isUuid(note.id) && !(exists && strategy == ImportStrategy.KEEP_BOTH)) {
                note.id.lowercase()
            } else {
                CryptoService.generateUUID()
            }

            try {
                var noteAttachments = 0
                var noteAttachmentFailures = 0
                transaction {
                    val refs = note.attachments.mapNotNull { attachment ->
                        val ref = storeAttachment(attachment, key)
                        if (ref == null) noteAttachmentFailures++ else noteAttachments++
                        ref
                    }
                    noteRepo.createImported(id, note, refs, key)
                }
                imported++
                attachments += noteAttachments
                attachmentsFailed += noteAttachmentFailures
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.e(TAG, "Failed to import a note: ${e.javaClass.simpleName}")
                failed++
            }
        }

        return Result(imported, skipped, failed, attachments, attachmentsFailed)
    }

    /** A bad attachment is reported, and doesn't cost the note its text. */
    private suspend fun storeAttachment(
        attachment: ExportService.ExportAttachment,
        key: SecretKey,
    ): AttachmentRef? {
        val data = try {
            Base64.decode(attachment.data, Base64.DEFAULT)
        } catch (_: IllegalArgumentException) {
            return null
        }
        val blob = CryptoService.serializeEncryptedJSON(CryptoService.encrypt(data, key))
            .toByteArray(Charsets.UTF_8)
        val id = CryptoService.generateUUID()
        attachmentRepo.insertOrReplace(
            AttachmentRecord(id = id, filename = "", mimeType = attachment.mimeType, size = data.size, data = blob)
        )
        return AttachmentRef(
            id = id,
            filename = CryptoService.serializeEncryptedJSON(CryptoService.encryptText(attachment.filename, key)),
            mimeType = attachment.mimeType,
            size = data.size,
            data = id,
        )
    }
}
