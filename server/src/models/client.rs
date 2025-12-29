use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Client {
    pub id: String,
    pub user_id: String,            // NEW: User ownership
    pub api_key: String,            // Hashed
    pub device_name: String,
    pub device_type: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub is_active: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    #[serde(rename = "deviceName")]
    pub device_name: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    #[serde(rename = "clientId")]
    pub client_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}
