use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Note {
    pub id: String,
    pub client_id: Option<String>, // First device to push it; None once deleted
    pub created_at: String,
    pub modified_at: String,
    pub server_modified_at: String,
    pub content: String,
    pub tags: String, // JSON string
    pub pinned: i64,
    pub archived: i64,
    pub archived_at: Option<String>,
    pub locked: i64,
    pub locked_at: Option<String>,
    pub deleted: i64,
    pub deleted_at: Option<String>,
    pub version: i64,
    pub server_version: i64,
    pub word_wrap: Option<i64>,
    pub syntax_language: Option<String>,
    pub show_preview: Option<i64>,
    pub color: Option<String>,
    // Hash chain fields for git-like conflict detection
    pub content_hash: Option<String>,
    pub parent_hash: Option<String>,
    pub hash_chain: Option<String>, // JSON array of ancestor hashes
}
