use serde::{Deserialize, Serialize};

// Sync push request
#[derive(Debug, Deserialize)]
pub struct SyncPushRequest {
    pub notes: Vec<SyncNote>,
    pub attachments: Vec<SyncAttachment>,
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
    pub deleted: bool,
    #[serde(rename = "deletedAt")]
    pub deleted_at: Option<String>,
    pub version: i64,
    #[serde(rename = "wordWrap")]
    pub word_wrap: Option<bool>,
    #[serde(rename = "syntaxLanguage")]
    pub syntax_language: Option<String>,
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
}

// Sync pull request
#[derive(Debug, Deserialize)]
pub struct SyncPullRequest {
    #[serde(rename = "lastSyncAt")]
    pub last_sync_at: Option<String>,
    #[serde(rename = "knownNoteIds")]
    pub known_note_ids: Vec<String>,
}

// Sync pull response
#[derive(Debug, Serialize)]
pub struct SyncPullResponse {
    pub notes: Vec<SyncNote>,
    pub deletions: Vec<SyncDeletion>,
    pub attachments: Vec<SyncAttachmentData>,
    #[serde(rename = "syncedAt")]
    pub synced_at: String,
}

#[derive(Debug, Serialize)]
pub struct SyncDeletion {
    pub id: String,
    #[serde(rename = "deletedAt")]
    pub deleted_at: String,
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
    #[serde(rename = "serverLastModified")]
    pub server_last_modified: String,
    #[serde(rename = "noteCount")]
    pub note_count: i64,
    #[serde(rename = "lastSyncedAt")]
    pub last_synced_at: Option<String>,
}
