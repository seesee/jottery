use serde::{Deserialize, Serialize};

/// Session model (for admin dashboard authentication)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,       // SHA-256 hash of session token
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

/// Session creation parameters
#[derive(Debug)]
pub struct CreateSessionParams {
    pub user_id: String,
    pub token: String,            // Plain token (not hashed)
    pub expires_at: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}
