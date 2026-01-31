#![allow(dead_code)]
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Sync metadata stored in the database
/// Contains global sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMetadata {
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_push_at: Option<DateTime<Utc>>,
    pub last_pull_at: Option<DateTime<Utc>>,
    pub api_key: Option<String>,        // Encrypted API key (JSON stringified EncryptedData)
    pub client_id: Option<String>,      // UUID assigned by server
    pub user_email: Option<String>,     // User email (for multi-user auth)
    pub sync_enabled: bool,
    pub sync_endpoint: String,
    pub auto_sync_interval: Option<i32>, // Minutes (0 = disabled, default: 1)
    pub pending_registration_email: Option<String>, // Email of pending registration awaiting approval
    #[serde(default)]
    pub pending_device_name: Option<String>, // Device name for pending clone-device call (during import)
}

impl Default for SyncMetadata {
    fn default() -> Self {
        Self {
            last_sync_at: None,
            last_push_at: None,
            last_pull_at: None,
            api_key: None,
            client_id: None,
            user_email: None,
            sync_enabled: false,
            sync_endpoint: String::new(),
            auto_sync_interval: Some(1),
            pending_registration_email: None,
            pending_device_name: None,
        }
    }
}

/// Per-note sync tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSyncMetadata {
    pub note_id: String,
    pub synced_at: DateTime<Utc>,
    pub sync_hash: String,               // SHA-256 of encrypted content
    pub server_version: i32,
    pub last_sync_status: SyncStatus,
    pub error_message: Option<String>,
    pub conflict_data: Option<ConflictData>, // Server version data when in conflict
}

/// Sync status for a note
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Synced,
    Pending,
    Conflict,
    Error,
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Synced => write!(f, "synced"),
            Self::Pending => write!(f, "pending"),
            Self::Conflict => write!(f, "conflict"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Current sync status for UI display
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct SyncStatusDisplay {
    pub is_enabled: bool,
    pub is_syncing: bool,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub pending_notes: usize,
    pub conflict_count: usize,
    pub client_id: Option<String>,
    pub sync_endpoint: Option<String>,
}


/// Push request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPushRequest {
    pub notes: Vec<SyncNote>,
    pub attachments: Vec<SyncAttachment>,
    pub versions: Vec<SyncNoteVersion>,
    /// Hard-deleted note IDs (from emptying trash)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deletions: Vec<SyncDeletion>,
}

impl Default for SyncPushRequest {
    fn default() -> Self {
        Self {
            notes: Vec::new(),
            attachments: Vec::new(),
            versions: Vec::new(),
            deletions: Vec::new(),
        }
    }
}

/// Note structure for sync (matches server expectations)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncNote {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub content: String,              // Encrypted JSON string
    pub tags: Vec<String>,            // Array of encrypted JSON strings
    pub attachments: Vec<AttachmentRef>,
    pub pinned: bool,
    pub archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub locked: bool,
    pub locked_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: i32,
    pub word_wrap: Option<bool>,
    pub syntax_language: Option<String>,
    pub color: Option<String>,        // Semantic color name (unencrypted)
}

/// Attachment reference (metadata only)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentRef {
    pub id: String,
    pub filename: String,             // Encrypted
    pub mime_type: String,
    pub size: i64,
    pub data: String,                 // Reference ID
}

/// Attachment with binary data (for sync transfer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAttachment {
    pub id: String,
    pub data: String,                 // Base64 encoded encrypted blob
}

/// Push response from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPushResponse {
    pub accepted: Vec<SyncAccepted>,
    pub rejected: Vec<SyncRejected>,
    pub errors: Vec<String>,
}

/// Accepted note info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAccepted {
    pub id: String,
    pub server_version: i32,
    pub synced_at: DateTime<Utc>,
}

/// Rejected note info (conflict) - includes full server note data for resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRejected {
    pub id: String,
    pub reason: String,
    pub server_modified_at: DateTime<Utc>,
    // Server note data for conflict resolution
    pub server_content: String,           // Encrypted content
    pub server_tags: Vec<String>,         // Encrypted tags
    pub server_version: i32,
    pub server_attachments: Vec<AttachmentRef>,
    pub server_pinned: bool,
    pub server_archived: bool,
    pub server_archived_at: Option<DateTime<Utc>>,
    pub server_locked: bool,
    pub server_locked_at: Option<DateTime<Utc>>,
    pub server_syntax_language: Option<String>,
    pub server_word_wrap: Option<bool>,
    pub server_color: Option<String>,     // Semantic color name
}

