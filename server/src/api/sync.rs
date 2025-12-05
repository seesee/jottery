use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use base64::Engine;
use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::{
        SyncAccepted, SyncAttachmentData, SyncNote, SyncPullRequest,
        SyncPullResponse, SyncPushRequest, SyncPushResponse, SyncRejected, SyncStatusResponse,
    },
    AppState,
};

// Custom extractor for authenticated client ID
pub struct ClientId(pub String);

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for ClientId
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
            .get::<String>()
            .cloned()
            .map(ClientId)
            .ok_or(AppError::Unauthorized)
    }
}

pub async fn get_status(
    State(state): State<Arc<AppState>>,
    ClientId(client_id): ClientId,
) -> AppResult<Json<SyncStatusResponse>> {

    // Get note count
    let count_result = sqlx::query!(
        "SELECT COUNT(*) as count FROM notes WHERE client_id = ?",
        client_id
    )
    .fetch_one(&state.pool)
    .await?;

    let note_count = count_result.count;

    // Get last modified timestamp
    let last_modified_result = sqlx::query!(
        "SELECT server_modified_at FROM notes WHERE client_id = ? ORDER BY server_modified_at DESC LIMIT 1",
        client_id
    )
    .fetch_optional(&state.pool)
    .await?;

    let server_last_modified = last_modified_result
        .map(|r| r.server_modified_at)
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    Ok(Json(SyncStatusResponse {
        client_id,
        server_last_modified,
        note_count: note_count as i64,
        last_synced_at: None,
    }))
}

pub async fn push(
    State(state): State<Arc<AppState>>,
    ClientId(client_id): ClientId,
    Json(push_req): Json<SyncPushRequest>,
) -> AppResult<Json<SyncPushResponse>> {

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let errors = Vec::new();

    tracing::info!(
        "Push from client {}: {} notes, {} attachments",
        client_id,
        push_req.notes.len(),
        push_req.attachments.len()
    );

    let now = chrono::Utc::now().to_rfc3339();

    for note in push_req.notes {
        // Check if note exists
        let existing = sqlx::query!(
            "SELECT modified_at, server_version FROM notes WHERE id = ? AND client_id = ?",
            note.id,
            client_id
        )
        .fetch_optional(&state.pool)
        .await?;

        let should_accept = match &existing {
            None => true, // New note
            Some(existing_note) => {
                // Last-Write-Wins: compare modifiedAt
                note.modified_at > existing_note.modified_at
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

            // Upsert note
            sqlx::query!(
                r#"
                INSERT INTO notes (
                    id, client_id, created_at, modified_at, server_modified_at,
                    content, tags, pinned, deleted, deleted_at, version, server_version,
                    word_wrap, syntax_language
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
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
                client_id,
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
                    INSERT INTO attachments_meta (id, note_id, filename, mime_type, size, created_at)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON CONFLICT(id) DO UPDATE SET
                        filename = excluded.filename,
                        mime_type = excluded.mime_type,
                        size = excluded.size
                    "#,
                    attachment_ref.id,
                    note.id,
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
            rejected.push(SyncRejected {
                id: note.id.clone(),
                reason: "Server version is newer".to_string(),
                server_modified_at: existing.unwrap().modified_at,
            });

            tracing::debug!("Rejected note: {} (conflict)", note.id);
        }
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
    ClientId(client_id): ClientId,
    Json(pull_req): Json<SyncPullRequest>,
) -> AppResult<Json<SyncPullResponse>> {

    tracing::info!(
        "Pull from client {}: lastSyncAt={:?}, {} known IDs",
        client_id,
        pull_req.last_sync_at,
        pull_req.known_note_ids.len()
    );

    // Get notes modified after lastSyncAt
    // We need to build the query string dynamically to avoid type incompatibility
    let db_notes: Vec<crate::models::Note> = if let Some(last_sync) = &pull_req.last_sync_at {
        let rows = sqlx::query!(
            "SELECT id, client_id, created_at, modified_at, server_modified_at, content, tags, pinned, deleted, deleted_at, version, server_version, word_wrap, syntax_language FROM notes WHERE client_id = ? AND server_modified_at > ? ORDER BY server_modified_at",
            client_id,
            last_sync
        )
        .fetch_all(&state.pool)
        .await?;

        rows.into_iter()
            .filter_map(|row| Some(crate::models::Note {
                id: row.id?,
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
            "SELECT id, client_id, created_at, modified_at, server_modified_at, content, tags, pinned, deleted, deleted_at, version, server_version, word_wrap, syntax_language FROM notes WHERE client_id = ? ORDER BY server_modified_at",
            client_id
        )
        .fetch_all(&state.pool)
        .await?;

        rows.into_iter()
            .filter_map(|row| Some(crate::models::Note {
                id: row.id?,
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

    let mut notes = Vec::new();
    let mut needed_attachments = Vec::new();

    for db_note in db_notes {
        // Deserialize tags
        let tags: Vec<String> = serde_json::from_str(&db_note.tags)
            .unwrap_or_default();

        // Get attachments for this note
        let db_attachments = sqlx::query!(
            "SELECT id, note_id, filename, mime_type, size, created_at FROM attachments_meta WHERE note_id = ?",
            db_note.id
        )
        .fetch_all(&state.pool)
        .await?;

        let attachments = db_attachments
            .into_iter()
            .filter_map(|a| {
                let att_id = a.id?;
                needed_attachments.push(att_id.clone());
                Some(crate::models::AttachmentRef {
                    id: att_id.clone(),
                    filename: a.filename,  // NOT NULL, so not Optional
                    mime_type: a.mime_type,  // NOT NULL, so not Optional
                    size: a.size,  // NOT NULL in schema, so not Optional
                    data: att_id, // Reference
                })
            })
            .collect();

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

    // Get attachment data
    let mut attachments_data = Vec::new();
    for att_id in needed_attachments {
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
                attachments_data.push(SyncAttachmentData {
                    id,
                    data: encoded,
                });
            }
        }
    }

    // Get deletions
    let deletions = Vec::new(); // Simplified for now

    let synced_at = chrono::Utc::now().to_rfc3339();

    tracing::info!("Pull response: {} notes, {} attachments", notes.len(), attachments_data.len());

    Ok(Json(SyncPullResponse {
        notes,
        deletions,
        attachments: attachments_data,
        synced_at,
    }))
}

pub async fn delete_note(
    State(state): State<Arc<AppState>>,
    ClientId(client_id): ClientId,
    Path(note_id): Path<String>,
) -> AppResult<StatusCode> {

    // Delete note (cascades to attachments via foreign keys)
    sqlx::query!(
        "DELETE FROM notes WHERE id = ? AND client_id = ?",
        note_id,
        client_id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("Deleted note: {} for client: {}", note_id, client_id);

    Ok(StatusCode::NO_CONTENT)
}
