// Admin statistics endpoints
// To be implemented in Phase 2

use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::{error::AppResult, AppState};

/// Get server statistics
/// GET /api/v1/admin/stats
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // Get user statistics
    let user_stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as total,
            SUM(CASE WHEN approved = 1 THEN 1 ELSE 0 END) as approved,
            SUM(CASE WHEN approved = 0 THEN 1 ELSE 0 END) as pending,
            SUM(CASE WHEN is_active = 1 THEN 1 ELSE 0 END) as active
        FROM users
        "#
    )
    .fetch_one(&state.pool)
    .await?;

    // Get device statistics
    let device_stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as total,
            SUM(CASE WHEN is_active = 1 THEN 1 ELSE 0 END) as active
        FROM clients
        "#
    )
    .fetch_one(&state.pool)
    .await?;

    // Get note statistics
    let note_stats = sqlx::query!(
        r#"
        SELECT COUNT(*) as total
        FROM notes
        WHERE deleted = 0
        "#
    )
    .fetch_one(&state.pool)
    .await?;

    // Get storage statistics (sum of attachment sizes)
    let storage_stats = sqlx::query!(
        r#"
        SELECT COALESCE(SUM(size), 0) as total_bytes
        FROM attachments_meta
        "#
    )
    .fetch_one(&state.pool)
    .await?;

    // Get total quota (sum of all user quotas)
    let quota_stats = sqlx::query!(
        r#"
        SELECT COALESCE(SUM(storage_quota_mb), 0) as total_quota_mb
        FROM users
        "#
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "users": {
                "total": user_stats.total,
                "approved": user_stats.approved.unwrap_or(0),
                "pending": user_stats.pending.unwrap_or(0),
                "active": user_stats.active.unwrap_or(0)
            },
            "devices": {
                "total": device_stats.total,
                "active": device_stats.active.unwrap_or(0)
            },
            "notes": {
                "total": note_stats.total
            },
            "storage": {
                "totalBytes": storage_stats.total_bytes,
                "quotaBytes": quota_stats.total_quota_mb * 1024 * 1024
            }
        })),
    ))
}

/// Get audit log
/// GET /api/v1/admin/audit
pub async fn get_audit_log(
    State(_state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // TODO: Implement audit log retrieval
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "logs": []
        })),
    ))
}

/// Get note metadata (for browsing)
/// GET /api/v1/admin/notes/metadata
pub async fn get_notes_metadata(
    State(_state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // TODO: Implement note metadata retrieval
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "notes": []
        })),
    ))
}
