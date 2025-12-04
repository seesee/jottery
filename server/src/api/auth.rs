use axum::{extract::State, http::StatusCode, Json};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::AppResult,
    models::{RegisterRequest, RegisterResponse},
    AppState,
};

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<RegisterResponse>)> {
    // Generate client ID
    let client_id = Uuid::new_v4().to_string();

    // Generate random API key (64 hex characters)
    let api_key = generate_api_key();

    // Hash API key for storage
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let hashed_key = format!("{:x}", hasher.finalize());

    // Current timestamp
    let now = chrono::Utc::now().to_rfc3339();

    // Insert into database
    sqlx::query!(
        r#"
        INSERT INTO clients (id, api_key, device_name, device_type, created_at, last_seen_at, is_active)
        VALUES (?, ?, ?, ?, ?, ?, 1)
        "#,
        client_id,
        hashed_key,
        req.device_name,
        req.device_type,
        now,
        now
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("Registered new client: {} ({})", client_id, req.device_name);

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            api_key,
            client_id,
            created_at: now,
        }),
    ))
}

fn generate_api_key() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    hex::encode(bytes)
}
