// Admin statistics endpoints
// To be implemented in Phase 2

use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::{error::AppResult, AppState};

/// Get server statistics
/// GET /api/v1/admin/stats
pub async fn get_stats(
    State(_state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // TODO: Implement stats gathering
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "users": {
                "total": 0,
                "approved": 0,
                "pending": 0,
                "active": 0
            },
            "devices": {
                "total": 0,
                "active": 0
            },
            "notes": {
                "total": 0
            },
            "storage": {
                "totalBytes": 0,
                "quotaBytes": 0
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
