use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    api::extractors::AuthClient,
    db::{AttachmentRepository, BlobStored},
    error::{AppError, AppResult},
    models::{
        AttachmentRef, SyncAccepted, SyncAttachmentData, SyncNote, SyncPullRequest,
        SyncPullResponse, SyncPushRequest, SyncPushResponse, SyncRejected, SyncStatusResponse,
    },
    utils::validation,
    AppState,
};

/// Default page size for a pull.
const DEFAULT_PULL_LIMIT: i64 = 100;

/// Largest page a pull will return. It also keeps the queries that list a page's
/// notes well inside SQLite's limit on bound parameters.
const MAX_PULL_LIMIT: i64 = 500;

/// How many bytes of blobs one pull page may carry. A page holds them raw, as
/// base64 and as JSON at once, and the real vault's notes average nearly 2 MB, so
/// a page of 100 would otherwise take hundreds of MB per request. The first note
/// with blobs is always sent, so that a large attachment still gets through.
pub const PULL_BLOB_BUDGET_BYTES: usize = 16 * 1024 * 1024;

/// How far a `lastSyncAt` filter reaches back. Clients that haven't moved to the
/// cursor store a timestamp, and a row can be stamped before that timestamp yet
/// commit after it. Several clients also take theirs from their own clock when a
/// pull ends, so this has to cover a pull as well as a push.
///
/// It is kept short because a re-sent note isn't always harmless. Some released
/// clients overwrite a local edit made between their push and their pull, so
/// every note in this window is exposed to that for as long as the window lasts.
/// Cursor mode has no overlap.
const LAST_SYNC_OVERLAP_MINUTES: i64 = 2;

const CURSOR_PREFIX: &str = "v1.";

/// How far ahead of the server a client's clock can plausibly be. It allows for any
/// time zone mistake. Past this a time isn't believed, because the latest time wins
/// a conflict, and a device a year ahead would otherwise win all of them.
const MAX_CLOCK_SKEW_HOURS: i64 = 24;

