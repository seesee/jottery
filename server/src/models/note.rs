use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Note {
    pub id: String,
    pub client_id: String,
    pub created_at: String,
    pub modified_at: String,
    pub server_modified_at: String,
    pub content: String,
    pub tags: String, // JSON string
    pub pinned: i64,
    pub deleted: i64,
    pub deleted_at: Option<String>,
    pub version: i64,
    pub server_version: i64,
    pub word_wrap: Option<i64>,
    pub syntax_language: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub id: String,
    pub note_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AttachmentData {
    pub id: String,
    pub data: Vec<u8>,
    pub created_at: String,
}
