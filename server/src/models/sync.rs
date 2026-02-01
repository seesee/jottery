use serde::{Deserialize, Serialize};

// Sync push request
#[derive(Debug, Deserialize)]
pub struct SyncPushRequest {
    pub notes: Vec<SyncNote>,
    pub attachments: Vec<SyncAttachment>,
    pub versions: Vec<SyncNoteVersion>,
    #[serde(default)]
    #[serde(rename = "savedSearches")]
    pub saved_searches: Vec<super::saved_search::SyncSavedSearch>,
    /// Hard-deleted note IDs from client (when emptying trash)
    #[serde(default)]
    pub deletions: Vec<SyncDeletion>,
}

/// Hard deletion record for sync
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeletion {
    pub id: String,
    pub deleted_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SyncNote {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "modifiedAt")]
    pub modified_at: String,
    pub content: String,
    pub tags: Vec<String>,
    pub attachments: Vec<AttachmentRef>,
    pub pinned: bool,
    pub archived: bool,
    #[serde(rename = "archivedAt")]
    pub archived_at: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(rename = "lockedAt")]
    pub locked_at: Option<String>,
    pub deleted: bool,
    #[serde(rename = "deletedAt")]
    pub deleted_at: Option<String>,
    pub version: i64,
    #[serde(rename = "wordWrap")]
    pub word_wrap: Option<bool>,
    #[serde(rename = "syntaxLanguage")]
    pub syntax_language: Option<String>,
    #[serde(rename = "showPreview")]
    pub show_preview: Option<bool>,
    pub color: Option<String>,
    // Hash chain fields for git-like conflict detection
    #[serde(rename = "contentHash")]
    pub content_hash: Option<String>,
    #[serde(rename = "parentHash")]
    pub parent_hash: Option<String>,
    #[serde(rename = "hashChain")]
    pub hash_chain: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AttachmentRef {
    pub id: String,
    pub filename: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub size: i64,
    pub data: String, // Reference ID
}

#[derive(Debug, Deserialize)]
pub struct SyncAttachment {
    pub id: String,
    pub data: String, // Base64
}

// Sync push response
#[derive(Debug, Serialize)]
pub struct SyncPushResponse {
    pub accepted: Vec<SyncAccepted>,
    pub rejected: Vec<SyncRejected>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncAccepted {
    pub id: String,
    #[serde(rename = "serverVersion")]
    pub server_version: i64,
    #[serde(rename = "syncedAt")]
    pub synced_at: String,
}

#[derive(Debug, Serialize)]
pub struct SyncRejected {
    pub id: String,
    pub reason: String,
    #[serde(rename = "serverModifiedAt")]
    pub server_modified_at: String,
    // Server note data for conflict resolution
    #[serde(rename = "serverContent")]
    pub server_content: String,
    #[serde(rename = "serverTags")]
    pub server_tags: Vec<String>,
    #[serde(rename = "serverVersion")]
    pub server_version: i64,
    #[serde(rename = "serverAttachments")]
    pub server_attachments: Vec<AttachmentRef>,
    #[serde(rename = "serverPinned")]
    pub server_pinned: bool,
    #[serde(rename = "serverArchived")]
    pub server_archived: bool,
    #[serde(rename = "serverArchivedAt")]
    pub server_archived_at: Option<String>,
    #[serde(rename = "serverLocked")]
    pub server_locked: bool,
    #[serde(rename = "serverLockedAt")]
    pub server_locked_at: Option<String>,
    #[serde(rename = "serverSyntaxLanguage")]
    pub server_syntax_language: Option<String>,
    #[serde(rename = "serverWordWrap")]
    pub server_word_wrap: Option<bool>,
    #[serde(rename = "serverShowPreview")]
    pub server_show_preview: Option<bool>,
    #[serde(rename = "serverColor")]
    pub server_color: Option<String>,
    // Hash chain fields for git-like conflict detection
    #[serde(rename = "serverContentHash")]
    pub server_content_hash: Option<String>,
    #[serde(rename = "serverParentHash")]
    pub server_parent_hash: Option<String>,
    #[serde(rename = "serverHashChain")]
    pub server_hash_chain: Option<Vec<String>>,
    // Ancestor data for 3-way merge (if divergence point found)
    #[serde(rename = "ancestorHash", skip_serializing_if = "Option::is_none")]
    pub ancestor_hash: Option<String>,
    #[serde(rename = "ancestorContent", skip_serializing_if = "Option::is_none")]
    pub ancestor_content: Option<String>,
    #[serde(rename = "ancestorTags", skip_serializing_if = "Option::is_none")]
    pub ancestor_tags: Option<Vec<String>>,
}

// Sync pull request
#[derive(Debug, Deserialize)]
pub struct SyncPullRequest {
    #[serde(rename = "lastSyncAt")]
    pub last_sync_at: Option<String>,
    #[serde(rename = "knownNoteIds")]
    pub known_note_ids: Vec<String>,
    #[serde(rename = "knownAttachmentIds")]
    pub known_attachment_ids: Vec<String>,
    // Pagination support for large datasets
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

// Sync pull response
#[derive(Debug, Serialize)]
pub struct SyncPullResponse {
    pub notes: Vec<SyncNote>,
    pub deletions: Vec<SyncDeletion>,
    pub attachments: Vec<SyncAttachmentData>,
    pub versions: Vec<SyncNoteVersion>,
    #[serde(rename = "savedSearches")]
    pub saved_searches: Vec<super::saved_search::SyncSavedSearch>,
    #[serde(rename = "syncedAt")]
    pub synced_at: String,
    // Pagination metadata for large datasets
    #[serde(rename = "totalCount")]
    pub total_count: i64,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncAttachmentData {
    pub id: String,
    pub data: String, // Base64
}

// Sync status response
#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    #[serde(rename = "clientId")]
    pub client_id: String,
    #[serde(rename = "deviceName")]
    pub device_name: String,
    #[serde(rename = "serverLastModified")]
    pub server_last_modified: String,
    #[serde(rename = "noteCount")]
    pub note_count: i64,
    #[serde(rename = "lastSyncedAt")]
    pub last_synced_at: Option<String>,
}

// Note version for sync
#[derive(Debug, Deserialize, Serialize)]
pub struct SyncNoteVersion {
    #[serde(rename = "versionKey")]
    pub version_key: String,        // `${noteId}:${version}`
    #[serde(rename = "noteId")]
    pub note_id: String,
    pub version: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "syncedAt")]
    pub synced_at: String,
    pub content: String,            // Encrypted JSON string
    pub tags: Vec<String>,          // Array of encrypted JSON strings
    pub attachments: Vec<AttachmentRef>,
    #[serde(rename = "syntaxLanguage")]
    pub syntax_language: Option<String>,
    #[serde(rename = "wordWrap")]
    pub word_wrap: Option<bool>,
    #[serde(rename = "showPreview")]
    pub show_preview: Option<bool>,
    pub color: Option<String>,
    pub reason: String,             // 'sync' or 'manual-sync'
    // Hash chain fields for git-like conflict detection
    #[serde(rename = "contentHash")]
    pub content_hash: Option<String>,
    #[serde(rename = "parentHash")]
    pub parent_hash: Option<String>,
}
