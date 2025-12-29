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
    pub last_login_at: Option<String>,
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

/// Admin login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Admin login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    pub user: UserInfo,
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
}
