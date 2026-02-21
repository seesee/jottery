package com.jottery.android.model

/**
 * In-memory only — for conflict resolution UI.
 */
data class ConflictInfo(
    val id: String,
    val localContent: String,
    val localTags: List<String>,
    val localModifiedAt: String,
    val serverContent: String,
    val serverTags: List<String>,
    val serverModifiedAt: String,
    val serverEncryptedContent: String,
    val serverEncryptedTags: List<String>,
    val serverVersion: Int,
    val serverAttachments: List<AttachmentRef>,
    val serverPinned: Boolean,
    val serverSyntaxLanguage: String? = null,
    val serverWordWrap: Boolean? = null,
    val serverShowPreview: Boolean? = null,
    val serverContentHash: String? = null,
    val serverParentHash: String? = null,
    val serverHashChain: List<String>? = null,
)

enum class ConflictResolutionStrategy {
    KEEP_LOCAL,
    KEEP_SERVER,
    KEEP_BOTH,
}
