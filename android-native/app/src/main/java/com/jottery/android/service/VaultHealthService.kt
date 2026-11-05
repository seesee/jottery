package com.jottery.android.service

import com.jottery.android.database.repository.AttachmentRepository
import com.jottery.android.database.repository.NoteRepository
import javax.crypto.SecretKey

/** Finds what's wrong with a vault, for the user to act on. Nothing here changes anything. */
class VaultHealthService(
    private val noteRepo: NoteRepository,
    private val attachmentRepo: AttachmentRepository,
) {
    data class UndecryptableNote(val id: String, val modifiedAt: String)

    /** [noteTitle] is null when the note itself can't be read either. */
    data class MissingAttachment(val blobId: String, val noteId: String, val noteTitle: String?)

    data class Report(
        val undecryptable: List<UndecryptableNote> = emptyList(),
        val missing: List<MissingAttachment> = emptyList(),
    ) {
        val isHealthy: Boolean get() = undecryptable.isEmpty() && missing.isEmpty()
    }

    /** The bin is left out, as nobody is relying on what's in it. */
    suspend fun scan(key: SecretKey): Report {
        val stored = attachmentRepo.listBlobIds().toSet()
        val undecryptable = mutableListOf<UndecryptableNote>()
        val missing = mutableListOf<MissingAttachment>()

        for (record in noteRepo.listAllRaw().filterNot { it.deleted }) {
            val note = try {
                noteRepo.get(record.id, key)
            } catch (_: Exception) {
                undecryptable += UndecryptableNote(record.id, record.modifiedAt)
                null
            }
            for (ref in noteRepo.rawAttachmentRefs(record.id)) {
                if (ref.data !in stored) missing += MissingAttachment(ref.data, record.id, note?.title)
            }
        }
        return Report(undecryptable, missing)
    }
}