/// Stored conflict data for resolution UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictData {
    pub note_id: String,
    pub server_content: String,           // Encrypted content
    pub server_tags: Vec<String>,         // Encrypted tags
    pub server_modified_at: DateTime<Utc>,
    pub server_version: i32,
    pub server_attachments: Vec<AttachmentRef>,
    pub server_pinned: bool,
    pub server_archived: bool,
    pub server_archived_at: Option<DateTime<Utc>>,
    pub server_locked: bool,
    pub server_locked_at: Option<DateTime<Utc>>,
    pub server_syntax_language: Option<String>,
    pub server_word_wrap: Option<bool>,
    pub server_color: Option<String>,     // Semantic color name
    pub detected_at: DateTime<Utc>,       // When conflict was detected
}

/// Pull request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPullRequest {
    pub last_sync_at: Option<DateTime<Utc>>,
    pub known_note_ids: Vec<String>,
    pub known_attachment_ids: Vec<String>,
}

/// Pull response from server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPullResponse {
    pub notes: Vec<SyncNote>,
    pub deletions: Vec<SyncDeletion>,
    pub attachments: Vec<SyncAttachment>,
    pub versions: Vec<SyncNoteVersion>,
    pub synced_at: DateTime<Utc>,
}

/// Deleted note info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeletion {
    pub id: String,
    pub deleted_at: DateTime<Utc>,
}

/// Note version for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncNoteVersion {
    pub version_key: String,        // `${noteId}:${version}`
    pub note_id: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub synced_at: DateTime<Utc>,
    pub content: String,           // Encrypted JSON string
    pub tags: Vec<String>,         // Array of encrypted JSON strings
    pub attachments: Vec<AttachmentRef>,
    pub syntax_language: Option<String>,
    pub word_wrap: Option<bool>,
    pub show_preview: Option<bool>, // Whether to show preview in version history
    pub color: Option<String>,     // Semantic color name (unencrypted)
    pub reason: String,            // 'sync' or 'manual-sync'
}

/// Server status response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusResponse {
    pub client_id: String,
    pub server_last_modified: DateTime<Utc>,
    pub note_count: i32,
    pub last_synced_at: Option<DateTime<Utc>>,
}

/// Authentication/registration types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRegisterRequest {
    pub device_name: String,
    pub device_type: DeviceType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Web,
    Cli,
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Web => write!(f, "web"),
            Self::Cli => write!(f, "cli"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRegisterResponse {
    pub api_key: String,
    pub client_id: String,
    pub created_at: DateTime<Utc>,
}

/// Sync credentials payload for clipboard sharing
/// This is what gets encoded to base64 and shared between devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCredentials {
    pub endpoint: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "clientId")]
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salt: Option<String>,  // Optional, may be present from web app
}

impl SyncCredentials {
    /// Create new sync credentials
    pub fn new(endpoint: String, api_key: String, client_id: String) -> Self {
        Self {
            endpoint,
            api_key,
            client_id,
            salt: None,  // Salt is not needed for TUI (uses database encryption)
        }
    }

