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

// MARK: - Accounts

@Serializable
data class AccountCredentials(
    val email: String,
    val password: String,
)

@Serializable
data class RegisterUserResponse(
    val userId: String? = null,
)

@Serializable
data class UserStatusResponse(
    val exists: Boolean = false,
    val isApproved: Boolean = false,
    val isActive: Boolean = false,
)

@Serializable
data class UserLoginResponse(
    val sessionId: String? = null,
    val mustChangePassword: Boolean = false,
)

@Serializable
data class UserAccountInfo(
    val email: String = "",
    val noteCount: Int = 0,
    val attachmentCount: Int = 0,
    val storageUsedBytes: Long = 0,
    val storageQuotaMb: Long = 0,
    val inbox: InboxAccountInfo = InboxAccountInfo(),
)

@Serializable
data class InboxAccountInfo(
    val hasToken: Boolean = false,
)

// MARK: - Sync Push

@Serializable
data class SyncPushRequest(
    val notes: List<SyncNote>,
    val attachments: List<SyncAttachment>,
    val versions: List<SyncNoteVersion>,
    val deletions: List<SyncDeletion> = emptyList(),
    val savedSearches: List<SyncSavedSearch> = emptyList(),
    /** Every note here carries its complete list, so the server can drop the rest. */
    val attachmentsAuthoritative: Boolean = false,
)

@Serializable
data class SyncPushResponse(
    val accepted: List<SyncAccepted>,
    val rejected: List<SyncRejected>,
    val errors: List<String>? = null,
    /**
     * Accepted notes referencing attachment data the server does not hold.
     * Absent on older servers; the client re-pushes blobs it has (self-heal).
     */
    val attachmentWarnings: List<AttachmentWarning>? = null,
    /**
     * Deletions refused because the note has a newer edit, which comes back on
     * the next pull. Absent on older servers, which never refuse one.
     */
    val rejectedDeletions: List<String>? = null,
    /**
     * Notes this push sent that were permanently deleted after the edit. Absent on
     * older servers, which take them or reject them instead.
     */
    val skippedDeleted: List<String>? = null,
)

/** One accepted note whose attachment references have no data on the server. */
@Serializable
data class AttachmentWarning(
    val noteId: String,
    val attachmentIds: List<String>,
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
    val serverArchived: Boolean = false,
    val serverArchivedAt: String? = null,
    val serverLocked: Boolean = false,
    val serverLockedAt: String? = null,
    val serverSyntaxLanguage: String? = null,
    val serverWordWrap: Boolean? = null,
    val serverShowPreview: Boolean? = null,
    val serverColor: String? = null,
    val serverContentHash: String? = null,
    val serverParentHash: String? = null,
    val serverHashChain: List<String>? = null,
    /** Whether the server's copy is in the bin. An older server doesn't say. */
    val serverDeleted: Boolean = false,
    val serverDeletedAt: String? = null,
    val ancestorHash: String? = null,
    val ancestorContent: String? = null,
    val ancestorTags: List<String>? = null,
)

// MARK: - Sync Pull

@Serializable
data class SyncPullRequest(
    /** Selects cursor mode, even when empty. An older server ignores it. */
    val cursor: String? = null,
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
    /** Only from a server that supports the cursor. Sent on every page if so. */
    val nextCursor: String? = null,
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

// MARK: - Wrapped Key (Envelope Encryption)

@Serializable
data class WrappedKeyResponse(
    val blob: String,           // JSON {"ciphertext":"...","iv":"..."}
    val kdfVersion: Int,
    val kdfIterations: Int,
    /** Opaque. Absent on a server without conditional writes. */
    val updatedAt: String? = null,
)

@Serializable
data class PutWrappedKeyRequest(
    val blob: String,
    val kdfVersion: Int,
    val kdfIterations: Int,
    /** False: only if the account has no key. True: whatever is there. */
    val replace: Boolean? = null,
)
