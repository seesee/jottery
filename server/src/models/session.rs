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
    /// 1 when the session has completed any required second factor; 0
    /// when the user has enrolled passkeys but only presented their
    /// password so far. The normal auth middleware rejects
    /// `mfa_verified = 0` sessions on everything except the passkey
    /// authentication endpoints. See migration 015.
    pub mfa_verified: i64,
}

/// Session creation parameters
#[derive(Debug)]
pub struct CreateSessionParams {
    pub user_id: String,
    pub token: String,            // Plain token (not hashed)
    pub expires_at: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    /// When true, the session starts in MFA-pending state (mfa_verified=0).
    /// Used by the login handler when the user has passkeys enrolled — the
    /// session exists but can only call passkey-auth endpoints until the
    /// assertion completes. Defaults to false (fully verified) everywhere
    /// else, which matches the column default in the DB.
    pub mfa_pending: bool,
}
