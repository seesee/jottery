use serde::{Deserialize, Serialize};

/// Database model for saved searches
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SavedSearch {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub query: String,
    pub order: i64,
    pub created_at: String,
    pub modified_at: String,
    pub synced_at: Option<String>,
    pub deleted: i64,
    pub deleted_at: Option<String>,
    pub version: i64,
}

/// Sync model for saved searches (client-server communication)
#[derive(Debug, Deserialize, Serialize)]
pub struct SyncSavedSearch {
    pub id: String,
    pub name: String,
    pub query: String,
    pub order: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "modifiedAt")]
    pub modified_at: String,
    pub deleted: bool,
    #[serde(rename = "deletedAt")]
    pub deleted_at: Option<String>,
    pub version: i64,
}

impl From<SavedSearch> for SyncSavedSearch {
    fn from(db: SavedSearch) -> Self {
        Self {
            id: db.id,
            name: db.name,
            query: db.query,
            order: db.order,
            created_at: db.created_at,
            modified_at: db.modified_at,
            deleted: db.deleted != 0,
            deleted_at: db.deleted_at,
            version: db.version,
        }
    }
}