fn read_time(time: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(time)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn is_implausibly_future(time: DateTime<Utc>) -> bool {
    time > Utc::now() + chrono::Duration::hours(MAX_CLOCK_SKEW_HOURS)
}

/// Why a time from a client can't be used, if it can't.
fn time_problem(name: &str, time: &str) -> Option<String> {
    match read_time(time) {
        None => Some(format!("{name} can't be read")),
        Some(time) if is_implausibly_future(time) => {
            Some(format!("{name} is more than {MAX_CLOCK_SKEW_HOURS} hours ahead"))
        }
        Some(_) => None,
    }
}

/// Why a pushed note's times can't be used, if they can't. An optional time that's
/// empty counts as absent.
fn note_time_problem(note: &SyncNote) -> Option<String> {
    let required = [("createdAt", &note.created_at), ("modifiedAt", &note.modified_at)];
    let optional = [
        ("deletedAt", &note.deleted_at),
        ("archivedAt", &note.archived_at),
        ("lockedAt", &note.locked_at),
    ];

    required
        .into_iter()
        .filter_map(|(name, time)| time_problem(name, time))
        .chain(optional.into_iter().filter_map(|(name, time)| {
            time.as_deref()
                .filter(|time| !time.is_empty())
                .and_then(|time| time_problem(name, time))
        }))
        .next()
}

/// Whether a pushed time is at least as late as a stored one. Clients format times
/// differently, so these are compared as instants. A stored time from before they
/// were checked may not be readable, and is compared as text, as it always was.
fn is_not_older(pushed: &str, stored: &str) -> bool {
    match (read_time(pushed), read_time(stored)) {
        (Some(pushed), Some(stored)) => pushed >= stored,
        _ => pushed >= stored,
    }
}

/// Whether an edit made at `edit` wins over a deletion made at `deletion`.
///
/// The same rule applies whichever arrives first, so devices agree. Clients
/// format times differently, so these must be compared as instants, not as
/// text. A time that can't be read can't prove the edit is older, so the edit
/// wins: keeping a note can be undone, deleting it can't. A deletion dated far in
/// the future can't be believed either, or it would beat every edit there will be.
fn edit_wins_over_deletion(edit: &str, deletion: &str) -> bool {
    match (read_time(edit), read_time(deletion)) {
        (Some(_), Some(deletion)) if is_implausibly_future(deletion) => true,
        (Some(edit), Some(deletion)) => edit > deletion,
        _ => true,
    }
}

fn encode_cursor(seq: i64) -> String {
    format!("{CURSOR_PREFIX}{seq}")
}

fn decode_cursor(cursor: &str) -> Option<i64> {
    cursor
        .strip_prefix(CURSOR_PREFIX)?
        .parse()
        .ok()
        .filter(|seq| *seq >= 0)
}

/// How long a tombstone is sent to clients on timestamp mode.
const TOMBSTONE_RETENTION_DAYS: i64 = 30;

/// Record that a note is gone, so other devices hear about it. Must be called in
/// the transaction that deletes the note. `client_id` is the device that asked, if
/// one did.
pub(crate) async fn record_tombstone(
    conn: &mut SqliteConnection,
    user_id: &str,
    note_id: &str,
    deleted_at: &str,
    client_id: Option<&str>,
    now: &str,
) -> Result<(), sqlx::Error> {
    let expires_at =
        (chrono::Utc::now() + chrono::Duration::days(TOMBSTONE_RETENTION_DAYS)).to_rfc3339();
    let change_seq = next_seq(&mut *conn, user_id).await?;

    sqlx::query!(
        r#"
        INSERT INTO note_deletions (id, user_id, deleted_at, synced_from_client_id, expires_at, received_at, change_seq)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id, user_id) DO UPDATE SET
            deleted_at = excluded.deleted_at,
            synced_from_client_id = excluded.synced_from_client_id,
            received_at = excluded.received_at,
            change_seq = excluded.change_seq
        "#,
        note_id,
        user_id,
        deleted_at,
        client_id,
        expires_at,
        now,
        change_seq
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Take the next position in a user's change sequence.
///
/// Must be called inside the write transaction. SQLite has one writer, so the
/// transaction holds the lock from here to its commit, which makes sequence
/// order the same as commit order. That is what lets a pull trust that every
/// row at or below a sequence it has seen is already visible.
async fn next_seq(conn: &mut SqliteConnection, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        "INSERT INTO sync_state (user_id, last_seq) VALUES (?, 1)
         ON CONFLICT(user_id) DO UPDATE SET last_seq = last_seq + 1
         RETURNING last_seq",
        user_id
    )
    .fetch_one(conn)
    .await
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

    // Get user's limits
    let user = sqlx::query!(
        "SELECT max_upload_size_mb, storage_quota_mb, storage_used_bytes FROM users WHERE id = ?",
        client_info.user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    let max_upload_bytes = user
        .as_ref()
        .map(|u| u.max_upload_size_mb)
        .unwrap_or(state.config.default_max_upload_size_mb) as usize * 1024 * 1024;

    let blob_bytes: usize = push_req.attachments.iter().map(|a| a.data.len()).sum();
    let note_bytes: usize = push_req.notes.iter().map(|n| n.content.len()).sum();
    let version_bytes: usize = push_req.versions.iter().map(|v| v.content.len()).sum();

    // Estimate payload size from attachments (main contributor to size). Versions
    // only count once quotas are on: released clients send a lot of them, and an
    // install that hasn't asked for limits shouldn't start refusing their pushes.
    let estimated_size = blob_bytes
        + note_bytes
        + if state.config.enforce_storage_quota { version_bytes } else { 0 };

    if estimated_size > max_upload_bytes {
        let max_mb = max_upload_bytes / (1024 * 1024);
        return Err(AppError::PayloadTooLarge(format!(
            "Upload size (~{}MB) exceeds your limit of {}MB",
            estimated_size / (1024 * 1024),
            max_mb
        )));
    }

    // What this push adds. A note going to the bin doesn't count, so that a full
    // account can still make room. Blobs are counted as base64, a little over.
    let growth = blob_bytes
        + version_bytes
        + push_req
            .notes
            .iter()
            .filter(|n| !n.deleted)
            .map(|n| n.content.len())
            .sum::<usize>();

    if state.config.enforce_storage_quota && growth > 0 {
        let quota_mb = user
            .as_ref()
            .and_then(|u| u.storage_quota_mb)
            .unwrap_or(state.config.default_storage_quota_mb);
        let used = user.as_ref().map_or(0, |u| u.storage_used_bytes);

        if used + growth as i64 > quota_mb * 1024 * 1024 {
            tracing::warn!(
                "Refusing a push from user {}: {} bytes used, {} more would pass the {}MB quota",
                client_info.user_id,
                used,
                growth,
                quota_mb
            );
            return Err(AppError::StorageQuotaExceeded);
        }
    }

    tracing::info!(
        "Push from user {} (client {}): {} notes, {} attachments (~{}KB)",
        client_info.user_id,
        client_info.client_id,
        push_req.notes.len(),
        push_req.attachments.len(),
        estimated_size / 1024
    );

    let attachments_authoritative = push_req.attachments_authoritative;

    // Everything that can fail on the input alone happens before the first
    // write, so a bad request leaves no trace.
    for note in &push_req.notes {
        validation::validate_note_content(&note.content, &state.config)?;
    }

    let mut errors = Vec::new();

    // Decoding happens outside the transaction so the write lock is held for
    // database work only.
    let mut blobs = Vec::new();
    for attachment in push_req.attachments {
        match base64::engine::general_purpose::STANDARD.decode(&attachment.data) {
            Ok(data) => blobs.push((attachment.id, data)),
            Err(e) => {
                tracing::warn!("Invalid base64 for attachment {:?}: {} (skipping)", attachment.id, e);
                errors.push(format!("Attachment {} skipped: invalid base64", attachment.id));
            }
        }
    }

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut skipped_deleted = Vec::new();

    // The whole push is one transaction. BEGIN IMMEDIATE takes the write lock
    // up front, so reading a note and acting on what was read can't interleave
    // with another push, and a pull can never see half of this one. A deferred
    // transaction could also fail on the upgrade from read to write.
    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;

    // Stamped inside the lock, so timestamps follow commit order
    let now = chrono::Utc::now().to_rfc3339();

    // Housekeeping corrects this to what's really stored, every hour
    let growth = growth as i64;
    sqlx::query!(
        "UPDATE users SET storage_used_bytes = storage_used_bytes + ? WHERE id = ?",
        growth,
        client_info.user_id
    )
    .execute(&mut *tx)
    .await?;

    for note in push_req.notes {
        // A time that can't be trusted can't be allowed to decide anything. The
        // note stays on the device, which sends it again once its clock is right.
        if let Some(problem) = note_time_problem(&note) {
            tracing::warn!("Skipping note {:?}: {}", note.id, problem);
            errors.push(format!("Note {} skipped: {}", note.id, problem));
            continue;
        }

        // Check if this note was hard-deleted by another device. A pushed
        // edit made *after* the tombstone wins (edit-wins-over-delete,
        // matching the client's own conflict semantics): clear the
        // tombstone and let the note fall through to the normal upsert
        // below, resurrecting it. An edit that is not newer than the
        // tombstone stays rejected as deleted.
        let tombstone = sqlx::query!(
            "SELECT deleted_at FROM note_deletions WHERE id = ? AND user_id = ?",
            note.id,
            client_info.user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(tombstone) = tombstone {
            if edit_wins_over_deletion(&note.modified_at, &tombstone.deleted_at) {
                // Edit postdates the deletion - resurrect the note. Clear
                // the tombstone first so the upsert below (which finds no
                // existing row in `notes`) inserts a fresh row, and other
                // devices see this as a normal update - not a deletion - on
                // their next pull.
                sqlx::query!(
                    "DELETE FROM note_deletions WHERE id = ? AND user_id = ?",
                    note.id,
                    client_info.user_id
                )
                .execute(&mut *tx)
                .await?;

                tracing::info!(
                    "Resurrecting note {:?} - pushed edit ({:?}) postdates tombstone ({:?})",
                    note.id,
                    note.modified_at,
                    tombstone.deleted_at
                );
            } else {
                // Edit does not postdate the deletion - the note stays
                // deleted. There is no live server note to describe for the
                // `rejected` conflict-resolution channel (that structure
                // reports the *current* server note's content/tags/version
                // for a 3-way merge, which does not exist here), so we keep
                // this as an explicit skip. The client will re-learn the
                // note is gone from the `deletions` list on its next pull, and
                // `skippedDeleted` tells it now, so it can stop sending the note.
                tracing::info!(
                    "Ignoring push for note {:?} - was hard-deleted by another device (edit {:?} does not postdate tombstone {:?})",
                    note.id,
                    note.modified_at,
                    tombstone.deleted_at
                );
                skipped_deleted.push(note.id);
                continue;
            }
        }

        // Check if note exists for this user (across all their devices)
        let existing = sqlx::query!(
            "SELECT modified_at, server_version, content_hash, parent_hash, hash_chain FROM notes WHERE id = ? AND user_id = ?",
            note.id,
            client_info.user_id
        )
        .fetch_optional(&mut *tx)
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
                        is_not_older(&note.modified_at, &existing_note.modified_at)
                    }
                } else {
                    // Client doesn't have hash fields (legacy client) - use timestamp fallback
                    // Not older, to avoid false conflicts when timestamps are identical
                    is_not_older(&note.modified_at, &existing_note.modified_at)
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

            let change_seq = next_seq(&mut tx, &client_info.user_id).await?;

            // Upsert note (with both user_id for access control and client_id for audit)
            // Use composite primary key (id, user_id) to allow same note UUIDs across users
            sqlx::query!(
                r#"
                INSERT INTO notes (
                    id, user_id, client_id, created_at, modified_at, server_modified_at,
                    content, tags, pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, version, server_version,
                    word_wrap, syntax_language, show_preview, color,
                    content_hash, parent_hash, hash_chain, change_seq
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                    hash_chain = excluded.hash_chain,
                    change_seq = excluded.change_seq
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
                hash_chain_json,
                change_seq
            )
            .execute(&mut *tx)
            .await?;

            accepted.push(SyncAccepted {
                id: note.id.clone(),
                server_version,
                synced_at: now.clone(),
            });

            // Store attachment metadata for this note
            for attachment_ref in &note.attachments {
                AttachmentRepository::link_to_note(
                    &mut tx,
                    &client_info.user_id,
                    &note.id,
                    attachment_ref,
                    &now,
                )
                .await?;

                tracing::debug!("Stored attachment metadata: {} for note {}", attachment_ref.id, note.id);
            }

            // Only a client that vouches for its list gets pruning: some older
            // clients send empty lists by mistake.
            if attachments_authoritative {
                let keep: Vec<&str> = note.attachments.iter().map(|a| a.id.as_str()).collect();
                AttachmentRepository::unlink_others(
                    &mut tx,
                    &client_info.user_id,
                    &note.id,
                    &keep,
                )
                .await?;
            }

            tracing::debug!("Accepted note: {}", note.id);
        } else {
            // Fetch full server note data for conflict resolution
            let server_note = sqlx::query!(
                r#"SELECT content, tags, pinned, archived, archived_at, locked, locked_at, server_version, syntax_language, word_wrap, show_preview, color, modified_at,
                          content_hash, parent_hash, hash_chain, deleted, deleted_at
                   FROM notes WHERE id = ? AND user_id = ?"#,
                note.id,
                client_info.user_id
            )
            .fetch_one(&mut *tx)
            .await?;

            // Fetch this user's attachments for the note
            let server_attachments: Vec<AttachmentRef> = AttachmentRepository::refs_for_notes(
                &mut tx,
                &client_info.user_id,
                &[note.id.as_str()],
            )
            .await?
            .into_iter()
            .map(|(_, attachment)| attachment)
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
                    .fetch_optional(&mut *tx)
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
                server_deleted: server_note.deleted != 0,
                server_deleted_at: server_note.deleted_at,
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

        // Serialize tags and attachments as JSON
        let tags_json = serde_json::to_string(&version.tags)
            .map_err(|e| AppError::InternalError(format!("Failed to serialize version tags: {}", e)))?;
        let attachments_json = serde_json::to_string(&version.attachments)
            .map_err(|e| AppError::InternalError(format!("Failed to serialize version attachments: {}", e)))?;

        // Convert optional fields
        let word_wrap = version.word_wrap.map(|w| if w { 1 } else { 0 });
        let show_preview = version.show_preview.map(|p| if p { 1 } else { 0 });

        let change_seq = next_seq(&mut tx, &client_info.user_id).await?;

        // Insert the version, or re-announce it if it's the one we have. Two devices
        // that diverge from one version produce the same key with different content.
        // The first stays: updating only the hash would label one device's content
        // with the other's, and a later 3-way merge would use it as a wrong base.
        // The loser's version stays on that device.
        // Use match to handle potential FK errors gracefully
        let result = sqlx::query!(
            r#"
            INSERT INTO note_versions (
                version_key, note_id, user_id, client_id, version, created_at, synced_at,
                server_synced_at, content, tags, attachments, syntax_language, word_wrap, show_preview, color, reason,
                content_hash, parent_hash, change_seq
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            ON CONFLICT(user_id, version_key) DO UPDATE SET
                synced_at = excluded.synced_at,
                server_synced_at = excluded.server_synced_at,
                change_seq = excluded.change_seq
            WHERE note_versions.content_hash IS excluded.content_hash
            "#,
            version.version_key,
            version.note_id,
            client_info.user_id,
            client_info.client_id,
            version.version,
            version.created_at,
            version.synced_at,
            now,
            version.content,
            tags_json,
            attachments_json,
            version.syntax_language,
            word_wrap,
            show_preview,
            version.color,
            version.reason,
            version.content_hash,
            version.parent_hash,
            change_seq
        )
        .execute(&mut *tx)
        .await;

        match result {
            Ok(_) => {
                tracing::debug!("Stored version: {} for note {}", version.version_key, version.note_id);
            }
            Err(e) => {
                // Log and skip on FK or other errors (graceful degradation)
                tracing::warn!(
                    "Failed to store version {:?} for note {:?}: {} (skipping)",
                    version.version_key,
                    version.note_id,
                    e
                );
                errors.push(format!("Version {} skipped", version.version_key));
            }
        }
    }

    // Store attachment data (binary blobs). A blob is only stored against
    // this user's own metadata, so one user can't reach another's attachment.
    let mut blobs_stored = false;
    for (attachment_id, data) in &blobs {
        let stored = AttachmentRepository::store_blob(
            &mut tx,
            &client_info.user_id,
            attachment_id,
            data,
            &now,
        )
        .await?;

        match stored {
            BlobStored::NoMetadata => {
                // Expected when the note was rejected: its metadata was never stored.
                tracing::warn!(
                    "Skipping attachment data {:?} from client {}: no matching metadata for user {}",
                    attachment_id,
                    client_info.client_id,
                    client_info.user_id
                );
            }
            BlobStored::Replaced => {
                blobs_stored = true;
                tracing::debug!("Replaced attachment: {}", attachment_id);
            }
            BlobStored::Added => {
                blobs_stored = true;
                tracing::debug!("Stored attachment: {}", attachment_id);

                // A blob that arrives after its note, as a repair push does,
                // would otherwise never reach devices that already pulled the
                // note: pull only sends blobs with the notes it returns.
                let notes = AttachmentRepository::notes_referencing(
                    &mut tx,
                    &client_info.user_id,
                    attachment_id,
                )
                .await?;
                for note_id in notes.iter().filter(|id| !accepted_note_ids.contains(*id)) {
                    announce_note(&mut tx, &client_info.user_id, note_id, &now).await?;
                }
            }
        }
    }

    // Store saved searches (similar to notes, with last-write-wins conflict resolution)
    let mut searches_changed = false;
    for search in push_req.saved_searches {
        if let Some(problem) = time_problem("modifiedAt", &search.modified_at) {
            tracing::warn!("Skipping saved search {:?}: {}", search.id, problem);
            errors.push(format!("Saved search {} skipped: {}", search.id, problem));
            continue;
        }

        // Check if saved search exists for this user
        let existing = sqlx::query!(
            "SELECT modified_at FROM saved_searches WHERE id = ? AND user_id = ?",
            search.id,
            client_info.user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        let should_accept = match &existing {
            None => true, // New saved search
            Some(existing_search) => {
                // Last-Write-Wins: compare modifiedAt
                is_not_older(&search.modified_at, &existing_search.modified_at)
            }
        };

        if should_accept {
            let deleted = if search.deleted { 1 } else { 0 };
            let change_seq = next_seq(&mut tx, &client_info.user_id).await?;

            // Upsert saved search
            sqlx::query!(
                r#"
                INSERT INTO saved_searches (
                    id, user_id, name, query, "order", created_at, modified_at,
                    synced_at, deleted, deleted_at, version, change_seq
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id, user_id) DO UPDATE SET
                    name = excluded.name,
                    query = excluded.query,
                    "order" = excluded."order",
                    modified_at = excluded.modified_at,
                    synced_at = excluded.synced_at,
                    deleted = excluded.deleted,
                    deleted_at = excluded.deleted_at,
                    version = excluded.version,
                    change_seq = excluded.change_seq
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
                search.version,
                change_seq
            )
            .execute(&mut *tx)
            .await?;

            searches_changed = true;
            tracing::debug!("Stored saved search: {}", search.id);
        }
    }

    // Process hard deletions (notes permanently removed from client's recycle bin)
    let mut deletions_applied = false;
    let mut rejected_deletions = Vec::new();
    for deletion in push_req.deletions {
        let existing = sqlx::query_scalar!(
            "SELECT modified_at FROM notes WHERE id = ? AND user_id = ?",
            deletion.id,
            client_info.user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        // A deletion older than the note's last edit loses, as it would had the
        // edit arrived second. Re-announcing the note returns it to the device
        // that deleted it, on any client version.
        if let Some(modified_at) = existing {
            if edit_wins_over_deletion(&modified_at, &deletion.deleted_at) {
                tracing::info!(
                    "Refusing deletion of note {:?} - edit ({:?}) postdates deletion ({:?})",
                    deletion.id,
                    modified_at,
                    deletion.deleted_at
                );
                announce_note(&mut tx, &client_info.user_id, &deletion.id, &now).await?;
                rejected_deletions.push(deletion.id);
                continue;
            }
        }

        // Delete the note from the notes table
        let delete_result = sqlx::query!(
            "DELETE FROM notes WHERE id = ? AND user_id = ?",
            deletion.id,
            client_info.user_id
        )
        .execute(&mut *tx)
        .await?;

        if delete_result.rows_affected() > 0 {
            // Record the deletion so other clients can pick it up
            record_tombstone(
                &mut tx,
                &client_info.user_id,
                &deletion.id,
                &deletion.deleted_at,
                Some(&client_info.client_id),
                &now,
            )
            .await?;

            deletions_applied = true;
            tracing::info!("Recorded hard deletion for note {:?} (user {})", deletion.id, client_info.user_id);
        }
    }

    tx.commit().await?;

    // Broadcast sync notification to other connected devices of this user
    // This triggers real-time sync via SSE for any other connected clients
    if !accepted.is_empty()
        || blobs_stored
        || deletions_applied
        || searches_changed
        || !rejected_deletions.is_empty()
    {
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

    // Warn about accepted notes whose attachment references have no stored
    // data (payload omitted the blob and the server never received it).
    // Clients holding the blob react with an attachments-only repair push.
    // This must never fail the push: on query error, log and omit the field.
    let attachment_warnings: Option<Vec<crate::models::AttachmentWarning>> = if accepted.is_empty() {
        None
    } else {
        let mut qb: sqlx::QueryBuilder<'_, sqlx::Sqlite> = sqlx::QueryBuilder::new(
            "SELECT na.note_id, m.attachment_id FROM note_attachments na \
             JOIN attachments_meta m ON m.user_id = na.user_id AND m.attachment_id = na.attachment_id \
             LEFT JOIN attachments_data d ON d.id = m.id \
             WHERE d.id IS NULL AND na.user_id = ",
        );
        qb.push_bind(&client_info.user_id);
        qb.push(" AND na.note_id IN (");
        let mut sep = qb.separated(", ");
        for a in &accepted {
            sep.push_bind(a.id.as_str());
        }
        sep.push_unseparated(") ORDER BY na.note_id, m.attachment_id");

        match qb
            .build_query_as::<(String, String)>()
            .fetch_all(&state.pool)
            .await
        {
            Ok(rows) if !rows.is_empty() => {
                let mut by_note: std::collections::BTreeMap<String, Vec<String>> = Default::default();
                for (note_id, att_id) in rows {
                    tracing::warn!(
                        "Note {:?} (user {}) references attachment {:?} with no stored data",
                        note_id,
                        client_info.user_id,
                        att_id
                    );
                    by_note.entry(note_id).or_default().push(att_id);
                }
                Some(
                    by_note
                        .into_iter()
                        .map(|(note_id, attachment_ids)| crate::models::AttachmentWarning {
                            note_id,
                            attachment_ids,
                        })
                        .collect(),
                )
            }
            Ok(_) => None,
            Err(e) => {
                tracing::error!("Attachment warning query failed (push still succeeds): {}", e);
                None
            }
        }
    };

    Ok(Json(SyncPushResponse {
        accepted,
        rejected,
        errors,
        attachment_warnings,
        rejected_deletions,
        skipped_deleted,
    }))
}

/// Give a note a fresh place in the change stream without changing it, so that
/// every device receives it again.
async fn announce_note(
    conn: &mut SqliteConnection,
    user_id: &str,
    note_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let change_seq = next_seq(&mut *conn, user_id).await?;
    sqlx::query!(
        "UPDATE notes SET change_seq = ?, server_modified_at = ? WHERE id = ? AND user_id = ?",
        change_seq,
        now,
        note_id,
        user_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

const NOTE_COLUMNS: &str = "id, client_id, created_at, modified_at, server_modified_at, content, \
     tags, pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, version, \
     server_version, word_wrap, syntax_language, show_preview, color, content_hash, parent_hash, \
     hash_chain";

const VERSION_COLUMNS: &str = "version_key, note_id, version, created_at, synced_at, content, \
     tags, attachments, syntax_language, word_wrap, show_preview, color, reason, content_hash, \
     parent_hash";

const SAVED_SEARCH_COLUMNS: &str = r#"id, user_id, name, query, "order", created_at, modified_at,
     synced_at, deleted, deleted_at, version"#;

#[derive(sqlx::FromRow)]
struct VersionRow {
    version_key: String,
    note_id: String,
    version: i64,
    created_at: String,
    synced_at: String,
    content: String,
    tags: String,
    attachments: String,
    syntax_language: Option<String>,
    word_wrap: Option<i64>,
    show_preview: Option<i64>,
    color: Option<String>,
    reason: String,
    content_hash: Option<String>,
    parent_hash: Option<String>,
}

#[derive(sqlx::FromRow)]
struct DeletionRow {
    id: String,
    deleted_at: String,
}

/// The rows one pull returns, before attachments are added.
struct PullPage {
    notes: Vec<crate::models::Note>,
    deletions: Vec<DeletionRow>,
    versions: Vec<VersionRow>,
    saved_searches: Vec<crate::models::SavedSearch>,
    total_count: i64,
    has_more: bool,
    next_cursor: Option<String>,
}

/// Where a cursor pull starts from. A cursor this server didn't issue, or one
/// ahead of its sequence because the database was restored from a backup, can't
/// be trusted, so it restarts just as an empty cursor does. That can only send
/// too much, never too little.
async fn cursor_start(
    conn: &mut SqliteConnection,
    user_id: &str,
    cursor: &str,
    last_sync_at: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let last_seq = sqlx::query_scalar!(
        "SELECT last_seq FROM sync_state WHERE user_id = ?",
        user_id
    )
    .fetch_optional(&mut *conn)
    .await?
    .unwrap_or(0);

    if let Some(seq) = decode_cursor(cursor).filter(|seq| *seq <= last_seq) {
        return Ok(seq);
    }

    let Some(last_sync_at) = last_sync_at else {
        return Ok(0);
    };

    // A client moving over from lastSyncAt: start just before its oldest
    // change since then. Rows from there on may be older, which is harmless.
    let since = last_sync_filter(last_sync_at);
    let oldest_new: Option<i64> = sqlx::query_scalar(
        "SELECT MIN(seq) FROM (
             SELECT MIN(change_seq) AS seq FROM notes WHERE user_id = ?1 AND server_modified_at > ?2
             UNION ALL
             SELECT MIN(change_seq) FROM note_deletions WHERE user_id = ?1 AND received_at > ?2
             UNION ALL
             SELECT MIN(change_seq) FROM note_versions WHERE user_id = ?1 AND server_synced_at > ?2
             UNION ALL
             SELECT MIN(change_seq) FROM saved_searches WHERE user_id = ?1 AND synced_at > ?2
         )",
    )
    .bind(user_id)
    .bind(&since)
    .fetch_one(&mut *conn)
    .await?;

    Ok(oldest_new.map_or(last_seq, |seq| seq - 1))
}

/// How many of `notes`, in order, fit in one page's blob budget. Blobs the client
/// holds aren't sent, so they cost nothing, and a blob two notes share is sent once.
async fn notes_within_budget(
    conn: &mut SqliteConnection,
    user_id: &str,
    notes: &[crate::models::Note],
    known_attachments: &HashSet<&str>,
) -> Result<usize, sqlx::Error> {
    let note_ids: Vec<&str> = notes.iter().map(|n| n.id.as_str()).collect();
    let mut blobs: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for (note_id, attachment_id, size) in
        AttachmentRepository::blob_sizes(conn, user_id, &note_ids).await?
    {
        blobs.entry(note_id).or_default().push((attachment_id, size));
    }

    let mut counted = HashSet::new();
    let mut total = 0;
    for (index, note_id) in note_ids.iter().enumerate() {
        let mut size = 0;
        for (attachment_id, blob_size) in blobs.get(*note_id).into_iter().flatten() {
            if !known_attachments.contains(attachment_id.as_str())
                && counted.insert(attachment_id.as_str())
            {
                size += blob_size;
            }
        }

        if total > 0 && total + size > PULL_BLOB_BUDGET_BYTES {
            return Ok(index);
        }
        total += size;
    }

    Ok(notes.len())
}

/// One page of changes after `from`.
///
/// The end of the page is read first, from committed rows, and every query is
/// then bounded by it. Sequence order is commit order, so nothing at or below
/// that bound can still appear: reads need no snapshot and hold no lock, and a
/// row updated meanwhile simply moves past the bound into the next page.
async fn cursor_page(
    conn: &mut SqliteConnection,
    user_id: &str,
    from: i64,
    limit: i64,
    known_attachments: &HashSet<&str>,
) -> Result<PullPage, sqlx::Error> {
    let upcoming: Vec<i64> = sqlx::query_scalar(
        "SELECT seq FROM (
             SELECT change_seq AS seq FROM notes WHERE user_id = ?1 AND change_seq > ?2
             UNION ALL
             SELECT change_seq FROM note_deletions WHERE user_id = ?1 AND change_seq > ?2
             UNION ALL
             SELECT change_seq FROM note_versions WHERE user_id = ?1 AND change_seq > ?2
             UNION ALL
             SELECT change_seq FROM saved_searches WHERE user_id = ?1 AND change_seq > ?2
         )
         ORDER BY seq LIMIT ?3",
    )
    .bind(user_id)
    .bind(from)
    .bind(limit + 1)
    .fetch_all(&mut *conn)
    .await?;

    let mut has_more = upcoming.len() as i64 > limit;
    let mut to = upcoming
        .iter()
        .take(limit as usize)
        .last()
        .copied()
        .unwrap_or(from);

    let mut notes: Vec<crate::models::Note> = sqlx::query_as(&format!(
        "SELECT {NOTE_COLUMNS} FROM notes
         WHERE user_id = ? AND change_seq > ? AND change_seq <= ? ORDER BY change_seq"
    ))
    .bind(user_id)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await?;

    // A page is a range of the sequence, so ending it early means ending the range
    // just before the first note that doesn't fit. Everything else in the range
    // moves to the next page with that note.
    let fitting = notes_within_budget(&mut *conn, user_id, &notes, known_attachments).await?;
    if fitting < notes.len() {
        let first_left_out = &notes[fitting].id;
        let seq: i64 = sqlx::query_scalar("SELECT change_seq FROM notes WHERE id = ? AND user_id = ?")
            .bind(first_left_out)
            .bind(user_id)
            .fetch_one(&mut *conn)
            .await?;
        to = seq - 1;
        has_more = true;
        notes.truncate(fitting);
    }

    let deletions = sqlx::query_as(
        "SELECT id, deleted_at FROM note_deletions
         WHERE user_id = ? AND change_seq > ? AND change_seq <= ? ORDER BY change_seq",
    )
    .bind(user_id)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await?;

    let versions = sqlx::query_as(&format!(
        "SELECT {VERSION_COLUMNS} FROM note_versions
         WHERE user_id = ? AND change_seq > ? AND change_seq <= ? ORDER BY change_seq"
    ))
    .bind(user_id)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await?;

    let saved_searches = sqlx::query_as(&format!(
        "SELECT {SAVED_SEARCH_COLUMNS} FROM saved_searches
         WHERE user_id = ? AND change_seq > ? AND change_seq <= ? ORDER BY change_seq"
    ))
    .bind(user_id)
    .bind(from)
    .bind(to)
    .fetch_all(&mut *conn)
    .await?;

    let total_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notes WHERE user_id = ? AND change_seq > ?")
            .bind(user_id)
            .bind(from)
            .fetch_one(&mut *conn)
            .await?;

    Ok(PullPage {
        notes,
        deletions,
        versions,
        saved_searches,
        total_count,
        has_more,
        next_cursor: Some(encode_cursor(to)),
    })
}

/// Reach a `lastSyncAt` value back by the overlap. A value that doesn't parse is
/// compared as it stands, as it always was.
fn last_sync_filter(last_sync_at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(last_sync_at)
        .map(|time| {
            (time.with_timezone(&chrono::Utc) - chrono::Duration::minutes(LAST_SYNC_OVERLAP_MINUTES))
                .to_rfc3339()
        })
        .unwrap_or_else(|_| last_sync_at.to_string())
}

/// One page for a client without a cursor. Every filter uses a time the server
/// set, because `lastSyncAt` is a point in the server's delivery stream:
/// comparing it with a client's time loses anything pushed late.
async fn last_sync_page(
    conn: &mut SqliteConnection,
    user_id: &str,
    last_sync_at: Option<&str>,
    limit: i64,
    offset: i64,
    known_attachments: &HashSet<&str>,
) -> Result<PullPage, sqlx::Error> {
    // With no timestamp every row matches, which is a full sync
    let since = last_sync_at.map_or_else(String::new, last_sync_filter);

    let mut notes: Vec<crate::models::Note> = sqlx::query_as(&format!(
        "SELECT {NOTE_COLUMNS} FROM notes
         WHERE user_id = ? AND server_modified_at > ?
         ORDER BY server_modified_at, id LIMIT ? OFFSET ?"
    ))
    .bind(user_id)
    .bind(&since)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *conn)
    .await?;

    // Clients advance `offset` by the notes they received, so a short page is safe
    let fitting = notes_within_budget(&mut *conn, user_id, &notes, known_attachments).await?;
    notes.truncate(fitting);

    let total_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notes WHERE user_id = ? AND server_modified_at > ?",
    )
    .bind(user_id)
    .bind(&since)
    .fetch_one(&mut *conn)
    .await?;

    let now_for_expiry = chrono::Utc::now().to_rfc3339();
    let deletions = sqlx::query_as(
        "SELECT id, deleted_at FROM note_deletions
         WHERE user_id = ? AND received_at > ? AND expires_at > ?
         ORDER BY received_at",
    )
    .bind(user_id)
    .bind(&since)
    .bind(&now_for_expiry)
    .fetch_all(&mut *conn)
    .await?;

    let versions = sqlx::query_as(&format!(
        "SELECT {VERSION_COLUMNS} FROM note_versions
         WHERE user_id = ? AND server_synced_at > ?
         ORDER BY note_id, version"
    ))
    .bind(user_id)
    .bind(&since)
    .fetch_all(&mut *conn)
    .await?;

    let saved_searches = sqlx::query_as(&format!(
        r#"SELECT {SAVED_SEARCH_COLUMNS} FROM saved_searches
           WHERE user_id = ? AND synced_at > ?
           ORDER BY "order""#
    ))
    .bind(user_id)
    .bind(&since)
    .fetch_all(&mut *conn)
    .await?;

    let has_more = offset + (notes.len() as i64) < total_count;

    Ok(PullPage {
        notes,
        deletions,
        versions,
        saved_searches,
        total_count,
        has_more,
        next_cursor: None,
    })
}

pub async fn pull(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
    Json(pull_req): Json<SyncPullRequest>,
) -> AppResult<Json<SyncPullResponse>> {
    // Taken before the first read: a client that stores this as lastSyncAt can
    // then only be sent a row twice, never miss one.
    let synced_at = chrono::Utc::now().to_rfc3339();

    let limit = pull_req
        .limit
        .unwrap_or(DEFAULT_PULL_LIMIT)
        .clamp(1, MAX_PULL_LIMIT);
    let offset = pull_req.offset.unwrap_or(0).max(0);
    let known_attachments: HashSet<&str> =
        pull_req.known_attachment_ids.iter().map(String::as_str).collect();

    tracing::info!(
        "Pull from user {} (client {}): cursor={:?}, lastSyncAt={:?}, {} known IDs, limit={}, offset={}",
        client_info.user_id,
        client_info.client_id,
        pull_req.cursor,
        pull_req.last_sync_at,
        pull_req.known_note_ids.len(),
        limit,
        offset
    );

    let mut conn = state.pool.acquire().await?;

    let page = match &pull_req.cursor {
        Some(cursor) => {
            let from = cursor_start(
                &mut conn,
                &client_info.user_id,
                cursor,
                pull_req.last_sync_at.as_deref(),
            )
            .await?;
            cursor_page(&mut conn, &client_info.user_id, from, limit, &known_attachments).await?
        }
        None => {
            last_sync_page(
                &mut conn,
                &client_info.user_id,
                pull_req.last_sync_at.as_deref(),
                limit,
                offset,
                &known_attachments,
            )
            .await?
        }
    };

    let PullPage {
        notes: db_notes,
        deletions,
        versions: db_versions,
        saved_searches: db_saved_searches,
        total_count,
        has_more,
        next_cursor,
    } = page;

    // Collect all note IDs for batch attachment query
    let note_ids: Vec<&str> = db_notes.iter().map(|n| n.id.as_str()).collect();

    // Fetch all attachments for all notes in a single query (avoids N+1 problem)
    let all_attachments =
        AttachmentRepository::refs_for_notes(&mut conn, &client_info.user_id, &note_ids).await?;

    // Group attachments by note_id for O(1) lookup
    // Pre-allocate capacity based on number of notes to avoid reallocations
    let mut attachments_by_note: std::collections::HashMap<String, Vec<crate::models::AttachmentRef>> =
        std::collections::HashMap::with_capacity(db_notes.len());
    let mut needed_attachments = Vec::new();

    for (note_id, attachment) in all_attachments {
        needed_attachments.push(attachment.id.clone());
        attachments_by_note.entry(note_id).or_default().push(attachment);
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
    // Filter out known attachments first, then batch fetch remaining
    let unknown_attachments: Vec<&str> = needed_attachments
        .iter()
        .filter(|att_id| !known_attachments.contains(att_id.as_str()))
        .map(|s| s.as_str())
        .collect();

    // Batch fetch all unknown attachments in a single query
    let attachments_data: Vec<SyncAttachmentData> =
        AttachmentRepository::blobs(&mut conn, &client_info.user_id, &unknown_attachments)
            .await?
            .into_iter()
            .map(|(id, data)| {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                tracing::debug!("Sending attachment {} to client", id);
                SyncAttachmentData { id, data: encoded }
            })
            .collect();

    let deletions: Vec<crate::models::SyncDeletion> = deletions
        .into_iter()
        .map(|row| crate::models::SyncDeletion {
            id: row.id,
            deleted_at: row.deleted_at,
        })
        .collect();

    if !deletions.is_empty() {
        tracing::info!("Returning {} deletions to client", deletions.len());
    }

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
    .fetch_all(&mut *conn)
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
        next_cursor,
        inbox_items: inbox_items_opt,
        inbox_count: inbox_count_opt,
    }))
}

pub async fn delete_note(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
    Path(note_id): Path<String>,
) -> AppResult<StatusCode> {

    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;

    // Delete note for this user (cascades to attachments via foreign keys)
    // Filter by user_id to ensure users can only delete their own notes
    let deleted = sqlx::query!(
        "DELETE FROM notes WHERE id = ? AND user_id = ?",
        note_id,
        client_info.user_id
    )
    .execute(&mut *tx)
    .await?;

    if deleted.rows_affected() > 0 {
        let now = chrono::Utc::now().to_rfc3339();
        record_tombstone(
            &mut tx,
            &client_info.user_id,
            &note_id,
            &now,
            Some(&client_info.client_id),
            &now,
        )
        .await?;
    }

    tx.commit().await?;

    tracing::info!(
        "Deleted note: {:?} for user: {} (client: {})",
        note_id,
        client_info.user_id,
        client_info.client_id
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Fetch a single attachment blob the client is missing locally (vault health
/// repair). Lookups are scoped to the caller, so unknown or foreign attachments
/// return 404 and existence is not leaked.
pub async fn get_attachment(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
    Path(attachment_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let mut conn = state.pool.acquire().await?;
    let blob = AttachmentRepository::blobs(
        &mut conn,
        &client_info.user_id,
        &[attachment_id.as_str()],
    )
    .await?
    .into_iter()
    .next();
    drop(conn);

    match blob {
        Some((_, data)) => {
            tracing::info!(
                "Serving attachment {:?} ({} bytes) to user {} (client {})",
                attachment_id,
                data.len(),
                client_info.user_id,
                client_info.client_id
            );
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            Ok(Json(serde_json::json!({ "id": attachment_id, "data": encoded })))
        }
        None => Err(AppError::NotFound("Attachment not found".to_string())),
    }
}

/// Revoke the calling device.
///
/// Authenticated with the device's own API key, so a client can unlink itself
/// without the user re-entering their account password. The middleware only
/// admits active devices, so this is always the soft-revoke path — the device
/// remains listed for the user (and hard-deletable) in the web UI.
pub async fn revoke_self(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
) -> AppResult<StatusCode> {
    sqlx::query!(
        "UPDATE clients SET is_active = 0 WHERE id = ?",
        client_info.client_id
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to self-revoke device: {}", e);
        AppError::InternalServerError
    })?;

    tracing::info!(
        "Device {} self-revoked (user {})",
        client_info.client_id,
        client_info.user_id
    );

    Ok(StatusCode::NO_CONTENT)
}
