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

    // Get device name for this client
    let client_result = sqlx::query!(
        "SELECT device_name FROM clients WHERE id = ?",
        client_info.client_id
    )
    .fetch_optional(&state.pool)
    .await?;

    let device_name = client_result
        .map(|r| r.device_name)
        .unwrap_or_else(|| "Unknown Device".to_string());

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
        device_name,
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

    // Clean up notes that have been deleted for more than 30 days
    // This prevents deleted notes from accumulating indefinitely
    if let Err(e) = cleanup_old_deleted_notes(&state.pool).await {
        tracing::warn!("Failed to cleanup old deleted notes: {}", e);
    }

    let now = chrono::Utc::now().to_rfc3339();

    for note in push_req.notes {
        // Check if this note was hard-deleted by another device
        // If so, skip it - the client will receive the deletion in the pull response
        let deletion_count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM note_deletions WHERE id = ? AND user_id = ?",
            note.id,
            client_info.user_id
        )
        .fetch_one(&state.pool)
        .await?;
        let was_deleted = deletion_count > 0;

        if was_deleted {
            tracing::info!(
                "Ignoring push for note {} - was hard-deleted by another device",
                note.id
            );
            continue;
        }

        // Check if note exists for this user (across all their devices)
        let existing = sqlx::query!(
            "SELECT modified_at, server_version, content_hash, parent_hash, hash_chain FROM notes WHERE id = ? AND user_id = ?",
            note.id,
            client_info.user_id
        )
        .fetch_optional(&state.pool)
        .await?;

        // Hash-based conflict detection (like git)
        let should_accept = match &existing {
            None => true, // New note - always accept
            Some(existing_note) => {
                // If incoming note has hash chain fields, use hash-based detection
                if let (Some(incoming_content_hash), Some(incoming_parent_hash)) =
                    (&note.content_hash, &note.parent_hash)
                {
                    if let Some(server_content_hash) = &existing_note.content_hash {
                        // Case 1: Server hash == incoming parent hash -> clean fast-forward
                        if server_content_hash == incoming_parent_hash {
                            true
                        }
                        // Case 2: Same content hash -> identical content, no-op accept
                        else if server_content_hash == incoming_content_hash {
                            true
                        }
                        // Case 3: Check if server hash exists in incoming hash chain
                        else if let Some(incoming_chain) = &note.hash_chain {
                            incoming_chain.contains(server_content_hash)
                        }
                        // Case 4: No match in chain -> conflict
                        else {
                            false
                        }
                    } else {
                        // Server has no hash yet (legacy note) - use timestamp fallback
                        note.modified_at >= existing_note.modified_at
                    }
                } else {
                    // Client doesn't have hash fields (legacy client) - use timestamp fallback
                    // Use >= to avoid false conflicts when timestamps are identical
                    note.modified_at >= existing_note.modified_at
                }
            }
        };

        if should_accept {
            // Convert types
            let pinned = if note.pinned { 1 } else { 0 };
            let archived = if note.archived { 1 } else { 0 };
            let locked = if note.locked { 1 } else { 0 };
            let deleted = if note.deleted { 1 } else { 0 };
            let word_wrap = note.word_wrap.map(|w| if w { 1 } else { 0 });
            let show_preview = note.show_preview.map(|p| if p { 1 } else { 0 });

            // Serialize tags as JSON
            let tags_json = serde_json::to_string(&note.tags)
                .map_err(|e| AppError::InternalError(format!("Failed to serialize tags: {}", e)))?;

            // Calculate new server version
            let server_version = existing
                .as_ref()
                .map(|e| e.server_version + 1)
                .unwrap_or(1);

            // Serialize hash chain as JSON
            let hash_chain_json = note.hash_chain.as_ref()
                .map(|chain| serde_json::to_string(chain).ok())
                .flatten();

            // Upsert note (with both user_id for access control and client_id for audit)
            // Use composite primary key (id, user_id) to allow same note UUIDs across users
            sqlx::query!(
                r#"
                INSERT INTO notes (
                    id, user_id, client_id, created_at, modified_at, server_modified_at,
                    content, tags, pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, version, server_version,
                    word_wrap, syntax_language, show_preview, color,
                    content_hash, parent_hash, hash_chain
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id, user_id) DO UPDATE SET
                    modified_at = excluded.modified_at,
                    server_modified_at = excluded.server_modified_at,
                    content = excluded.content,
                    tags = excluded.tags,
                    pinned = excluded.pinned,
                    archived = excluded.archived,
                    archived_at = excluded.archived_at,
                    locked = excluded.locked,
                    locked_at = excluded.locked_at,
                    deleted = excluded.deleted,
                    deleted_at = excluded.deleted_at,
                    version = excluded.version,
                    server_version = excluded.server_version,
                    word_wrap = excluded.word_wrap,
                    syntax_language = excluded.syntax_language,
                    show_preview = excluded.show_preview,
                    color = excluded.color,
                    content_hash = excluded.content_hash,
                    parent_hash = excluded.parent_hash,
                    hash_chain = excluded.hash_chain
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
                archived,
                note.archived_at,
                locked,
                note.locked_at,
                deleted,
                note.deleted_at,
                note.version,
                server_version,
                word_wrap,
                note.syntax_language,
                show_preview,
                note.color,
                note.content_hash,
                note.parent_hash,
                hash_chain_json
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
                r#"SELECT content, tags, pinned, archived, archived_at, locked, locked_at, server_version, syntax_language, word_wrap, show_preview, color, modified_at,
                          content_hash, parent_hash, hash_chain
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

            // Parse server hash chain from JSON
            let server_hash_chain: Option<Vec<String>> = server_note.hash_chain
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok());

            // Try to find common ancestor for 3-way merge
            let mut ancestor_hash: Option<String> = None;
            let mut ancestor_content: Option<String> = None;
            let mut ancestor_tags: Option<Vec<String>> = None;

            // Find common ancestor by checking if incoming parent exists in server chain
            // or server parent exists in incoming chain
            if let (Some(incoming_chain), Some(server_chain)) = (&note.hash_chain, &server_hash_chain) {
                // Look for common hash in both chains
                let server_chain_set: std::collections::HashSet<&String> = server_chain.iter().collect();
                for hash in incoming_chain {
                    if server_chain_set.contains(hash) {
                        ancestor_hash = Some(hash.clone());
                        break;
                    }
                }

                // If we found an ancestor hash, try to retrieve its content from note_versions
                if let Some(ref anc_hash) = ancestor_hash {
                    // Look up version by content hash
                    if let Ok(Some(version_row)) = sqlx::query!(
                        r#"SELECT content, tags FROM note_versions
                           WHERE note_id = ? AND user_id = ? AND content_hash = ?
                           LIMIT 1"#,
                        note.id,
                        client_info.user_id,
                        anc_hash
                    )
                    .fetch_optional(&state.pool)
                    .await
                    {
                        ancestor_content = Some(version_row.content);
                        ancestor_tags = serde_json::from_str(&version_row.tags).ok();
                    }
                }
            }

            // Determine rejection reason based on conflict type
            let rejection_reason = if note.content_hash.is_some() && note.parent_hash.is_some() {
                // Hash-based conflict detection was used
                "Conflict: diverged from common ancestor".to_string()
            } else {
                // Timestamp-based fallback was used
                "Server has newer version".to_string()
            };

            rejected.push(SyncRejected {
                id: note.id.clone(),
                reason: rejection_reason,
                server_modified_at: server_note.modified_at,
                server_content: server_note.content,
                server_tags,
                server_version: server_note.server_version,
                server_attachments,
                server_pinned: server_note.pinned != 0,
                server_archived: server_note.archived,
                server_archived_at: server_note.archived_at,
                server_locked: server_note.locked != 0,
                server_locked_at: server_note.locked_at,
                server_syntax_language: server_note.syntax_language,
                server_word_wrap: server_note.word_wrap.map(|w| w == 1),
                server_show_preview: server_note.show_preview.map(|p| p == 1),
                server_color: server_note.color,
                // Hash chain fields
                server_content_hash: server_note.content_hash,
                server_parent_hash: server_note.parent_hash,
                server_hash_chain,
                // Ancestor data for 3-way merge
                ancestor_hash,
                ancestor_content,
                ancestor_tags,
            });

            tracing::debug!("Rejected note: {} (conflict) - included server data and ancestor for resolution", note.id);
        }
    }

    // Collect accepted note IDs for filtering versions and attachments
    let accepted_note_ids: std::collections::HashSet<String> =
        accepted.iter().map(|a| a.id.clone()).collect();

    // Store note versions (only for accepted notes to avoid FK errors)
    for version in push_req.versions {
        // Skip versions for notes that weren't accepted (avoids FK constraint errors)
        if !accepted_note_ids.contains(&version.note_id) {
            tracing::debug!(
                "Skipping version {} for rejected note {}",
                version.version_key,
                version.note_id
            );
            continue;
        }

        let now_for_version = chrono::Utc::now().to_rfc3339();

        // Serialize tags and attachments as JSON
        let tags_json = serde_json::to_string(&version.tags)
            .map_err(|e| AppError::InternalError(format!("Failed to serialize version tags: {}", e)))?;
        let attachments_json = serde_json::to_string(&version.attachments)
            .map_err(|e| AppError::InternalError(format!("Failed to serialize version attachments: {}", e)))?;

        // Convert optional fields
        let word_wrap = version.word_wrap.map(|w| if w { 1 } else { 0 });
        let show_preview = version.show_preview.map(|p| if p { 1 } else { 0 });

        // Upsert version (insert or replace if exists)
        // Use match to handle potential FK errors gracefully
        let result = sqlx::query!(
            r#"
            INSERT INTO note_versions (
                version_key, note_id, user_id, client_id, version, created_at, synced_at,
                server_synced_at, content, tags, attachments, syntax_language, word_wrap, show_preview, color, reason,
                content_hash, parent_hash
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
            ON CONFLICT(version_key) DO UPDATE SET
                synced_at = excluded.synced_at,
                server_synced_at = excluded.server_synced_at,
                content_hash = excluded.content_hash,
                parent_hash = excluded.parent_hash
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
            show_preview,
            version.color,
            version.reason,
            version.content_hash,
            version.parent_hash
        )
        .execute(&state.pool)
        .await;

        match result {
            Ok(_) => {
                tracing::debug!("Stored version: {} for note {}", version.version_key, version.note_id);
            }
            Err(e) => {
                // Log and skip on FK or other errors (graceful degradation)
                tracing::warn!(
                    "Failed to store version {} for note {}: {} (skipping)",
                    version.version_key,
                    version.note_id,
                    e
                );
            }
        }
    }

    // Store attachment data (binary blobs)
    // Handle FK errors gracefully - attachment metadata may not exist if note was rejected
    for attachment in push_req.attachments {
        // Decode base64
        let data = match base64::engine::general_purpose::STANDARD.decode(&attachment.data) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Invalid base64 for attachment {}: {} (skipping)", attachment.id, e);
                continue;
            }
        };

        // Store in attachments_data
        let result = sqlx::query!(
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
        .await;

        match result {
            Ok(_) => {
                tracing::debug!("Stored attachment: {}", attachment.id);
            }
            Err(e) => {
                // Log and skip on FK or other errors (graceful degradation)
                // This can happen if attachment metadata wasn't stored because note was rejected
                tracing::warn!(
                    "Failed to store attachment data {}: {} (skipping)",
                    attachment.id,
                    e
                );
            }
        }
    }

    // Store saved searches (similar to notes, with last-write-wins conflict resolution)
    for search in push_req.saved_searches {
        // Check if saved search exists for this user
        let existing = sqlx::query!(
            "SELECT modified_at FROM saved_searches WHERE id = ? AND user_id = ?",
            search.id,
            client_info.user_id
        )
        .fetch_optional(&state.pool)
        .await?;

        let should_accept = match &existing {
            None => true, // New saved search
            Some(existing_search) => {
                // Last-Write-Wins: compare modifiedAt
                search.modified_at >= existing_search.modified_at
            }
        };

        if should_accept {
            let deleted = if search.deleted { 1 } else { 0 };

            // Upsert saved search
            sqlx::query!(
                r#"
                INSERT INTO saved_searches (
                    id, user_id, name, query, "order", created_at, modified_at,
                    synced_at, deleted, deleted_at, version
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    query = excluded.query,
                    "order" = excluded."order",
                    modified_at = excluded.modified_at,
                    synced_at = excluded.synced_at,
                    deleted = excluded.deleted,
                    deleted_at = excluded.deleted_at,
                    version = excluded.version
                "#,
                search.id,
                client_info.user_id,
                search.name,
                search.query,
                search.order,
                search.created_at,
                search.modified_at,
                now,
                deleted,
                search.deleted_at,
                search.version
            )
            .execute(&state.pool)
            .await?;

            tracing::debug!("Stored saved search: {}", search.id);
        }
    }

    // Process hard deletions (notes permanently removed from client's recycle bin)
    for deletion in push_req.deletions {
        // Delete the note from the notes table
        let delete_result = sqlx::query!(
            "DELETE FROM notes WHERE id = ? AND user_id = ?",
            deletion.id,
            client_info.user_id
        )
        .execute(&state.pool)
        .await?;

        if delete_result.rows_affected() > 0 {
            // Record the deletion so other clients can pick it up
            // Expires after 30 days (same as cleanup_old_deleted_notes)
            let expires_at = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();

            sqlx::query!(
                r#"
                INSERT INTO note_deletions (id, user_id, deleted_at, synced_from_client_id, expires_at)
                VALUES (?, ?, ?, ?, ?)
                ON CONFLICT(id, user_id) DO UPDATE SET
                    deleted_at = excluded.deleted_at,
                    synced_from_client_id = excluded.synced_from_client_id
                "#,
                deletion.id,
                client_info.user_id,
                deletion.deleted_at,
                client_info.client_id,
                expires_at
            )
            .execute(&state.pool)
            .await?;

            tracing::info!("Recorded hard deletion for note {} (user {})", deletion.id, client_info.user_id);
        }
    }

    // Broadcast sync notification to other connected devices of this user
    // This triggers real-time sync via SSE for any other connected clients
    if !accepted.is_empty() {
        use crate::api::sse::SyncNotification;
        let _ = state.sync_broadcast.send(SyncNotification {
            user_id: client_info.user_id.clone(),
            source_client_id: client_info.client_id.clone(),
        });
        tracing::debug!(
            "Broadcast sync notification for user {} (from client {})",
            client_info.user_id,
            client_info.client_id
        );
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
            "SELECT id, client_id, created_at, modified_at, server_modified_at, content, tags, pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, version, server_version, word_wrap, syntax_language, show_preview, color, content_hash, parent_hash, hash_chain FROM notes WHERE user_id = ? AND server_modified_at > ? ORDER BY server_modified_at LIMIT ? OFFSET ?",
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
                archived: if row.archived { 1 } else { 0 },
                archived_at: row.archived_at,
                locked: row.locked,
                locked_at: row.locked_at,
                deleted: row.deleted,
                deleted_at: row.deleted_at,
                version: row.version,
                server_version: row.server_version,
                word_wrap: row.word_wrap,
                syntax_language: row.syntax_language,
                show_preview: row.show_preview,
                color: row.color,
                content_hash: row.content_hash,
                parent_hash: row.parent_hash,
                hash_chain: row.hash_chain,
            }))
            .collect()
    } else {
        let rows = sqlx::query!(
            "SELECT id, client_id, created_at, modified_at, server_modified_at, content, tags, pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, version, server_version, word_wrap, syntax_language, show_preview, color, content_hash, parent_hash, hash_chain FROM notes WHERE user_id = ? ORDER BY server_modified_at LIMIT ? OFFSET ?",
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
                archived: if row.archived { 1 } else { 0 },
                archived_at: row.archived_at,
                locked: row.locked,
                locked_at: row.locked_at,
                deleted: row.deleted,
                deleted_at: row.deleted_at,
                version: row.version,
                server_version: row.server_version,
                word_wrap: row.word_wrap,
                syntax_language: row.syntax_language,
                show_preview: row.show_preview,
                color: row.color,
                content_hash: row.content_hash,
                parent_hash: row.parent_hash,
                hash_chain: row.hash_chain,
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

        // Parse hash chain from JSON
        let hash_chain: Option<Vec<String>> = db_note.hash_chain
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok());

        notes.push(SyncNote {
            id: db_note.id,
            created_at: db_note.created_at,
            modified_at: db_note.modified_at,
            content: db_note.content,
            tags,
            attachments,
            pinned: db_note.pinned != 0,
            archived: db_note.archived != 0,
            archived_at: db_note.archived_at,
            locked: db_note.locked != 0,
            locked_at: db_note.locked_at,
            deleted: db_note.deleted != 0,
            deleted_at: db_note.deleted_at,
            version: db_note.version,
            word_wrap: db_note.word_wrap.map(|w| w != 0),
            syntax_language: db_note.syntax_language,
            show_preview: db_note.show_preview.map(|p| p != 0),
            color: db_note.color,
            // Hash chain fields
            content_hash: db_note.content_hash,
            parent_hash: db_note.parent_hash,
            hash_chain,
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

    // Get hard deletions that this client needs to apply
    // Query deletions since last sync (or all if no last sync)
    let now_for_expiry = chrono::Utc::now().to_rfc3339();
    let deletions: Vec<crate::models::SyncDeletion> = if let Some(last_sync) = &pull_req.last_sync_at {
        sqlx::query!(
            r#"SELECT id, deleted_at FROM note_deletions
               WHERE user_id = ? AND deleted_at > ? AND expires_at > ?
               ORDER BY deleted_at"#,
            client_info.user_id,
            last_sync,
            now_for_expiry
        )
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|row| crate::models::SyncDeletion {
            id: row.id,
            deleted_at: row.deleted_at,
        })
        .collect()
    } else {
        // Full sync - return all non-expired deletions
        sqlx::query!(
            r#"SELECT id, deleted_at FROM note_deletions
               WHERE user_id = ? AND expires_at > ?
               ORDER BY deleted_at"#,
            client_info.user_id,
            now_for_expiry
        )
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|row| crate::models::SyncDeletion {
            id: row.id,
            deleted_at: row.deleted_at,
        })
        .collect()
    };

    if !deletions.is_empty() {
        tracing::info!("Returning {} deletions to client", deletions.len());
    }

    // Get versions for all notes belonging to this user (shared across devices)
    let epoch = "1970-01-01T00:00:00Z".to_string();
    let last_sync_filter = pull_req.last_sync_at.as_ref().unwrap_or(&epoch);
    let db_versions = sqlx::query!(
        "SELECT version_key, note_id, version, created_at, synced_at, content, tags, attachments, syntax_language, word_wrap, show_preview, color, reason, content_hash, parent_hash
         FROM note_versions
         WHERE user_id = ? AND (? = '1970-01-01T00:00:00Z' OR server_synced_at > ?)
         ORDER BY note_id, version",
        client_info.user_id,
        last_sync_filter,
        last_sync_filter
    )
    .fetch_all(&state.pool)
    .await?;

    // Convert to SyncNoteVersion
    let mut versions_response = Vec::new();
    for db_version in db_versions {
        // Deserialize tags (log warning if corrupted)
        let tags: Vec<String> = serde_json::from_str(&db_version.tags)
            .inspect_err(|e| tracing::warn!(
                "Failed to parse tags for version {} of note {}: {}",
                db_version.version_key, db_version.note_id, e
            ))
            .unwrap_or_default();

        // Deserialize attachments (log warning if corrupted)
        let attachments: Vec<crate::models::AttachmentRef> = serde_json::from_str(&db_version.attachments)
            .inspect_err(|e| tracing::warn!(
                "Failed to parse attachments for version {} of note {}: {}",
                db_version.version_key, db_version.note_id, e
            ))
            .unwrap_or_default();

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
            show_preview: db_version.show_preview.map(|p| p != 0),
            color: db_version.color,
            reason: db_version.reason,
            // Hash chain fields
            content_hash: db_version.content_hash,
            parent_hash: db_version.parent_hash,
        });
    }

    let synced_at = chrono::Utc::now().to_rfc3339();

    // Fetch saved searches for this user (modified since last sync)
    let db_saved_searches: Vec<crate::models::SavedSearch> = if let Some(last_sync) = &pull_req.last_sync_at {
        sqlx::query_as!(
            crate::models::SavedSearch,
            r#"SELECT id, user_id, name, query, "order", created_at, modified_at, synced_at, deleted, deleted_at, version
               FROM saved_searches WHERE user_id = ? AND modified_at > ?
               ORDER BY "order""#,
            client_info.user_id,
            last_sync
        )
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as!(
            crate::models::SavedSearch,
            r#"SELECT id, user_id, name, query, "order", created_at, modified_at, synced_at, deleted, deleted_at, version
               FROM saved_searches WHERE user_id = ?
               ORDER BY "order""#,
            client_info.user_id
        )
        .fetch_all(&state.pool)
        .await?
    };

    // Convert to sync models
    let saved_searches: Vec<crate::models::SyncSavedSearch> = db_saved_searches
        .into_iter()
        .map(|db| db.into())
        .collect();

    // Fetch inbox items for this user
    let inbox_rows = sqlx::query!(
        "SELECT id, content, tags, created_at, source, size_bytes FROM inbox_items WHERE user_id = ? ORDER BY created_at DESC",
        client_info.user_id
    )
    .fetch_all(&state.pool)
    .await?;

    let inbox_count = inbox_rows.len() as i64;
    let inbox_items: Vec<crate::models::InboxItemResponse> = inbox_rows
        .into_iter()
        .map(|row| {
            let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap_or_default();
            crate::models::InboxItemResponse {
                id: row.id,
                content: row.content,
                tags,
                created_at: row.created_at,
                source: row.source,
                size_bytes: row.size_bytes,
            }
        })
        .collect();

    let (inbox_items_opt, inbox_count_opt) = if inbox_count > 0 {
        (Some(inbox_items), Some(inbox_count))
    } else {
        (None, None)
    };

    tracing::info!(
        "Pull response: {} notes (total: {}, hasMore: {}), {} attachments, {} versions, {} saved searches, {} inbox items",
        notes.len(),
        total_count,
        has_more,
        attachments_data.len(),
        versions_response.len(),
        saved_searches.len(),
        inbox_count
    );

    Ok(Json(SyncPullResponse {
        notes,
        deletions,
        attachments: attachments_data,
        versions: versions_response,
        saved_searches,
        synced_at,
        total_count,
        has_more,
        inbox_items: inbox_items_opt,
        inbox_count: inbox_count_opt,
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

/// Delete notes that have been marked as deleted for more than 30 days
/// This is called automatically during sync operations to prevent deleted notes
/// from accumulating indefinitely on the server
async fn cleanup_old_deleted_notes(pool: &sqlx::SqlitePool) -> Result<u64, sqlx::Error> {
    // Calculate 30 days ago
    let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);
    let cutoff_date = thirty_days_ago.to_rfc3339();

    // Delete notes that were marked deleted more than 30 days ago
    let result = sqlx::query!(
        r#"
        DELETE FROM notes
        WHERE deleted = 1 AND deleted_at IS NOT NULL AND deleted_at < ?
        "#,
        cutoff_date
    )
    .execute(pool)
    .await?;

    let deleted_count = result.rows_affected();

    if deleted_count > 0 {
        tracing::info!("Cleaned up {} notes that were deleted more than 30 days ago", deleted_count);
    }

    Ok(deleted_count)
}
