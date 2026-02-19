package com.jottery.android.service

import android.util.Base64
import android.util.Log
import com.jottery.android.crypto.CryptoService
import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.NoteRepository
import com.jottery.android.model.AttachmentRecord
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.DecryptedNote
import kotlinx.serialization.json.Json
import javax.crypto.SecretKey

/**
 * Import notes from the decrypted JSON export format.
 * Encrypts content and tags before storing.
 */
class ImportService(
    private val noteRepo: NoteRepository,
    private val attachmentRepo: AttachmentRepository,
) {
    companion object {
        private const val TAG = "ImportService"
    }

    private val json = Json { ignoreUnknownKeys = true }

    /**
     * Import notes from a JSON string.
     * Returns the number of notes imported.
     */
    suspend fun import(jsonString: String, key: SecretKey): Int {
        val payload: ExportService.ExportPayload = json.decodeFromString(jsonString)

        Log.d(TAG, "Importing ${payload.notes.size} notes (format v${payload.version})")

        var count = 0
        for (exportNote in payload.notes) {
            try {
                // Handle attachments
                val attachmentRefs = mutableListOf<AttachmentRef>()
                for (att in exportNote.attachments) {
                    val data = Base64.decode(att.data, Base64.NO_WRAP)
                    val encrypted = CryptoService.encrypt(data, key)
                    val encryptedJSON = CryptoService.serializeEncryptedJSON(encrypted)
                    val blobData = encryptedJSON.toByteArray(Charsets.UTF_8)
                    val id = CryptoService.generateUUID()

                    attachmentRepo.insertOrReplace(
                        AttachmentRecord(
                            id = id,
                            filename = att.filename,
                            mimeType = att.mimeType,
                            size = data.size,
                            data = blobData,
                        )
                    )

                    val encFilename = CryptoService.encryptText(att.filename, key)
                    attachmentRefs.add(
                        AttachmentRef(
                            id = id,
                            filename = CryptoService.serializeEncryptedJSON(encFilename),
                            mimeType = att.mimeType,
                            size = data.size,
                            data = id,
                        )
                    )
                }

                // Create the note
                noteRepo.create(
                    content = exportNote.content,
                    tags = exportNote.tags,
                    key = key,
                )
                count++
            } catch (e: Exception) {
                Log.e(TAG, "Failed to import note ${exportNote.id}: ${e.message}")
            }
        }

        return count
    }
}
