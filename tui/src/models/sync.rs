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
    pub sync_enabled: bool,
    pub sync_endpoint: String,
    pub auto_sync_interval: Option<i32>, // Minutes (0 = disabled, default: 5)
}

impl Default for SyncMetadata {
    fn default() -> Self {
        Self {
            last_sync_at: None,
            last_push_at: None,
            last_pull_at: None,
            api_key: None,
            client_id: None,
            sync_enabled: false,
            sync_endpoint: String::new(),
            auto_sync_interval: Some(5),
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

impl Default for SyncStatusDisplay {
    fn default() -> Self {
        Self {
            is_enabled: false,
            is_syncing: false,
            last_sync_at: None,
            last_error: None,
            pending_notes: 0,
            conflict_count: 0,
            client_id: None,
            sync_endpoint: None,
        }
    }
}

/// Push request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPushRequest {
    pub notes: Vec<SyncNote>,
    pub attachments: Vec<SyncAttachment>,
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
    pub deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: i32,
    pub word_wrap: Option<bool>,
    pub syntax_language: Option<String>,
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

/// Rejected note info (conflict)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRejected {
    pub id: String,
    pub reason: String,
    pub server_modified_at: DateTime<Utc>,
}

/// Pull request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPullRequest {
    pub last_sync_at: Option<DateTime<Utc>>,
    pub known_note_ids: Vec<String>,
}

/// Pull response from server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPullResponse {
    pub notes: Vec<SyncNote>,
    pub deletions: Vec<SyncDeletion>,
    pub attachments: Vec<SyncAttachment>,
    pub synced_at: DateTime<Utc>,
}

/// Deleted note info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDeletion {
    pub id: String,
    pub deleted_at: DateTime<Utc>,
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
}
