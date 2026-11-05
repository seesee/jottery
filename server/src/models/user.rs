use serde::{Deserialize, Serialize};

/// User account model
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: String,
    pub approved: i64,               // SQLite uses INTEGER for BOOLEAN (0/1)
    pub approved_at: Option<String>,
    pub approved_by: Option<String>,
    pub is_active: i64,              // 0/1
    pub is_admin: i64,               // 0/1
    pub storage_quota_mb: Option<i64>, // Can be NULL in DB (has DEFAULT but no NOT NULL)
    pub max_upload_size_mb: Option<i64>, // Per-request upload limit (default: 5MB)
    pub last_login_at: Option<String>,
    pub inbox_token_hash: Option<String>,
    pub inbox_max_items: Option<i64>,
    pub inbox_max_size_mb: Option<i64>,
    pub must_change_password: i64,   // 0/1
    pub storage_used_bytes: i64,     // Approximate between housekeeping passes
}

/// User registration request (email + password)
#[derive(Debug, Deserialize)]
pub struct RegisterUserRequest {
    pub email: String,
    pub password: String,
}

/// User registration response
#[derive(Debug, Serialize)]
pub struct RegisterUserResponse {
    #[serde(rename = "userId")]
    pub user_id: String,
    pub email: String,
    pub status: String, // "pending_approval" or "approved"
    pub message: String,
}

/// Device registration request (updated with user auth)
#[derive(Debug, Deserialize)]
pub struct RegisterDeviceRequest {
    pub email: String,
    pub password: String,
    #[serde(rename = "deviceName")]
    pub device_name: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
}

/// Device registration response
#[derive(Debug, Serialize)]
pub struct RegisterDeviceResponse {
    #[serde(rename = "clientId")]
    pub client_id: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "deviceName")]
    pub device_name: String,
}

/// Clone device request (using existing API key to register a new device)
/// Used when importing sync credentials on a new device
#[derive(Debug, Deserialize)]
pub struct CloneDeviceRequest {
    /// Existing valid API key from another device
    #[serde(rename = "apiKey")]
    pub api_key: String,
    /// Name for the new device
    #[serde(rename = "deviceName")]
    pub device_name: String,
    /// Type of device (web, tui, etc.)
    #[serde(rename = "deviceType")]
    pub device_type: String,
}

/// Clone device response (same as register device response)
pub type CloneDeviceResponse = RegisterDeviceResponse;

/// Wrapped key request (for envelope encryption)
#[derive(Debug, Deserialize)]
pub struct PutWrappedKeyRequest {
    pub blob: String,
    #[serde(rename = "kdfVersion")]
    pub kdf_version: i32,
    #[serde(rename = "kdfIterations")]
    pub kdf_iterations: i32,
    /// `true` overwrites the stored key, and `false` writes only if there is none.
    /// Released clients don't send it. See docs/sync-protocol.md.
    #[serde(default)]
    pub replace: Option<bool>,
}

/// Wrapped key response (for envelope encryption)
#[derive(Debug, Serialize)]
pub struct WrappedKeyResponse {
    pub blob: String,
    #[serde(rename = "kdfVersion")]
    pub kdf_version: i32,
    #[serde(rename = "kdfIterations")]
    pub kdf_iterations: i32,
    /// What a client sends as `If-Match` to write only over this key.
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// Admin login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    /// Ask for the session as an HttpOnly cookie. See `api::cookies`.
    #[serde(rename = "cookieSession", default)]
    pub cookie_session: bool,
}

/// Admin login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// Absent when the session is in a cookie.
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Present, and true, only when the session is in a cookie.
    #[serde(rename = "cookieSession", skip_serializing_if = "std::ops::Not::not")]
    pub cookie_session: bool,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    pub user: UserInfo,
    /// The session can only change the password until this is cleared.
    #[serde(rename = "mustChangePassword")]
    pub must_change_password: bool,
}

/// User info (for login response)
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
}

/// User list item (for admin dashboard)
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct UserListItem {
    pub id: String,
    pub email: String,
    pub approved: bool,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "deviceCount")]
    pub device_count: i64,
    #[serde(rename = "noteCount")]
    pub note_count: i64,
}

/// User detail (for admin dashboard)
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct UserDetail {
    pub id: String,
    pub email: String,
    pub approved: bool,
    #[serde(rename = "approvedAt")]
    pub approved_at: Option<String>,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "lastLoginAt")]
    pub last_login_at: Option<String>,
    #[serde(rename = "storageQuotaMb")]
    pub storage_quota_mb: i64,
    #[serde(rename = "maxUploadSizeMb")]
    pub max_upload_size_mb: i64,
}
