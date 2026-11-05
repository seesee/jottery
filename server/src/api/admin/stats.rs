// Admin statistics endpoints
// To be implemented in Phase 2

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::{
    audit::{self, AuditFilter},
    error::{AppError, AppResult},
    AppState,
};

const DEFAULT_AUDIT_LIMIT: i64 = 100;
const MAX_AUDIT_LIMIT: i64 = 500;

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

/// Filters for the audit log. Every one is optional.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogQuery {
    pub admin_id: Option<String>,
    pub target_id: Option<String>,
    pub action: Option<String>,
    /// RFC 3339, inclusive
    pub from: Option<String>,
    /// RFC 3339, inclusive
    pub to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Times are stored in one UTC format and compared as text, so a filter has to be
/// in that format too.
fn stored_time(value: Option<String>, name: &str) -> AppResult<Option<String>> {
    value
        .map(|time| {
            chrono::DateTime::parse_from_rfc3339(&time)
                .map(|time| time.with_timezone(&chrono::Utc).to_rfc3339())
                .map_err(|_| AppError::BadRequest(format!("{name} must be an RFC 3339 time")))
        })
        .transpose()
}

/// Get audit log, newest first
/// GET /api/v1/admin/audit
pub async fn get_audit_log(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditLogQuery>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let filter = AuditFilter {
        admin_user_id: query.admin_id,
        target_user_id: query.target_id,
        action: query.action,
        from: stored_time(query.from, "from")?,
        to: stored_time(query.to, "to")?,
    };
    let limit = query.limit.unwrap_or(DEFAULT_AUDIT_LIMIT).clamp(1, MAX_AUDIT_LIMIT);
    let offset = query.offset.unwrap_or(0).max(0);

    let (entries, total) = audit::list(&state.pool, &filter, limit, offset).await?;

    let logs: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|entry| {
            let metadata = entry
                .metadata_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());
            let mut value = serde_json::json!(entry);
            value["metadata"] = metadata.unwrap_or(serde_json::Value::Null);
            value
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "logs": logs,
            "total": total
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