    /// Encode credentials to base64 JSON string
    pub fn to_base64(&self) -> anyhow::Result<String> {
        let json = serde_json::to_string(self)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(json.as_bytes()))
    }

    /// Decode credentials from base64 JSON string
    pub fn from_base64(encoded: &str) -> anyhow::Result<Self> {
        let decoded = base64::engine::general_purpose::STANDARD.decode(encoded)?;
        let json = String::from_utf8(decoded)?;
        let creds: Self = serde_json::from_str(&json)?;
        Ok(creds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_metadata_defaults() {
        let metadata = SyncMetadata::default();
        assert!(!metadata.sync_enabled);
        assert!(metadata.sync_endpoint.is_empty());
        assert!(metadata.api_key.is_none());
        assert!(metadata.client_id.is_none());
        assert!(metadata.last_sync_at.is_none());
        assert_eq!(metadata.auto_sync_interval, Some(1)); // Default to 1 minute
    }

    #[test]
    fn test_credentials_roundtrip() {
        let creds = SyncCredentials::new(
            "https://example.com/api".to_string(),
            "test-api-key-12345".to_string(),
            "client-id-abcde".to_string(),
        );

        let encoded = creds.to_base64().unwrap();
        let decoded = SyncCredentials::from_base64(&encoded).unwrap();

        assert_eq!(decoded.endpoint, creds.endpoint);
        assert_eq!(decoded.api_key, creds.api_key);
        assert_eq!(decoded.client_id, creds.client_id);
    }

    #[test]
    fn test_credentials_from_invalid_base64() {
        let result = SyncCredentials::from_base64("invalid!!!base64");
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_note_includes_color_field() {
        // Test that SyncNote struct serializes with color field
        let sync_note = SyncNote {
            id: "test-id".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            content: "encrypted-content".to_string(),
            tags: vec!["tag1".to_string()],
            attachments: vec![],
            pinned: false,
            deleted: false,
            deleted_at: None,
            version: 1,
            word_wrap: Some(true),
            syntax_language: Some("markdown".to_string()),
            archived: false,
            archived_at: None,
            locked: false,
            locked_at: None,
            color: Some("red".to_string()),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&sync_note).expect("Failed to serialize");

        // Verify color field is present in JSON
        assert!(json.contains("\"color\""), "JSON should contain color field");
        assert!(json.contains("\"red\""), "JSON should contain red color value");
    }

    #[test]
    fn test_sync_note_color_none() {
        // Test that SyncNote with no color serializes correctly
        let sync_note = SyncNote {
            id: "test-id".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            content: "encrypted-content".to_string(),
            tags: vec![],
            attachments: vec![],
            pinned: false,
            deleted: false,
            deleted_at: None,
            version: 1,
            word_wrap: Some(true),
            syntax_language: Some("markdown".to_string()),
            color: None,
            archived: false,
            archived_at: None,
            locked: false,
            locked_at: None,
        };

        let json = serde_json::to_string(&sync_note).expect("Failed to serialize");

        // Verify color field is null or omitted in JSON
        assert!(json.contains("\"color\":null") || !json.contains("\"color\""),
                "Color should be null or omitted");
    }

    #[test]
    fn test_sync_note_version_includes_color() {
        // Test that SyncNoteVersion includes color field
        let version = SyncNoteVersion {
            version_key: "note-id:1".to_string(),
            note_id: "note-id".to_string(),
            version: 1,
            created_at: Utc::now(),
            synced_at: Utc::now(),
            content: "encrypted-content".to_string(),
            tags: vec![],
            attachments: vec![],
            syntax_language: Some("markdown".to_string()),
            word_wrap: Some(true),
            show_preview: Some(true),
            color: Some("blue".to_string()),
            reason: "sync".to_string(),
        };

        let json = serde_json::to_string(&version).expect("Failed to serialize");

        assert!(json.contains("\"color\""), "Version JSON should contain color");
        assert!(json.contains("\"blue\""), "Version JSON should contain blue color");
    }

    #[test]
    fn test_conflict_data_includes_server_color() {
        // Test that ConflictData includes server_color field
        let conflict = ConflictData {
            note_id: "note-id".to_string(),
            server_content: "server-content".to_string(),
            server_tags: vec![],
            server_modified_at: Utc::now(),
            server_version: 2,
            server_attachments: vec![],
            server_pinned: false,
            server_syntax_language: Some("markdown".to_string()),
            server_word_wrap: Some(true),
            server_color: Some("green".to_string()),
            server_archived: false,
            server_archived_at: None,
            server_locked: false,
            server_locked_at: None,
            detected_at: Utc::now(),
        };

        let json = serde_json::to_string(&conflict).expect("Failed to serialize");

        assert!(json.contains("\"serverColor\""), "Conflict JSON should contain serverColor");
        assert!(json.contains("\"green\""), "Conflict JSON should contain green color");
    }

    #[test]
    fn test_sync_rejected_includes_server_color() {
        // Test that SyncRejected includes server_color field
        let rejected = SyncRejected {
            id: "note-id".to_string(),
            reason: "conflict".to_string(),
            server_modified_at: Utc::now(),
            server_content: "server-content".to_string(),
            server_tags: vec![],
            server_version: 2,
            server_attachments: vec![],
            server_pinned: false,
            server_syntax_language: Some("markdown".to_string()),
            server_word_wrap: Some(true),
            server_color: Some("yellow".to_string()),
            server_archived: false,
            server_archived_at: None,
            server_locked: false,
            server_locked_at: None,
        };

        let json = serde_json::to_string(&rejected).expect("Failed to serialize");

        assert!(json.contains("\"serverColor\""), "Rejected JSON should contain serverColor");
        assert!(json.contains("\"yellow\""), "Rejected JSON should contain yellow color");
    }

    #[test]
    fn test_color_roundtrip_serialization() {
        // Test that color field survives JSON roundtrip
        let original = SyncNote {
            id: "test-id".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            content: "content".to_string(),
            tags: vec![],
            attachments: vec![],
            pinned: false,
            deleted: false,
            deleted_at: None,
            version: 1,
            word_wrap: Some(true),
            syntax_language: Some("markdown".to_string()),
            color: Some("purple".to_string()),
            archived: false,
            archived_at: None,
            locked: false,
            locked_at: None,
        };

        // Serialize and deserialize
        let json = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: SyncNote =
            serde_json::from_str(&json).expect("Failed to deserialize");

        // Verify color is preserved
        assert_eq!(deserialized.color, Some("purple".to_string()),
                   "Color should be preserved through roundtrip");
        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.version, original.version);
    }
}
