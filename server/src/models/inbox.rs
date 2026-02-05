use serde::{Deserialize, Serialize};

/// API input for creating an inbox item (POST /api/v1/inbox)
#[derive(Debug, Deserialize)]
pub struct CreateInboxItemRequest {
    pub content: String,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    pub source: Option<String>,
}

/// API output after creating an inbox item
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInboxItemResponse {
    pub id: String,
    pub created_at: String,
}

/// Full inbox item for listing (API response)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemResponse {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub source: Option<String>,
    pub size_bytes: i64,
}

/// Inbox status/quota information
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxStatusResponse {
    pub count: i64,
    pub total_size_bytes: i64,
    pub max_items: i64,
    pub max_size_mb: i64,
}
