package com.jottery.android.service

import com.jottery.android.crypto.CryptoService
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.SyncAttachment
import com.jottery.android.model.SyncNote
import com.jottery.android.model.SyncNoteVersion
import com.jottery.android.model.SyncSavedSearch
import com.jottery.android.util.DateUtils
import java.util.Base64

/**
 * The server stores ciphertext and is not trusted, so everything it sends is
 * checked here, at the boundary (D4). What fails is dropped, and the rest of the
 * sync carries on. Decrypted text is checked separately, in SyncService.
 */
object SyncValidator {
    private val UUID = Regex("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
    private val MIME_TYPE = Regex(
        "^[A-Za-z0-9][A-Za-z0-9!#$&^_.+-]{0,126}/[A-Za-z0-9][A-Za-z0-9!#$&^_.+-]{0,126}" +
            "(\\s*;\\s*[A-Za-z0-9_-]{1,64}=[A-Za-z0-9_.\"-]{1,64}){0,4}$"
    )

    /** Web allows 10 MB attachments; encryption and base64 add about a half. */
    private const val MAX_BLOB_BYTES = 32 * 1024 * 1024
    private const val MAX_FIELD_LENGTH = 256

    /** Well past the server's own limit, so only a hostile value reaches it. */
    private const val MAX_CONTENT_LENGTH = 64 * 1024 * 1024
    private const val MAX_HASH_LENGTH = 128

    fun isUuid(value: String): Boolean = UUID.matches(value)

    private fun isTimestamp(value: String?): Boolean =
        value == null || DateUtils.parseISO8601(value) != null

    private fun isEncrypted(value: String): Boolean = try {
        CryptoService.parseEncryptedJSON(value)
        true
    } catch (_: Exception) {
        false
    }

    fun isValidRef(ref: AttachmentRef): Boolean =
        isUuid(ref.id) && isUuid(ref.data) && ref.size >= 0 && ref.size <= MAX_BLOB_BYTES &&
            (ref.mimeType.isEmpty() || MIME_TYPE.matches(ref.mimeType))

    fun isValidNote(note: SyncNote): Boolean =
        isUuid(note.id) &&
            DateUtils.parseISO8601(note.createdAt) != null &&
            DateUtils.parseISO8601(note.modifiedAt) != null &&
            isTimestamp(note.archivedAt) && isTimestamp(note.lockedAt) && isTimestamp(note.deletedAt) &&
            note.content.length <= MAX_CONTENT_LENGTH &&
            isEncrypted(note.content) &&
            note.attachments.all(::isValidRef) &&
            (note.syntaxLanguage?.length ?: 0) <= MAX_FIELD_LENGTH &&
            (note.color?.length ?: 0) <= MAX_FIELD_LENGTH &&
            (note.contentHash?.length ?: 0) <= MAX_HASH_LENGTH &&
            (note.parentHash?.length ?: 0) <= MAX_HASH_LENGTH

    /** Keep only what the hash chain can hold. */
    fun sanitised(note: SyncNote): SyncNote = note.copy(
        hashChain = note.hashChain
            ?.filter { it.length <= MAX_HASH_LENGTH }
            ?.take(CryptoService.MAX_HASH_CHAIN_LENGTH),
    )

    fun isValidVersion(version: SyncNoteVersion): Boolean =
        isUuid(version.noteId) &&
            version.versionKey.startsWith(version.noteId) &&
            version.versionKey.length <= MAX_FIELD_LENGTH &&
            version.reason.length <= MAX_FIELD_LENGTH &&
            DateUtils.parseISO8601(version.createdAt) != null &&
            isEncrypted(version.content) &&
            version.attachments.all(::isValidRef)

    fun isValidSavedSearch(search: SyncSavedSearch): Boolean =
        isUuid(search.id) &&
            DateUtils.parseISO8601(search.modifiedAt) != null &&
            DateUtils.parseISO8601(search.createdAt) != null &&
            isEncrypted(search.name) && isEncrypted(search.query)

    /** The blob's bytes, or null if it isn't one we should store. */
    fun decodeBlob(attachment: SyncAttachment): ByteArray? {
        if (!isUuid(attachment.id) || attachment.data.length > MAX_BLOB_BYTES / 3 * 4 + 4) return null
        return try {
            Base64.getDecoder().decode(attachment.data)
        } catch (_: IllegalArgumentException) {
            null
        }
    }
}
