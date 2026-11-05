package com.jottery.android.model

import kotlinx.serialization.Serializable

/**
 * The server's side of a conflict, as received: every encrypted field stays
 * encrypted. This is what is persisted in note_sync_metadata.conflict_data, so
 * a conflict survives lock and process death without any plaintext on disk.
 */
@Serializable
data class StoredConflict(
    val id: String,
    val serverModifiedAt: String,
    val serverEncryptedContent: String,
    val serverEncryptedTags: List<String>,
    val serverVersion: Int,
    val serverAttachments: List<AttachmentRef> = emptyList(),
    val serverPinned: Boolean = false,
    val serverArchived: Boolean = false,
    val serverArchivedAt: String? = null,
    val serverLocked: Boolean = false,
    val serverLockedAt: String? = null,
    val serverDeleted: Boolean = false,
    val serverDeletedAt: String? = null,
    val serverColor: String? = null,
    val serverSyntaxLanguage: String? = null,
    val serverWordWrap: Boolean? = null,
    val serverShowPreview: Boolean? = null,
    val serverContentHash: String? = null,
    val serverParentHash: String? = null,
    val serverHashChain: List<String>? = null,
    val ancestorContent: String? = null,
    val ancestorTags: List<String>? = null,
)

/**
 * A conflict ready to show: the stored server side plus both sides decrypted.
 * In memory only, so never persist this.
 *
 * [serverContent] is null when the server's text could not be decrypted. It is
 * never replaced with the raw value, and the server copy cannot then be chosen.
 */
data class ConflictInfo(
    val stored: StoredConflict,
    val localContent: String,
    val localTags: List<String>,
    val localModifiedAt: String,
    val serverContent: String?,
    val serverTags: List<String>,
    val ancestorContent: String? = null,
) {
    val id: String get() = stored.id
    val serverModifiedAt: String get() = stored.serverModifiedAt
    val serverUnreadable: Boolean get() = serverContent == null

    /** The note's first line, from whichever side has one. */
    val title: String
        get() = (localContent.lineSequence() + serverContent.orEmpty().lineSequence())
            .map { it.trim().trimStart('#').trim() }
            .firstOrNull { it.isNotEmpty() }
            ?.take(80)
            .orEmpty()
}

enum class ConflictResolutionStrategy {
    KEEP_LOCAL,
    KEEP_SERVER,
    KEEP_BOTH,
}
