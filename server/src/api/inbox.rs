use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    api::extractors::{AuthClient, InboxAuthUser},
    error::{AppError, AppResult},
    models::{
        CreateInboxItemRequest, CreateInboxItemResponse, InboxItemResponse, InboxStatusResponse,
    },
    utils::validation,
    AppState,
};

// ============================================================================
// Inbox submission (inbox token auth)
// ============================================================================

/// POST /api/v1/inbox — submit an item to the inbox
pub async fn create_item(
    State(state): State<Arc<AppState>>,
    InboxAuthUser(auth): InboxAuthUser,
    Json(req): Json<CreateInboxItemRequest>,
) -> AppResult<(StatusCode, Json<CreateInboxItemResponse>)> {
    // Validate content (not empty and within size limits)
    validation::validate_inbox_content(&req.content, &state.config)?;

    // Validate tags if provided
    if let Some(tags) = &req.tags {
        validation::validate_tags(tags, &state.config)?;
    }

    // Get user quota limits
    let user = sqlx::query!(
        "SELECT inbox_max_items, inbox_max_size_mb FROM users WHERE id = ?",
        auth.user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user quota: {}", e);
        AppError::InternalServerError
    })?;

    let max_items = user.inbox_max_items.unwrap_or(100);
    let max_size_mb = user.inbox_max_size_mb.unwrap_or(10);

    // Check current usage
    let usage = sqlx::query!(
        "SELECT COUNT(*) as count, COALESCE(SUM(size_bytes), 0) as total_size FROM inbox_items WHERE user_id = ?",
        auth.user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch inbox usage: {}", e);
        AppError::InternalServerError
    })?;

    let current_count = usage.count as i64;
    let current_size = usage.total_size as i64;

    // Calculate size of new item
    let tags_json = serde_json::to_string(&req.tags.as_deref().unwrap_or(&[]))
        .map_err(|e| AppError::InternalError(format!("Failed to serialise tags: {}", e)))?;
    let new_size = (req.content.len() + tags_json.len()) as i64;

    // Enforce count quota
    if current_count >= max_items {
        return Err(AppError::TooManyRequests(format!(
            "Inbox item limit reached ({}/{})",
            current_count, max_items
        )));
    }

    // Enforce size quota
    let max_size_bytes = max_size_mb * 1024 * 1024;
    if current_size + new_size > max_size_bytes {
        return Err(AppError::PayloadTooLarge(format!(
            "Inbox size quota exceeded ({:.1}/{} MB)",
            (current_size + new_size) as f64 / (1024.0 * 1024.0),
            max_size_mb
        )));
    }

    // Create the inbox item
    let id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"
        INSERT INTO inbox_items (id, user_id, content, tags, created_at, source, size_bytes)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
        id,
        auth.user_id,
        req.content,
        tags_json,
        created_at,
        req.source,
        new_size
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to insert inbox item: {}", e);
        AppError::InternalServerError
    })?;

    tracing::info!(
        "Inbox item created: id={}, user={}, size={}B",
        id,
        auth.user_id,
        new_size
    );

    // Broadcast SSE notification so connected clients fetch the new inbox item
    use crate::api::sse::SyncNotification;
    let _ = state.sync_broadcast.send(SyncNotification {
        user_id: auth.user_id.clone(),
        source_client_id: String::new(), // No source client for inbox submissions
    });

    Ok((
        StatusCode::CREATED,
        Json(CreateInboxItemResponse {
            id,
            created_at,
        }),
    ))
}

// ============================================================================
// Inbox management (device API key auth)
// ============================================================================

/// GET /api/v1/inbox — list all inbox items for the authenticated user
pub async fn list_items(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
) -> AppResult<Json<Vec<InboxItemResponse>>> {
    let rows = sqlx::query!(
        "SELECT id, content, tags, created_at, source, size_bytes FROM inbox_items WHERE user_id = ? ORDER BY created_at DESC",
        client_info.user_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list inbox items: {}", e);
        AppError::InternalServerError
    })?;

    let items: Vec<InboxItemResponse> = rows
        .into_iter()
        .map(|row| {
            let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap_or_default();
            InboxItemResponse {
                id: row.id,
                content: row.content,
                tags,
                created_at: row.created_at,
                source: row.source,
                size_bytes: row.size_bytes,
            }
        })
        .collect();

    Ok(Json(items))
}

/// DELETE /api/v1/inbox/:id — delete a single inbox item
pub async fn delete_item(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let result = sqlx::query!(
        "DELETE FROM inbox_items WHERE id = ? AND user_id = ?",
        id,
        client_info.user_id
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete inbox item: {}", e);
        AppError::InternalServerError
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Inbox item not found".to_string()));
    }

    tracing::info!("Inbox item deleted: id={}, user={}", id, client_info.user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/inbox — delete all inbox items for the authenticated user
pub async fn delete_all(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
) -> AppResult<StatusCode> {
    let result = sqlx::query!(
        "DELETE FROM inbox_items WHERE user_id = ?",
        client_info.user_id
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete all inbox items: {}", e);
        AppError::InternalServerError
    })?;

    tracing::info!(
        "All inbox items deleted: user={}, count={}",
        client_info.user_id,
        result.rows_affected()
    );
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/v1/inbox/status — get inbox quota status
pub async fn get_status(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
) -> AppResult<Json<InboxStatusResponse>> {
    // Get usage
    let usage = sqlx::query!(
        "SELECT COUNT(*) as count, COALESCE(SUM(size_bytes), 0) as total_size FROM inbox_items WHERE user_id = ?",
        client_info.user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch inbox usage: {}", e);
        AppError::InternalServerError
    })?;

    // Get limits
    let user = sqlx::query!(
        "SELECT inbox_max_items, inbox_max_size_mb FROM users WHERE id = ?",
        client_info.user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user quota: {}", e);
        AppError::InternalServerError
    })?;

    Ok(Json(InboxStatusResponse {
        count: usage.count as i64,
        total_size_bytes: usage.total_size as i64,
        max_items: user.inbox_max_items.unwrap_or(100),
        max_size_mb: user.inbox_max_size_mb.unwrap_or(10),
    }))
}
