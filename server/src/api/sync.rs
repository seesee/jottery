use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use base64::Engine;
use std::sync::Arc;

use crate::{
    api::middleware::ClientInfo,
    error::{AppError, AppResult},
    models::{
        AttachmentRef, SyncAccepted, SyncAttachmentData, SyncNote, SyncPullRequest,
        SyncPullResponse, SyncPushRequest, SyncPushResponse, SyncRejected, SyncStatusResponse,
    },
    AppState,
};

// Custom extractor for authenticated client info (client_id + user_id)
pub struct AuthClient(pub ClientInfo);

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AuthClient
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ClientInfo>()
            .cloned()
            .map(AuthClient)
            .ok_or(AppError::Unauthorized)
    }
}

pub async fn get_status(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
) -> AppResult<Json<SyncStatusResponse>> {

    // Get note count for this user (across all their devices)
    let count_result = sqlx::query!(
        "SELECT COUNT(*) as count FROM notes WHERE user_id = ?",
        client_info.user_id
    )
    .fetch_one(&state.pool)
    .await?;

    let note_count = count_result.count;

    // Get last modified timestamp for this user
    let last_modified_result = sqlx::query!(
        "SELECT server_modified_at FROM notes WHERE user_id = ? ORDER BY server_modified_at DESC LIMIT 1",
        client_info.user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    let server_last_modified = last_modified_result
        .map(|r| r.server_modified_at)
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    Ok(Json(SyncStatusResponse {
        client_id: client_info.client_id,
        server_last_modified,
        note_count: note_count as i64,
        last_synced_at: None,
    }))
}

pub async fn push(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
    Json(push_req): Json<SyncPushRequest>,
) -> AppResult<Json<SyncPushResponse>> {

    // Get user's max upload size limit
    let user = sqlx::query!(
        "SELECT max_upload_size_mb FROM users WHERE id = ?",
        client_info.user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    let max_upload_bytes = user
        .map(|u| u.max_upload_size_mb)
        .unwrap_or(5) as usize * 1024 * 1024;

    // Estimate payload size from attachments (main contributor to size)
    let estimated_size: usize = push_req.attachments.iter()
        .map(|a| a.data.len())
        .sum::<usize>()
        + push_req.notes.iter()
            .map(|n| n.content.len())
            .sum::<usize>();

    if estimated_size > max_upload_bytes {
        let max_mb = max_upload_bytes / (1024 * 1024);
        return Err(AppError::PayloadTooLarge(format!(
            "Upload size (~{}MB) exceeds your limit of {}MB",
            estimated_size / (1024 * 1024),
            max_mb
        )));
    }

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let errors = Vec::new();

    tracing::info!(
        "Push from user {} (client {}): {} notes, {} attachments (~{}KB)",
        client_info.user_id,
        client_info.client_id,
        push_req.notes.len(),
        push_req.attachments.len(),
        estimated_size / 1024
    );

    let now = chrono::Utc::now().to_rfc3339();

    for note in push_req.notes {
        // Check if note exists for this user (across all their devices)
        let existing = sqlx::query!(
            "SELECT modified_at, server_version FROM notes WHERE id = ? AND user_id = ?",
            note.id,
            client_info.user_id
        )
        .fetch_optional(&state.pool)
        .await?;

        let should_accept = match &existing {
            None => true, // New note
            Some(existing_note) => {
                // Last-Write-Wins: compare modifiedAt
                // Use >= to avoid false conflicts when timestamps are identical
                // (can happen if note is synced, then client re-syncs without changes)
                note.modified_at >= existing_note.modified_at
            }
        };

        if should_accept {
            // Convert types
            let pinned = if note.pinned { 1 } else { 0 };
            let deleted = if note.deleted { 1 } else { 0 };
            let word_wrap = note.word_wrap.map(|w| if w { 1 } else { 0 });

            // Serialize tags as JSON
            let tags_json = serde_json::to_string(&note.tags)
                .map_err(|e| AppError::InternalError(format!("Failed to serialize tags: {}", e)))?;

            // Calculate new server version
            let server_version = existing
                .as_ref()
                .map(|e| e.server_version + 1)
                .unwrap_or(1);

            // Upsert note (with both user_id for access control and client_id for audit)
            // Use composite primary key (id, user_id) to allow same note UUIDs across users
            sqlx::query!(
                r#"
                INSERT INTO notes (
                    id, user_id, client_id, created_at, modified_at, server_modified_at,
                    content, tags, pinned, deleted, deleted_at, version, server_version,
                    word_wrap, syntax_language
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id, user_id) DO UPDATE SET
                    modified_at = excluded.modified_at,
                    server_modified_at = excluded.server_modified_at,
                    content = excluded.content,
                    tags = excluded.tags,
                    pinned = excluded.pinned,
                    deleted = excluded.deleted,
                    deleted_at = excluded.deleted_at,
                    version = excluded.version,
                    server_version = excluded.server_version,
                    word_wrap = excluded.word_wrap,
                    syntax_language = excluded.syntax_language
                "#,
                note.id,
                client_info.user_id,
                client_info.client_id,
                note.created_at,
                note.modified_at,
                now,
                note.content,
                tags_json,
                pinned,
                deleted,
                note.deleted_at,
                note.version,
                server_version,
                word_wrap,
                note.syntax_language
            )
            .execute(&state.pool)
            .await?;

            accepted.push(SyncAccepted {
                id: note.id.clone(),
                server_version,
                synced_at: now.clone(),
            });

            // Store attachment metadata for this note
            for attachment_ref in &note.attachments {
                sqlx::query!(
                    r#"
                    INSERT INTO attachments_meta (id, note_id, note_user_id, filename, mime_type, size, created_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(id) DO UPDATE SET
                        filename = excluded.filename,
                        mime_type = excluded.mime_type,
                        size = excluded.size
                    "#,
                    attachment_ref.id,
                    note.id,
                    client_info.user_id,
                    attachment_ref.filename,
                    attachment_ref.mime_type,
                    attachment_ref.size,
                    now
                )
                .execute(&state.pool)
                .await?;

                tracing::debug!("Stored attachment metadata: {} for note {}", attachment_ref.id, note.id);
            }

            tracing::debug!("Accepted note: {}", note.id);
        } else {
            // Fetch full server note data for conflict resolution
            let server_note = sqlx::query!(
                r#"SELECT content, tags, pinned, server_version, syntax_language, word_wrap, modified_at
                   FROM notes WHERE id = ? AND user_id = ?"#,
                note.id,
                client_info.user_id
            )
            .fetch_one(&state.pool)
            .await?;

            // Fetch server attachments for this note (note ownership already verified)
            let server_attachments_rows = sqlx::query!(
                r#"SELECT id, filename, mime_type, size FROM attachments_meta WHERE note_id = ?"#,
                note.id
            )
            .fetch_all(&state.pool)
            .await?;

            let server_attachments: Vec<AttachmentRef> = server_attachments_rows
                .into_iter()
                .filter_map(|row| {
                    let id = row.id?;
                    Some(AttachmentRef {
                        id: id.clone(),
                        filename: row.filename,
                        mime_type: row.mime_type,
                        size: row.size,
                        data: id, // Reference ID same as attachment ID
                    })
                })
                .collect();

            // Parse server tags from JSON
            let server_tags: Vec<String> = serde_json::from_str(&server_note.tags)
                .unwrap_or_default();

            rejected.push(SyncRejected {
                id: note.id.clone(),
                reason: "Server version is newer".to_string(),
                server_modified_at: server_note.modified_at,
                server_content: server_note.content,
                server_tags,
                server_version: server_note.server_version,
                server_attachments,
                server_pinned: server_note.pinned == 1,
                server_syntax_language: server_note.syntax_language,
                server_word_wrap: server_note.word_wrap.map(|w| w == 1),
            });

            tracing::debug!("Rejected note: {} (conflict) - included server data for resolution", note.id);
        }
    }

    // Store note versions
    for version in push_req.versions {
        let now_for_version = chrono::Utc::now().to_rfc3339();

        // Serialize tags and attachments as JSON
        let tags_json = serde_json::to_string(&version.tags)
            .map_err(|e| AppError::InternalError(format!("Failed to serialize version tags: {}", e)))?;
        let attachments_json = serde_json::to_string(&version.attachments)
            .map_err(|e| AppError::InternalError(format!("Failed to serialize version attachments: {}", e)))?;

        // Convert optional fields
        let word_wrap = version.word_wrap.map(|w| if w { 1 } else { 0 });

        // Upsert version (insert or replace if exists)
        sqlx::query!(
            r#"
            INSERT INTO note_versions (
                version_key, note_id, user_id, client_id, version, created_at, synced_at,
                server_synced_at, content, tags, attachments, syntax_language, word_wrap, reason
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(version_key) DO UPDATE SET
                synced_at = excluded.synced_at,
                server_synced_at = excluded.server_synced_at
            "#,
            version.version_key,
            version.note_id,
            client_info.user_id,
            client_info.client_id,
            version.version,
            version.created_at,
            version.synced_at,
            now_for_version,
            version.content,
            tags_json,
            attachments_json,
            version.syntax_language,
            word_wrap,
            version.reason
        )
        .execute(&state.pool)
        .await?;

        tracing::debug!("Stored version: {} for note {}", version.version_key, version.note_id);
    }

    // Store attachment data (binary blobs)
    for attachment in push_req.attachments {
        // Decode base64
        let data = base64::engine::general_purpose::STANDARD
            .decode(&attachment.data)
            .map_err(|e| AppError::BadRequest(format!("Invalid base64: {}", e)))?;

        // Store in attachments_data
        sqlx::query!(
            r#"
            INSERT INTO attachments_data (id, data, created_at)
            VALUES (?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET data = excluded.data
            "#,
            attachment.id,
            data,
            now
        )
        .execute(&state.pool)
        .await?;

        tracing::debug!("Stored attachment: {}", attachment.id);
    }

    Ok(Json(SyncPushResponse {
        accepted,
        rejected,
        errors,
    }))
}

pub async fn pull(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
    Json(pull_req): Json<SyncPullRequest>,
) -> AppResult<Json<SyncPullResponse>> {

    // Default pagination: 100 notes per page to avoid memory issues
    let limit = pull_req.limit.unwrap_or(100);
    let offset = pull_req.offset.unwrap_or(0);

    tracing::info!(
        "Pull from user {} (client {}): lastSyncAt={:?}, {} known IDs, limit={}, offset={}",
        client_info.user_id,
        client_info.client_id,
        pull_req.last_sync_at,
        pull_req.known_note_ids.len(),
        limit,
        offset
    );

    // Get total count first (for pagination metadata)
    let total_count: i64 = if let Some(last_sync) = &pull_req.last_sync_at {
        let count_result = sqlx::query!(
            "SELECT COUNT(*) as count FROM notes WHERE user_id = ? AND server_modified_at > ?",
            client_info.user_id,
            last_sync
        )
        .fetch_one(&state.pool)
        .await?;
        count_result.count as i64
    } else {
        let count_result = sqlx::query!(
            "SELECT COUNT(*) as count FROM notes WHERE user_id = ?",
            client_info.user_id
        )
        .fetch_one(&state.pool)
        .await?;
        count_result.count as i64
    };

    // Get notes with pagination (LIMIT/OFFSET)
    let db_notes: Vec<crate::models::Note> = if let Some(last_sync) = &pull_req.last_sync_at {
        let rows = sqlx::query!(
            "SELECT id, client_id, created_at, modified_at, server_modified_at, content, tags, pinned, deleted, deleted_at, version, server_version, word_wrap, syntax_language FROM notes WHERE user_id = ? AND server_modified_at > ? ORDER BY server_modified_at LIMIT ? OFFSET ?",
            client_info.user_id,
            last_sync,
            limit,
            offset
        )
        .fetch_all(&state.pool)
        .await?;

        rows.into_iter()
            .filter_map(|row| Some(crate::models::Note {
                id: row.id,
                client_id: row.client_id,
                created_at: row.created_at,
                modified_at: row.modified_at,
                server_modified_at: row.server_modified_at,
                content: row.content,
                tags: row.tags,
                pinned: row.pinned,
                deleted: row.deleted,
                deleted_at: row.deleted_at,
                version: row.version,
                server_version: row.server_version,
                word_wrap: row.word_wrap,
                syntax_language: row.syntax_language,
            }))
            .collect()
    } else {
        let rows = sqlx::query!(
            "SELECT id, client_id, created_at, modified_at, server_modified_at, content, tags, pinned, deleted, deleted_at, version, server_version, word_wrap, syntax_language FROM notes WHERE user_id = ? ORDER BY server_modified_at LIMIT ? OFFSET ?",
            client_info.user_id,
            limit,
            offset
        )
        .fetch_all(&state.pool)
        .await?;

        rows.into_iter()
            .filter_map(|row| Some(crate::models::Note {
                id: row.id,
                client_id: row.client_id,
                created_at: row.created_at,
                modified_at: row.modified_at,
                server_modified_at: row.server_modified_at,
                content: row.content,
                tags: row.tags,
                pinned: row.pinned,
                deleted: row.deleted,
                deleted_at: row.deleted_at,
                version: row.version,
                server_version: row.server_version,
                word_wrap: row.word_wrap,
                syntax_language: row.syntax_language,
            }))
            .collect()
    };

    // Calculate if there are more pages
    let has_more = offset + (db_notes.len() as i64) < total_count;

    // Collect all note IDs for batch attachment query
    let note_ids: Vec<&str> = db_notes.iter().map(|n| n.id.as_str()).collect();

    // Fetch all attachments for all notes in a single query (avoids N+1 problem)
    let all_attachments = if !note_ids.is_empty() {
        // Build placeholders for IN clause
        let placeholders = note_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let query = format!(
            "SELECT id, note_id, filename, mime_type, size, created_at FROM attachments_meta WHERE note_id IN ({})",
            placeholders
        );

        // Execute with dynamic binding
        let mut query_builder = sqlx::query_as::<_, (Option<String>, Option<String>, String, String, i64, String)>(&query);
        for note_id in &note_ids {
            query_builder = query_builder.bind(*note_id);
        }
        query_builder.fetch_all(&state.pool).await?
    } else {
        Vec::new()
    };

    // Group attachments by note_id for O(1) lookup
    let mut attachments_by_note: std::collections::HashMap<String, Vec<crate::models::AttachmentRef>> =
        std::collections::HashMap::new();
    let mut needed_attachments = Vec::new();

    for (id, note_id, filename, mime_type, size, _created_at) in all_attachments {
        if let (Some(att_id), Some(nid)) = (id, note_id) {
            needed_attachments.push(att_id.clone());
            let attachment = crate::models::AttachmentRef {
                id: att_id.clone(),
                filename,
                mime_type,
                size,
                data: att_id,
            };
            attachments_by_note
                .entry(nid)
                .or_default()
                .push(attachment);
        }
    }

    // Build notes with pre-fetched attachments
    let mut notes = Vec::new();
    for db_note in db_notes {
        // Deserialize tags
        let tags: Vec<String> = serde_json::from_str(&db_note.tags)
            .unwrap_or_default();

        // Get attachments from pre-fetched map (O(1) lookup)
        let attachments = attachments_by_note
            .remove(&db_note.id)
            .unwrap_or_default();

        notes.push(SyncNote {
            id: db_note.id,
            created_at: db_note.created_at,
            modified_at: db_note.modified_at,
            content: db_note.content,
            tags,
            attachments,
            pinned: db_note.pinned != 0,
            deleted: db_note.deleted != 0,
            deleted_at: db_note.deleted_at,
            version: db_note.version,
            word_wrap: db_note.word_wrap.map(|w| w != 0),
            syntax_language: db_note.syntax_language,
        });
    }

    // Get attachment data - only for attachments the client doesn't already have
    let mut attachments_data = Vec::new();
    for att_id in needed_attachments {
        // Skip if client already has this attachment
        if pull_req.known_attachment_ids.contains(&att_id) {
            tracing::debug!("Skipping attachment {} (client already has it)", att_id);
            continue;
        }

        if let Some(att_data) = sqlx::query!(
            "SELECT id, data FROM attachments_data WHERE id = ?",
            att_id
        )
        .fetch_optional(&state.pool)
        .await?
        {
            if let Some(id) = att_data.id {
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&att_data.data);
                tracing::debug!("Sending attachment {} to client", id);
                attachments_data.push(SyncAttachmentData {
                    id,
                    data: encoded,
                });
            }
        }
    }

    // Get deletions
    let deletions = Vec::new(); // Simplified for now

    // Get versions for all notes from this client
    // Note: versions are still per-client (not shared across user's devices)
    // This is intentional as versions track device-specific history
    let epoch = "1970-01-01T00:00:00Z".to_string();
    let last_sync_filter = pull_req.last_sync_at.as_ref().unwrap_or(&epoch);
    let db_versions = sqlx::query!(
        "SELECT version_key, note_id, version, created_at, synced_at, content, tags, attachments, syntax_language, word_wrap, reason
         FROM note_versions
         WHERE client_id = ? AND (? = '1970-01-01T00:00:00Z' OR server_synced_at > ?)
         ORDER BY note_id, version",
        client_info.client_id,
        last_sync_filter,
        last_sync_filter
    )
    .fetch_all(&state.pool)
    .await?;

    // Convert to SyncNoteVersion
    let mut versions_response = Vec::new();
    for db_version in db_versions {
        // Deserialize tags
        let tags: Vec<String> = serde_json::from_str(&db_version.tags).unwrap_or_default();

        // Deserialize attachments
        let attachments: Vec<crate::models::AttachmentRef> = serde_json::from_str(&db_version.attachments).unwrap_or_default();

        versions_response.push(crate::models::SyncNoteVersion {
            version_key: db_version.version_key,
            note_id: db_version.note_id,
            version: db_version.version,
            created_at: db_version.created_at,
            synced_at: db_version.synced_at,
            content: db_version.content,
            tags,
            attachments,
            syntax_language: db_version.syntax_language,
            word_wrap: db_version.word_wrap.map(|w| w != 0),
            reason: db_version.reason,
        });
    }

    let synced_at = chrono::Utc::now().to_rfc3339();

    tracing::info!(
        "Pull response: {} notes (total: {}, hasMore: {}), {} attachments, {} versions",
        notes.len(),
        total_count,
        has_more,
        attachments_data.len(),
        versions_response.len()
    );

    Ok(Json(SyncPullResponse {
        notes,
        deletions,
        attachments: attachments_data,
        versions: versions_response,
        synced_at,
        total_count,
        has_more,
    }))
}

pub async fn delete_note(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
    Path(note_id): Path<String>,
) -> AppResult<StatusCode> {

    // Delete note for this user (cascades to attachments via foreign keys)
    // Filter by user_id to ensure users can only delete their own notes
    sqlx::query!(
        "DELETE FROM notes WHERE id = ? AND user_id = ?",
        note_id,
        client_info.user_id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "Deleted note: {} for user: {} (client: {})",
        note_id,
        client_info.user_id,
        client_info.client_id
    );

    Ok(StatusCode::NO_CONTENT)
}
