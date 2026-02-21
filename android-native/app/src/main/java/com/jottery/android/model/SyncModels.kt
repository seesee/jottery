package com.jottery.android.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// MARK: - Credential Import Payloads

/** Decrypted payload inside jottery:v1: encrypted credentials. */
@Serializable
data class CredentialPayload(
    val endpoint: String,
    val clientId: String,
    val apiKey: String,
)

/** Legacy base64 JSON credentials (includes salt). */
@Serializable
data class LegacyCredentialPayload(
    val endpoint: String,
    val clientId: String,
    val apiKey: String,
    val salt: String? = null,
)

// MARK: - Device Registration

@Serializable
data class RegisterDeviceRequest(
    val email: String,
    val password: String,
    val deviceName: String,
    val deviceType: String = "android",
)

@Serializable
data class RegisterDeviceResponse(
    val clientId: String,
    val apiKey: String,
    val userId: String,
    val deviceName: String,
)

@Serializable
data class CloneDeviceRequest(
    val apiKey: String,
    val deviceName: String,
    val deviceType: String = "android",
)

// MARK: - Sync Push

@Serializable
data class SyncPushRequest(
    val notes: List<SyncNote>,
    val attachments: List<SyncAttachment>,
    val versions: List<SyncNoteVersion>,
    val deletions: List<SyncDeletion> = emptyList(),
    val savedSearches: List<SyncSavedSearch> = emptyList(),
)

@Serializable
data class SyncPushResponse(
    val accepted: List<SyncAccepted>,
    val rejected: List<SyncRejected>,
    val errors: List<String>? = null,
)

@Serializable
data class SyncAccepted(
    val id: String,
    val serverVersion: Int? = null,
    val syncedAt: String? = null,
)

@Serializable
data class SyncRejected(
    val id: String,
    val reason: String,
    val serverModifiedAt: String,
    val serverContent: String,
    val serverTags: List<String>,
    val serverVersion: Int,
    val serverAttachments: List<AttachmentRef>,
    val serverPinned: Boolean,
    val serverSyntaxLanguage: String? = null,
    val serverWordWrap: Boolean? = null,
    val serverShowPreview: Boolean? = null,
    val serverContentHash: String? = null,
    val serverParentHash: String? = null,
    val serverHashChain: List<String>? = null,
    val ancestorHash: String? = null,
    val ancestorContent: String? = null,
    val ancestorTags: List<String>? = null,
)

// MARK: - Sync Pull

@Serializable
data class SyncPullRequest(
    val lastSyncAt: String? = null,
    val knownNoteIds: List<String>,
    val knownAttachmentIds: List<String>,
    val limit: Int? = null,
    val offset: Int? = null,
)

@Serializable
data class SyncPullResponse(
    val notes: List<SyncNote>,
    val deletions: List<SyncDeletion>? = null,
    val attachments: List<SyncAttachment>,
    val versions: List<SyncNoteVersion>,
    val savedSearches: List<SyncSavedSearch>? = null,
    val syncedAt: String? = null,
    val totalCount: Int? = null,
    val hasMore: Boolean? = null,
)

// MARK: - Sync Note

@Serializable
data class SyncNote(
    val id: String,
    val createdAt: String,
    val modifiedAt: String,
    val content: String,
    val tags: List<String>,
    val attachments: List<AttachmentRef>,
    val pinned: Boolean,
    val archived: Boolean = false,
    val archivedAt: String? = null,
    val locked: Boolean? = null,
    val lockedAt: String? = null,
    val deleted: Boolean,
    val deletedAt: String? = null,
    val version: Int,
    val wordWrap: Boolean? = null,
    val syntaxLanguage: String? = null,
    val showPreview: Boolean? = null,
    val color: String? = null,
    val contentHash: String? = null,
    val parentHash: String? = null,
    val hashChain: List<String>? = null,
)

@Serializable
data class SyncAttachment(
    val id: String,
    val data: String,
)

@Serializable
data class SyncDeletion(
    val id: String,
    val deletedAt: String,
)

@Serializable
data class SyncNoteVersion(
    val versionKey: String,
    val noteId: String,
    val version: Int,
    val createdAt: String,
    val syncedAt: String,
    val content: String,
    val tags: List<String>,
    val attachments: List<AttachmentRef>,
    val syntaxLanguage: String? = null,
    val wordWrap: Boolean? = null,
    val showPreview: Boolean? = null,
    val color: String? = null,
    val reason: String,
    val contentHash: String? = null,
    val parentHash: String? = null,
)

// MARK: - Inbox

@Serializable
data class InboxItem(
    val id: String,
    val content: String,
    val tags: List<String>,
    val createdAt: String,
    val source: String? = null,
    val sizeBytes: Int? = null,
)

@Serializable
data class InboxStatusResponse(
    val count: Int,
    val totalSizeBytes: Int,
    val maxItems: Int,
    val maxSizeMb: Int,
)

@Serializable
data class InboxTokenResponse(
    val token: String,
)

@Serializable
data class InboxTokenStatusResponse(
    val hasToken: Boolean,
)

// MARK: - Sync Status

@Serializable
data class SyncStatusResponse(
    val clientId: String,
    val deviceName: String,
    val serverLastModified: String,
    val noteCount: Int,
    val lastSyncedAt: String? = null,
)

// MARK: - SSE Event Token

@Serializable
data class SSETokenResponse(
    val token: String,
    @SerialName("expires_in")
    val expiresIn: Int,
)

// MARK: - Sync Saved Search

@Serializable
data class SyncSavedSearch(
    val id: String,
    val name: String,
    val query: String,
    val order: Int,
    val createdAt: String,
    val modifiedAt: String,
    val deleted: Boolean,
    val deletedAt: String? = null,
    val version: Int,
)
