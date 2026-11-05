//! Sync operations for bidirectional sync with server

use anyhow::{Context, Result};
use rust_i18n::t;
use std::time::Instant;

use crate::{
    crypto::{CryptoService, EncryptedData},
    models::{Attachment, Note, sync::{SyncPushRequest, SyncPullRequest, SyncNote, SyncPushResponse, SyncPullResponse, AttachmentRef, SyncAttachment, SyncNoteVersion, SyncDeletion}},
    repository::{attachment::AttachmentRepository, NoteRepository, NoteVersionRepository, VersionReason, sync::SyncRepository},
};

use super::super::app::App;

/// Notes to ask for per pull request.
const PULL_PAGE_SIZE: i64 = 100;

/// What a sync achieved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SyncOutcome {
    /// Notes pushed, or changed by the pull
    pub synced: usize,
    /// Notes the server refused because of a conflict
    pub rejected: usize,
    /// Notes or attachments from the server that couldn't be read, and were skipped
    pub skipped: usize,
}

/// A filename as it goes to the server: encrypted, like everything else in a note.
///
/// This device stores filenames in plaintext, but a version pulled by an older
/// build can hold one that's still encrypted, which goes back as it is.
fn filename_for_sync(crypto: &CryptoService, filename: &str, key: &[u8; 32]) -> Result<String> {
    if serde_json::from_str::<EncryptedData>(filename).is_ok() {
        return Ok(filename.to_string());
    }
    Ok(serde_json::to_string(&crypto.encrypt_text(filename, key)?)?)
}

/// Read a filename from the server.
///
/// Clients encrypt filenames, but older TUI builds pushed them in plaintext, and
/// those stay on the server. Text that isn't ciphertext is taken as one of those.
/// Ciphertext that won't decrypt is an error, not a name.
pub fn filename_from_sync(crypto: &CryptoService, filename: &str, key: &[u8; 32]) -> Result<String> {
    let Ok(encrypted) = serde_json::from_str::<EncryptedData>(filename) else {
        return Ok(filename.to_string());
    };

    let decrypted = crypto.decrypt_text(&encrypted, key)?;

    // Try JSON first (some clients encode it), and fall back to the text itself
    Ok(serde_json::from_str(&decrypted).unwrap_or(decrypted))
}

/// Attachment references for a push.
fn attachment_refs(crypto: &CryptoService, attachments: &[Attachment], key: &[u8; 32]) -> Result<Vec<AttachmentRef>> {
    attachments.iter()
        .map(|attachment| Ok(AttachmentRef {
            id: attachment.id.clone(),
            filename: filename_for_sync(crypto, &attachment.filename, key)?,
            mime_type: attachment.mime_type.clone(),
            size: attachment.size,
            data: attachment.data.clone(),
        }))
        .collect()
}

/// What applying a page of pulled notes needs.
struct PullPage<'a> {
    crypto: &'a CryptoService,
    key: &'a [u8; 32],
    note_repo: &'a NoteRepository<'a>,
    version_repo: &'a NoteVersionRepository<'a>,
    attachment_repo: &'a AttachmentRepository<'a>,
    attachment_data: &'a std::collections::HashMap<String, Vec<u8>>,
    synced_at: chrono::DateTime<chrono::Utc>,
    force: bool,
    log: &'a dyn Fn(&str),
}

/// A note's tags as sent by the server: each one encrypted on its own, holding
/// a JSON string, or a JSON array in the older format.
fn decrypt_note_tags(page: &PullPage<'_>, tags: &[String]) -> Vec<String> {
    tags.iter()
        .flat_map(|tag_json| {
            // Parse and decrypt the tag
            let encrypted_tag: EncryptedData = serde_json::from_str(tag_json).ok()?;
            let tag_json_str = page.crypto.decrypt_text(&encrypted_tag, page.key).ok()?;

            // Try parsing as individual string first (new format)
            if let Ok(tag) = serde_json::from_str::<String>(&tag_json_str) {
                if !tag.trim().is_empty() {
                    return Some(vec![tag]);
                }
            }

            // Try parsing as array (legacy format)
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tag_json_str) {
                let valid_tags: Vec<String> = tags.into_iter()
                    .filter(|t| !t.trim().is_empty())
                    .collect();
                if !valid_tags.is_empty() {
                    return Some(valid_tags);
                }
            }

            None
        })
        .flatten()
        .collect()
}

/// Store the attachments the server sent for a note, returning those that made it
/// and how many couldn't be read.
fn store_note_attachments(page: &PullPage<'_>, remote_note: &SyncNote) -> Result<(Vec<Attachment>, usize)> {
    let mut note_attachments: Vec<Attachment> = Vec::new();
    let mut unreadable = 0;

    for attachment_ref in &remote_note.attachments {
        // Without data here, the server left it out because this device has it
        let Some(decrypted_data) = page.attachment_data.get(&attachment_ref.id) else {
            continue;
        };

        let filename = match filename_from_sync(page.crypto, &attachment_ref.filename, page.key) {
            Ok(filename) => filename,
            Err(e) => {
                (page.log)(&format!("Pull - Failed to read filename for {}: {}", attachment_ref.id, e));
                unreadable += 1;
                continue;
            }
        };

        // Store in database
        page.attachment_repo.store(
            &attachment_ref.id,
            &filename,
            &attachment_ref.mime_type,
            attachment_ref.size,
            decrypted_data,
            page.key,
        )?;

        // Add to note's attachment array
        note_attachments.push(Attachment {
            id: attachment_ref.id.clone(),
            filename,
            mime_type: attachment_ref.mime_type.clone(),
            size: attachment_ref.size,
            data: attachment_ref.data.clone(),
            thumbnail_data: None,
        });
    }

    Ok((note_attachments, unreadable))
}

/// Apply one pulled note, returning whether it changed anything here and how many
/// of its attachments couldn't be read.
fn apply_remote_note(page: &PullPage<'_>, notes: &mut Vec<Note>, remote_note: SyncNote) -> Result<(bool, usize)> {
    // Decrypt content and tags from server (they're stored encrypted on server)
    let encrypted_content: EncryptedData = serde_json::from_str(&remote_note.content)?;
    let decrypted_content = page.crypto.decrypt_text(&encrypted_content, page.key)?;
    let decrypted_tags = decrypt_note_tags(page, &remote_note.tags);

    let (mut note_attachments, unreadable) = store_note_attachments(page, &remote_note)?;

    // Check if we have this note in the database (not just in-memory list)
    let existing_note = page.note_repo.get(&remote_note.id, page.key)?;

    // Keep any local attachment the server didn't resend because we already had it
    if let Some(ref local_note) = existing_note {
        for attachment_ref in &remote_note.attachments {
            if !note_attachments.iter().any(|a| a.id == attachment_ref.id) {
                if let Some(local_att) = local_note.attachments.iter().find(|a| a.id == attachment_ref.id) {
                    note_attachments.push(local_att.clone());
                } else {
                    (page.log)(&format!("Pull - Attachment {} not found locally either", attachment_ref.id));
                }
            }
        }
    }

    if let Some(mut local_note) = existing_note {
        // Conflict resolution: Last-Write-Wins, but also update if attachments differ
        // During force sync, always update to ensure we pull latest from server
        let should_update = page.force
            || remote_note.modified_at > local_note.modified_at
            || note_attachments.len() != local_note.attachments.len();

        if !should_update {
            return Ok((false, unreadable));
        }

        // Capture local version BEFORE overwriting with remote
        let _ = page.version_repo.create_version(&local_note, page.synced_at, VersionReason::Sync, page.key);

        // Remote is newer, update local with decrypted content
        local_note.content = decrypted_content;
        local_note.tags = decrypted_tags;
        local_note.attachments = note_attachments;
        local_note.modified_at = remote_note.modified_at;
        local_note.synced_at = Some(page.synced_at); // Mark as synced
        local_note.pinned = remote_note.pinned;
        local_note.archived = remote_note.archived;
        local_note.archived_at = remote_note.archived_at;
        local_note.locked = remote_note.locked;
        local_note.locked_at = remote_note.locked_at;
        local_note.deleted = remote_note.deleted;
        local_note.deleted_at = remote_note.deleted_at;
        local_note.version = remote_note.version;
        local_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
        if let Some(lang_str) = &remote_note.syntax_language {
            local_note.syntax_language = lang_str.parse().unwrap_or_default();
        }
        local_note.color = remote_note.color.clone();

        page.note_repo.update(&local_note, page.key)?;

        // Also update in-memory list if present
        if let Some(mem_note) = notes.iter_mut().find(|n| n.id == remote_note.id) {
            *mem_note = local_note;
        }
    } else {
        // New note from server, add it with decrypted content
        let mut new_note = Note::new(decrypted_content);
        new_note.id = remote_note.id.clone();
        new_note.created_at = remote_note.created_at;
        new_note.modified_at = remote_note.modified_at;
        new_note.synced_at = Some(page.synced_at); // Mark as synced
        new_note.tags = decrypted_tags;
        new_note.attachments = note_attachments;
        new_note.pinned = remote_note.pinned;
        new_note.archived = remote_note.archived;
        new_note.archived_at = remote_note.archived_at;
        new_note.locked = remote_note.locked;
        new_note.locked_at = remote_note.locked_at;
        new_note.deleted = remote_note.deleted;
        new_note.deleted_at = remote_note.deleted_at;
        new_note.version = remote_note.version;
        new_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
        if let Some(lang_str) = &remote_note.syntax_language {
            new_note.syntax_language = lang_str.parse().unwrap_or_default();
        }
        new_note.color = remote_note.color.clone();

        page.note_repo.create(&new_note, page.key)?;

        // Add to in-memory list only if not deleted
        if !new_note.deleted {
            notes.insert(0, new_note);
        }
    }

    Ok((true, unreadable))
}

/// Trigger manual sync
pub fn trigger_sync(app: &mut App) {
    app.debug_log("trigger_sync - Called");
    app.debug_log(&format!("trigger_sync - sync_enabled: {}", app.settings.sync_enabled));
    app.debug_log(&format!("trigger_sync - sync_endpoint: {:?}", app.settings.sync_endpoint));

    // Check if sync is configured
    if !app.settings.sync_enabled {
        app.debug_log("trigger_sync - Sync not enabled, returning");
        app.sync_status = Some(t!("sync.not_enabled").to_string());
        app.sync_status_set_at = Some(Instant::now());
        return;
    }

    if app.settings.sync_endpoint.is_none() {
        app.debug_log("trigger_sync - Sync endpoint not configured, returning");
        app.sync_status = Some(t!("sync.endpoint_not_configured").to_string());
        app.sync_status_set_at = Some(Instant::now());
        return;
    }

    // Perform sync
    app.debug_log("trigger_sync - Starting sync");
    app.sync_status = Some(t!("status.syncing").to_string());
    app.sync_status_set_at = Some(Instant::now());

    match perform_sync(app, false) {
        Ok(outcome) => {
            // Reload notes from database to pick up sync changes
            if let Err(e) = super::notes::load_notes(app) {
                app.error = Some(format!("Sync succeeded but failed to reload notes: {}", e));
            }
            // Refresh conflict cache after sync
            app.refresh_conflict_cache();
            let conflict_count = app.conflict_note_ids.len();

            let result = outcome.synced;
            let unit = if result == 1 { t!("sync.note").to_string() } else { t!("sync.notes").to_string() };
            let status_msg = if conflict_count > 0 {
                let conflict_word = if conflict_count == 1 {
                    t!("conflict.count_singular").to_string()
                } else {
                    t!("conflict.count_plural").to_string()
                };
                format!("{} ({} {})",
                    t!("sync.complete", count = result, unit = unit),
                    conflict_count,
                    conflict_word
                )
            } else {
                t!("sync.complete", count = result, unit = unit).to_string()
            };
            app.sync_status = Some(status_msg);
            app.sync_status_set_at = Some(Instant::now());
            report_skipped(app, &outcome);
        }
        Err(e) => {
            reload_after_failed_sync(app);
            app.error = Some(format!("Sync failed: {}", e));
            app.sync_status = Some(format!("Sync failed: {}", e));
            app.sync_status_set_at = Some(Instant::now());
        }
    }
}

/// Tell the user about anything the sync had to leave behind.
fn report_skipped(app: &mut App, outcome: &SyncOutcome) {
    if outcome.skipped > 0 {
        app.error = Some(t!("sync.skipped_notes", count = outcome.skipped).to_string());
    }
}

/// A failed pull rolls back its database changes but not the list in memory.
fn reload_after_failed_sync(app: &mut App) {
    if let Err(e) = super::notes::load_notes(app) {
        app.debug_log(&format!("Could not reload notes after a failed sync: {}", e));
    }
}

/// Force full resync from server
pub fn force_full_sync(app: &mut App) {
    app.debug_log("force_full_sync - Called");

    // Check if sync is configured
    if !app.settings.sync_enabled {
        app.debug_log("force_full_sync - Sync not enabled, returning");
        app.sync_status = Some(t!("sync.not_enabled").to_string());
        app.sync_status_set_at = Some(Instant::now());
        return;
    }

    if app.settings.sync_endpoint.is_none() {
        app.debug_log("force_full_sync - Sync endpoint not configured, returning");
        app.sync_status = Some(t!("sync.endpoint_not_configured").to_string());
        app.sync_status_set_at = Some(Instant::now());
        return;
    }

    // Perform force sync
    app.debug_log("force_full_sync - Starting force full sync");
    app.sync_status = Some(t!("sync.force_syncing").to_string());
    app.sync_status_set_at = Some(Instant::now());

    match perform_sync(app, true) {
        Ok(outcome) => {
            // Reload notes from database to pick up sync changes
            if let Err(e) = super::notes::load_notes(app) {
                app.error = Some(format!("Sync succeeded but failed to reload notes: {}", e));
            }
            // Refresh conflict cache after sync
            app.refresh_conflict_cache();
            let conflict_count = app.conflict_note_ids.len();

            let result = outcome.synced;
            let unit = if result == 1 { t!("sync.note").to_string() } else { t!("sync.notes").to_string() };
            let status_msg = if conflict_count > 0 {
                let conflict_word = if conflict_count == 1 {
                    t!("conflict.count_singular").to_string()
                } else {
                    t!("conflict.count_plural").to_string()
                };
                format!("{} ({} {})",
                    t!("sync.force_complete", count = result, unit = unit),
                    conflict_count,
                    conflict_word
                )
            } else {
                t!("sync.force_complete", count = result, unit = unit).to_string()
            };
            app.sync_status = Some(status_msg);
            app.sync_status_set_at = Some(Instant::now());
            report_skipped(app, &outcome);
        }
        Err(e) => {
            reload_after_failed_sync(app);
            app.error = Some(format!("Force sync failed: {}", e));
            app.sync_status = Some(format!("Force sync failed: {}", e));
            app.sync_status_set_at = Some(Instant::now());
        }
    }
}

/// Perform bidirectional sync with server
/// If force is true, pulls all notes from server regardless of last sync time
pub fn perform_sync(app: &mut App, force: bool) -> Result<SyncOutcome> {
    use chrono::Utc;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    app.debug_log(&format!("[Sync] Starting sync (force={}), key fingerprint: {:02x}{:02x}{:02x}{:02x}",
        force, key[0], key[1], key[2], key[3]));

    let sync_repo = SyncRepository::new(db.connection());
    let note_repo = NoteRepository::new(db.connection());
    let version_repo = NoteVersionRepository::new(db.connection());

    // Get sync metadata
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
    app.debug_log(&format!("[Sync] Metadata: endpoint={}, has_api_key={}, sync_enabled={}",
        metadata.sync_endpoint, metadata.api_key.is_some(), metadata.sync_enabled));

    // Get API key
    let encrypted_api_key = metadata.api_key.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No API key configured"))?;
    let api_key_encrypted: crate::crypto::EncryptedData = serde_json::from_str(encrypted_api_key)
        .map_err(|e| {
            app.debug_log(&format!("[Sync] Failed to parse API key JSON: {}", e));
            anyhow::anyhow!("Invalid API key format: {}", e)
        })?;
    let api_key = app.crypto.decrypt_text(&api_key_encrypted, key)
        .map_err(|e| {
            app.debug_log(&format!("[Sync] Failed to decrypt API key: {}", e));
            e
        })?;
    app.debug_log("[Sync] API key decrypted successfully");

    let endpoint = metadata.sync_endpoint.clone();

    let debug_log = app.debug_log.clone();
    let log = move |message: &str| crate::ui::app::write_debug_log(&debug_log, message);

    // PUSH: Send local changes to server
    let last_sync = metadata.last_sync_at;
    let notes_to_push = if let Some(last_sync) = last_sync {
        note_repo.get_modified_after(last_sync, key)?
    } else {
        // Include deleted notes in full sync so soft-deleted notes sync to other devices
        note_repo.list(true, key)?
    };

    let mut outcome = SyncOutcome::default();

    // Collect pending deletions (from emptying trash). These go even when no note
    // changed, or an emptied bin would wait for the next edit to reach the server.
    let pending_deletions = note_repo.get_pending_deletions()?;

    if !notes_to_push.is_empty() || !pending_deletions.is_empty() {
        use base64::{Engine as _, engine::general_purpose};

        // Collect all unique attachment IDs that need to be pushed
        let mut attachment_ids_to_push: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Convert notes to sync format, encrypting content and tags
        let sync_notes: Result<Vec<SyncNote>> = notes_to_push.iter().map(|note| {
            // Encrypt content and tags for transmission to server
            let encrypted_content = app.crypto.encrypt_text(&note.content, key)?;
            let content_json = serde_json::to_string(&encrypted_content)?;

            let encrypted_tags: Result<Vec<String>> = note.tags.iter()
                .map(|tag| {
                    // JSON-encode the tag first, then encrypt it
                    let tag_json = serde_json::to_string(tag)?;
                    let encrypted_tag = app.crypto.encrypt_text(&tag_json, key)?;
                    Ok(serde_json::to_string(&encrypted_tag)?)
                })
                .collect();

            // Build attachment references from note.attachments
            attachment_ids_to_push.extend(note.attachments.iter().map(|att| att.id.clone()));

            Ok(SyncNote {
                id: note.id.clone(),
                created_at: note.created_at,
                modified_at: note.modified_at,
                content: content_json,
                tags: encrypted_tags?,
                attachments: attachment_refs(&app.crypto, &note.attachments, key)?,
                pinned: note.pinned,
                archived: note.archived,
                archived_at: note.archived_at,
                locked: note.locked,
                locked_at: note.locked_at,
                deleted: note.deleted,
                deleted_at: note.deleted_at,
                version: note.version,
                word_wrap: Some(note.word_wrap),
                syntax_language: Some(note.syntax_language.to_string()),
                color: note.color.clone(),
                // Hash chain fields for git-like conflict detection
                content_hash: note.content_hash.clone(),
                parent_hash: note.parent_hash.clone(),
                hash_chain: note.hash_chain.clone(),
            })
        }).collect();

        let sync_notes = sync_notes?;

        // Collect versions for all notes being pushed
        let mut sync_versions: Vec<SyncNoteVersion> = Vec::new();

        for note in &notes_to_push {
            let note_versions = version_repo.get_versions_for_note(&note.id, key)?;

            for version in note_versions {
                // Encrypt content and tags for transmission
                let encrypted_content = app.crypto.encrypt_text(&version.content, key)?;
                let content_json = serde_json::to_string(&encrypted_content)?;

                let encrypted_tags: Result<Vec<String>> = version.tags.iter()
                    .map(|tag| {
                        let tag_json = serde_json::to_string(tag)?;
                        let encrypted_tag = app.crypto.encrypt_text(&tag_json, key)?;
                        Ok(serde_json::to_string(&encrypted_tag)?)
                    })
                    .collect();

                sync_versions.push(SyncNoteVersion {
                    version_key: format!("{}:{}", version.note_id, version.version),
                    note_id: version.note_id.clone(),
                    version: version.version,
                    created_at: version.created_at,
                    synced_at: version.synced_at,
                    content: content_json,
                    tags: encrypted_tags?,
                    attachments: attachment_refs(&app.crypto, &version.attachments, key)?,
                    syntax_language: version.syntax_language.as_ref().map(|s| s.to_string()),
                    word_wrap: version.word_wrap,
                    show_preview: version.show_preview,
                    color: version.color.clone(),
                    reason: version.reason.to_string(),
                    // Hash chain fields
                    content_hash: version.content_hash.clone(),
                    parent_hash: version.parent_hash.clone(),
                });
            }
        }

        // Fetch binary data for all attachments that need to be pushed.
        // A missing blob no longer aborts the whole push: the note syncs
        // without it, the server responds with an attachmentWarnings entry,
        // and any device still holding the blob heals the gap.
        let attachment_repo = AttachmentRepository::new(db.connection());
        let sync_attachments: Vec<SyncAttachment> = attachment_ids_to_push
            .iter()
            .filter_map(|att_id| {
                let encrypted_data = match attachment_repo.get(att_id, key) {
                    Ok(Some((_filename, _mime_type, _size, data))) => data,
                    Ok(None) => {
                        tracing::warn!(
                            "Attachment {} has no local blob — pushing note without it",
                            att_id
                        );
                        return None;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Attachment {} could not be read ({}) — pushing note without it",
                            att_id,
                            e
                        );
                        return None;
                    }
                };
                let encrypted_blob = app.crypto.encrypt_binary(&encrypted_data, key).ok()?;
                let payload = serde_json::to_vec(&encrypted_blob).ok()?;
                Some(SyncAttachment {
                    id: att_id.clone(),
                    data: general_purpose::STANDARD.encode(payload),
                })
            })
            .collect();

        let sync_deletions: Vec<SyncDeletion> = pending_deletions
            .iter()
            .map(|(id, deleted_at)| SyncDeletion {
                id: id.clone(),
                deleted_at: deleted_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
            })
            .collect();

        let push_request = SyncPushRequest {
            notes: sync_notes,
            attachments: sync_attachments,
            versions: sync_versions,
            deletions: sync_deletions,
        };

        // Create HTTP client
        let client = reqwest::blocking::Client::new();
        let push_url = format!("{}/api/v1/sync/push", endpoint);

        let response = client
            .post(&push_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&push_request)
            .send()
            .context("Failed to send push request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());

            // Provide user-friendly error messages
            let error_msg = if status == 403 {
                "Your account has been deactivated or is pending admin approval. Please contact the administrator."
            } else if status == 401 {
                "Invalid API key or authentication failed. Try re-registering your device."
            } else if status == 409 {
                "Sync conflict detected. Some notes have conflicting changes on the server."
            } else {
                &error_body
            };

            anyhow::bail!("Push failed: {}", error_msg);
        }

        let push_response = SyncPushResponse::parse(&response.text().context("Failed to read push response")?)
            .context("Failed to parse push response")?;

        outcome.synced += push_response.accepted.len();
        outcome.rejected += push_response.rejected.len();

        // Create version snapshots for accepted notes
        for accepted in &push_response.accepted {
            if let Ok(Some(note)) = note_repo.get(&accepted.id, key) {
                let _ = version_repo.create_version(&note, accepted.synced_at, VersionReason::Sync, key);
            }
        }

        // Store conflict data for rejected notes
        for rejected in &push_response.rejected {
            app.debug_log(&format!("Sync conflict for note {}: {}", rejected.id, rejected.reason));

            // Create ConflictData from the SyncRejected response
            let conflict_data = crate::models::sync::ConflictData {
                note_id: rejected.id.clone(),
                server_content: rejected.server_content.clone(),
                server_tags: rejected.server_tags.clone(),
                server_modified_at: rejected.server_modified_at,
                server_version: rejected.server_version,
                server_attachments: rejected.server_attachments.clone(),
                server_pinned: rejected.server_pinned,
                server_archived: rejected.server_archived,
                server_archived_at: rejected.server_archived_at,
                server_locked: rejected.server_locked,
                server_locked_at: rejected.server_locked_at,
                server_syntax_language: rejected.server_syntax_language.clone(),
                server_word_wrap: rejected.server_word_wrap,
                server_color: rejected.server_color.clone(),
                detected_at: Utc::now(),
                // Hash chain fields for git-like conflict detection
                server_content_hash: rejected.server_content_hash.clone(),
                server_parent_hash: rejected.server_parent_hash.clone(),
                server_hash_chain: rejected.server_hash_chain.clone(),
                // Ancestor data for 3-way merge
                ancestor_hash: rejected.ancestor_hash.clone(),
                ancestor_content: rejected.ancestor_content.clone(),
                ancestor_tags: rejected.ancestor_tags.clone(),
            };

            // Store conflict in sync metadata
            if let Err(e) = sync_repo.set_conflict(&rejected.id, &conflict_data) {
                app.debug_log(&format!("Failed to store conflict for note {}: {}", rejected.id, e));
            }
        }

        // React to server attachment warnings: re-push blobs this device
        // holds (one attachments-only push, no in-run retry), log the rest.
        if !push_response.attachment_warnings.is_empty() {
            let mut repair_attachments: Vec<SyncAttachment> = Vec::new();
            for warning in &push_response.attachment_warnings {
                for att_id in &warning.attachment_ids {
                    match attachment_repo.get(att_id, key) {
                        Ok(Some((_f, _m, _s, encrypted_data))) => {
                            match app.crypto.encrypt_binary(&encrypted_data, key)
                                .and_then(|blob| serde_json::to_vec(&blob).map_err(Into::into))
                            {
                                Ok(payload) => {
                                    tracing::warn!(
                                        "Server is missing attachment {} (note {}) — re-uploading",
                                        att_id, warning.note_id
                                    );
                                    repair_attachments.push(SyncAttachment {
                                        id: att_id.clone(),
                                        data: general_purpose::STANDARD.encode(payload),
                                    });
                                }
                                Err(e) => tracing::warn!(
                                    "Could not prepare attachment {} for repair: {}", att_id, e
                                ),
                            }
                        }
                        _ => tracing::warn!(
                            "Server has no data for attachment {} (note {}) and neither does this device",
                            att_id, warning.note_id
                        ),
                    }
                }
            }
            if !repair_attachments.is_empty() {
                let repair_request = SyncPushRequest {
                    notes: Vec::new(),
                    attachments: repair_attachments,
                    versions: Vec::new(),
                    deletions: Vec::new(),
                };
                match client
                    .post(&push_url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&repair_request)
                    .send()
                {
                    Ok(resp) if resp.status().is_success() => {
                        app.debug_log("[Sync] Re-uploaded attachments the server was missing");
                    }
                    Ok(resp) => tracing::warn!(
                        "Attachment repair push failed with status {}", resp.status()
                    ),
                    Err(e) => tracing::warn!("Attachment repair push failed: {}", e),
                }
            }
        }

        // Mark deletions as synced (push was successful)
        if !pending_deletions.is_empty() {
            let deletion_ids: Vec<String> = pending_deletions.iter().map(|(id, _)| id.clone()).collect();
            if let Err(e) = note_repo.mark_deletions_synced(&deletion_ids) {
                app.debug_log(&format!("Failed to mark deletions as synced: {}", e));
            } else {
                app.debug_log(&format!("Marked {} deletions as synced", deletion_ids.len()));
            }
        }

        // Update last push timestamp
        metadata.last_push_at = Some(Utc::now());
    }

    // PULL: Get changes from server
    let (last_sync_for_pull, known_note_ids, known_attachment_ids) = if force {
        // Force full sync: request all notes and attachments from server
        (None, vec![], vec![])
    } else {
        // Normal sync: use last sync time, known note IDs, and known attachment IDs
        let known_ids = app.notes.iter().map(|n| n.id.clone()).collect();

        // Collect all attachment IDs we already have locally
        let known_att_ids: Vec<String> = app.notes.iter()
            .flat_map(|note| note.attachments.iter().map(|att| att.id.clone()))
            .collect();

        (metadata.last_pull_at.or(last_sync), known_ids, known_att_ids)
    };

    let pull_url = format!("{}/api/v1/sync/pull", endpoint);
    let client = reqwest::blocking::Client::new();

    // The server sends one page of notes at a time. Versions, deletions and the
    // inbox come whole with every page, so they're applied once, from the last
    // page, when all the notes they refer to exist.
    let mut offset: i64 = 0;
    let mut first_page_at = None;
    loop {
        let pull_request = SyncPullRequest {
            last_sync_at: last_sync_for_pull,
            known_note_ids: known_note_ids.clone(),
            known_attachment_ids: known_attachment_ids.clone(),
            limit: Some(PULL_PAGE_SIZE),
            offset: Some(offset),
        };

        let response = client
            .post(&pull_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&pull_request)
            .send()
            .context("Failed to send pull request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());

            // Provide user-friendly error messages
            let error_msg = if status == 403 {
                "Your account has been deactivated or is pending admin approval. Please contact the administrator."
            } else if status == 401 {
                "Invalid API key or authentication failed. Try re-registering your device."
            } else {
                &error_body
            };

            anyhow::bail!("Pull failed: {}", error_msg);
        }

        // Parse the JSON response
        let response_text = response.text()
            .context("Failed to read pull response text")?;
        let pull_response = SyncPullResponse::parse(&response_text)
            .context("Failed to parse pull response")?;
        outcome.skipped += pull_response.dropped;

        let page_note_count = pull_response.notes.len() as i64;
        let is_last_page = !pull_response.has_more || page_note_count == 0;
        first_page_at.get_or_insert(pull_response.synced_at);

        use base64::{Engine as _, engine::general_purpose};
        let attachment_repo = AttachmentRepository::new(db.connection());

        // Decrypt attachments and build a map for quick lookup
        let mut attachment_data_map: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
        for sync_attachment in &pull_response.attachments {
            if let Ok(decoded_data) = general_purpose::STANDARD.decode(&sync_attachment.data) {
                if let Ok(encrypted_blob) = serde_json::from_slice::<EncryptedData>(&decoded_data) {
                    if let Ok(decrypted_data) = app.crypto.decrypt_binary(&encrypted_blob, key) {
                        attachment_data_map.insert(sync_attachment.id.clone(), decrypted_data);
                    }
                }
            }
        }

        // A page applies whole or not at all. The pull cursor only moves at the end
        // of the sync, so an interrupted page is simply fetched again.
        let transaction = db.connection().unchecked_transaction()?;

        let page = PullPage {
            crypto: &app.crypto,
            key,
            note_repo: &note_repo,
            version_repo: &version_repo,
            attachment_repo: &attachment_repo,
            attachment_data: &attachment_data_map,
            synced_at: pull_response.synced_at,
            force,
            log: &log,
        };

        // A note that can't be read costs that note, not the sync. Stopping here
        // would hold the cursor back and block every later change for good.
        for remote_note in pull_response.notes {
            let note_id = remote_note.id.clone();
            match apply_remote_note(&page, &mut app.notes, remote_note) {
                Ok((changed, unreadable_attachments)) => {
                    outcome.synced += changed as usize;
                    outcome.skipped += unreadable_attachments;
                }
                Err(e) => {
                    tracing::warn!("Skipping note {} from the server: {}", note_id, e);
                    log(&format!("Pull - Skipping note {}: {}", note_id, e));
                    outcome.skipped += 1;
                }
            }
        }

        if is_last_page {
            // Process incoming versions from server
            app.debug_log(&format!("Pull - Received {} versions from server", pull_response.versions.len()));

            for server_version in &pull_response.versions {
                app.debug_log(&format!("Pull - Processing version: {} (v{})", server_version.version_key, server_version.version));

                // Check if this version already exists
                let existing_version = version_repo.get_version_by_key(&server_version.version_key)?;

                if existing_version.is_none() {
                    // New version from server - decrypt and store it locally

                    // Decrypt content
                    let encrypted_content: crate::crypto::EncryptedData = match serde_json::from_str(&server_version.content) {
                        Ok(data) => data,
                        Err(e) => {
                            app.debug_log(&format!("Pull - Failed to parse version content: {}, skipping", e));
                            continue;
                        }
                    };

                    let decrypted_content = match app.crypto.decrypt_text(&encrypted_content, key) {
                        Ok(content) => content,
                        Err(e) => {
                            app.debug_log(&format!("Pull - Failed to decrypt version content: {}, skipping", e));
                            continue;
                        }
                    };

                    // Decrypt tags
                    let decrypted_tags: Vec<String> = server_version.tags.iter()
                        .flat_map(|tag_json| {
                            let encrypted_tag: crate::crypto::EncryptedData = serde_json::from_str(tag_json).ok()?;
                            let tag_json_str = app.crypto.decrypt_text(&encrypted_tag, key).ok()?;
                            serde_json::from_str::<String>(&tag_json_str).ok()
                        })
                        .collect();

                    // Convert attachment refs
                    let version_attachments: Vec<Attachment> = server_version.attachments.iter().filter_map(|att_ref| {
                        Some(Attachment {
                            id: att_ref.id.clone(),
                            filename: filename_from_sync(&app.crypto, &att_ref.filename, key).ok()?,
                            mime_type: att_ref.mime_type.clone(),
                            size: att_ref.size,
                            data: att_ref.data.clone(),
                            thumbnail_data: None,
                        })
                    }).collect();

                    // Parse version reason
                    let reason = if server_version.reason == "manual-sync" {
                        VersionReason::ManualSync
                    } else {
                        VersionReason::Sync
                    };

                    // Create local version
                    let local_version = crate::repository::NoteVersion {
                        version_key: server_version.version_key.clone(),
                        note_id: server_version.note_id.clone(),
                        version: server_version.version,
                        created_at: server_version.created_at,
                        synced_at: server_version.synced_at,
                        content: decrypted_content,
                        tags: decrypted_tags,
                        attachments: version_attachments,
                        syntax_language: server_version.syntax_language
                            .as_ref()
                            .and_then(|s| s.parse().ok()),
                        word_wrap: Some(server_version.word_wrap.unwrap_or(true)),
                        show_preview: server_version.show_preview,
                        color: server_version.color.clone(),
                        reason,
                        // Hash chain fields from server
                        content_hash: server_version.content_hash.clone(),
                        parent_hash: server_version.parent_hash.clone(),
                    };

                    // Store the version
                    if let Err(e) = version_repo.insert_version_from_sync(&local_version, key) {
                        app.debug_log(&format!("Pull - Failed to store version {}: {}", server_version.version_key, e));
                    } else {
                        app.debug_log(&format!("Pull - Stored version from server: {}", server_version.version_key));
                    }
                }
            }

            // Handle hard deletions from server
            // These are notes that were permanently deleted (trash emptied) on another device
            for deletion in pull_response.deletions {
                app.debug_log(&format!("Pull - Processing deletion for note {}", deletion.id));

                // Apply remote deletion (hard delete without re-syncing back)
                if note_repo.apply_remote_deletion(&deletion.id)? {
                    app.debug_log(&format!("Pull - Hard deleted note {}", deletion.id));

                    // Remove from in-memory list if present
                    if let Some(pos) = app.notes.iter().position(|n| n.id == deletion.id) {
                        app.notes.remove(pos);
                    }
                    outcome.synced += 1;
                } else {
                    app.debug_log(&format!("Pull - Note {} not found locally (already deleted)", deletion.id));
                }
            }

            // Process inbox items from pull response
            if let Some(inbox_items) = pull_response.inbox_items {
                app.inbox_items = inbox_items;
                app.debug_log(&format!("Pull - Received {} inbox items", app.inbox_items.len()));
            } else {
                app.inbox_items.clear();
            }
            transaction.commit()?;
            break;
        }

        transaction.commit()?;
        offset += page_note_count;
    }

    // Update sync metadata. The pull cursor is the server's time at the first page,
    // so whatever changes during a long pull is fetched next time. last_sync_at
    // stays on this device's clock: it picks which local notes to push, by comparing
    // them with modified_at.
    metadata.last_sync_at = Some(Utc::now());
    metadata.last_pull_at = first_page_at;
    sync_repo.update_metadata(&metadata)?;

    // Reload notes to ensure UI is up to date
    super::notes::load_notes(app)?;

    // The server is reachable, so a re-wrap a password change couldn't deliver can go now
    if metadata.server_key_stale {
        if let Some(password) = app.unlock_password.clone() {
            if let Err(e) = super::envelope::repair_server_key(app, &password) {
                app.debug_log(&format!("[Sync] Could not update the server's wrapped key: {}", e));
            }
        }
    }

    Ok(outcome)
}

/// Open conflict resolution for a note
/// Decrypts and loads both local and server versions for display
pub fn open_conflict_resolution(app: &mut App, note_id: &str) -> Result<()> {
    use crate::ui::state::ViewMode;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let note_repo = NoteRepository::new(db.connection());
    let sync_repo = SyncRepository::new(db.connection());

    // Get the local note
    let local_note = note_repo.get(note_id, key)?
        .ok_or_else(|| anyhow::anyhow!("Note not found"))?;

    // Get conflict data from sync metadata
    let note_sync = sync_repo.get_note_sync(note_id)?
        .ok_or_else(|| anyhow::anyhow!("No sync metadata for note"))?;

    let conflict_data = note_sync.conflict_data
        .ok_or_else(|| anyhow::anyhow!("No conflict data available"))?;

    // Decrypt server content
    let encrypted_content: crate::crypto::EncryptedData = serde_json::from_str(&conflict_data.server_content)?;
    let server_content = app.crypto.decrypt_text(&encrypted_content, key)?;

    // Decrypt server tags
    let server_tags: Vec<String> = conflict_data.server_tags.iter()
        .filter_map(|tag_json| {
            let encrypted_tag: crate::crypto::EncryptedData = serde_json::from_str(tag_json).ok()?;
            let tag_json_str = app.crypto.decrypt_text(&encrypted_tag, key).ok()?;
            serde_json::from_str::<String>(&tag_json_str).ok()
        })
        .collect();

    // Decrypt ancestor content if available (for 3-way merge)
    let ancestor_content = conflict_data.ancestor_content.as_ref().and_then(|encrypted_ancestor| {
        let encrypted: crate::crypto::EncryptedData = serde_json::from_str(encrypted_ancestor).ok()?;
        app.crypto.decrypt_text(&encrypted, key).ok()
    });

    // Decrypt ancestor tags if available
    let ancestor_tags = if let Some(ref encrypted_tags) = conflict_data.ancestor_tags {
        let tags: Vec<String> = encrypted_tags.iter()
            .filter_map(|tag_json| {
                let encrypted_tag: crate::crypto::EncryptedData = serde_json::from_str(tag_json).ok()?;
                let tag_json_str = app.crypto.decrypt_text(&encrypted_tag, key).ok()?;
                serde_json::from_str::<String>(&tag_json_str).ok()
            })
            .collect();
        if tags.is_empty() { None } else { Some(tags) }
    } else {
        None
    };

    // Store data in app for display
    app.conflict_note_id = Some(note_id.to_string());
    app.conflict_data = Some(conflict_data);
    app.conflict_local_content = local_note.content;
    app.conflict_server_content = server_content;
    app.conflict_local_tags = local_note.tags;
    app.conflict_server_tags = server_tags;
    app.conflict_ancestor_content = ancestor_content;
    app.conflict_ancestor_tags = ancestor_tags;
    app.conflict_local_scroll = 0;
    app.conflict_server_scroll = 0;
    app.conflict_focus_server = false;
    app.view_mode = ViewMode::ConflictResolution;

    Ok(())
}

/// Resolve conflict by keeping local version
/// Clears conflict status and marks note for sync
pub fn resolve_keep_local(app: &mut App) -> Result<()> {
    let note_id = app.conflict_note_id.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict being resolved"))?;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let note_repo = NoteRepository::new(db.connection());
    let sync_repo = SyncRepository::new(db.connection());

    // Clear conflict status - the local version will be pushed on next sync
    sync_repo.clear_conflict(&note_id)?;

    // Update the note's modified_at to ensure it gets pushed
    if let Some(mut note) = note_repo.get(&note_id, key)? {
        note.modified_at = chrono::Utc::now();
        note.version += 1;
        note_repo.update(&note, key)?;
    }

    // Clear conflict state in app
    clear_conflict_state(app);

    Ok(())
}

/// Resolve conflict by keeping server version
/// Replaces local content with server version and clears conflict
pub fn resolve_keep_server(app: &mut App) -> Result<()> {
    let note_id = app.conflict_note_id.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict being resolved"))?;

    let conflict_data = app.conflict_data.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict data available"))?;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let note_repo = NoteRepository::new(db.connection());
    let sync_repo = SyncRepository::new(db.connection());
    let version_repo = NoteVersionRepository::new(db.connection());

    // Get the local note to save a version before overwriting
    if let Some(local_note) = note_repo.get(&note_id, key)? {
        // Create version snapshot of local before overwriting
        let _ = version_repo.create_version(&local_note, chrono::Utc::now(), VersionReason::Sync, key);

        // Create updated note with server content (already decrypted in conflict_server_content)
        let mut updated_note = local_note.clone();
        updated_note.content = app.conflict_server_content.clone();
        updated_note.tags = app.conflict_server_tags.clone();
        updated_note.modified_at = conflict_data.server_modified_at;
        updated_note.version = conflict_data.server_version;
        updated_note.pinned = conflict_data.server_pinned;
        updated_note.archived = conflict_data.server_archived;
        updated_note.archived_at = conflict_data.server_archived_at;
        updated_note.locked = conflict_data.server_locked;
        updated_note.locked_at = conflict_data.server_locked_at;
        if let Some(ref lang) = conflict_data.server_syntax_language {
            updated_note.syntax_language = lang.parse().unwrap_or_default();
        }
        if let Some(wrap) = conflict_data.server_word_wrap {
            updated_note.word_wrap = wrap;
        }
        updated_note.color = conflict_data.server_color.clone();

        note_repo.update(&updated_note, key)?;
    }

    // Clear conflict status
    sync_repo.clear_conflict(&note_id)?;

    // Clear conflict state in app and reload notes
    clear_conflict_state(app);
    super::notes::load_notes(app)?;

    Ok(())
}

/// Resolve conflict by keeping both versions
/// Creates a new note from the server version
pub fn resolve_keep_both(app: &mut App) -> Result<String> {
    let note_id = app.conflict_note_id.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict being resolved"))?;

    let conflict_data = app.conflict_data.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict data available"))?;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let note_repo = NoteRepository::new(db.connection());
    let sync_repo = SyncRepository::new(db.connection());

    // Create a new note from the server version
    let mut new_note = Note::new(app.conflict_server_content.clone());
    new_note.tags = app.conflict_server_tags.clone();
    new_note.modified_at = conflict_data.server_modified_at;
    new_note.pinned = conflict_data.server_pinned;
    new_note.archived = conflict_data.server_archived;
    new_note.archived_at = conflict_data.server_archived_at;
    new_note.locked = conflict_data.server_locked;
    new_note.locked_at = conflict_data.server_locked_at;
    if let Some(ref lang) = conflict_data.server_syntax_language {
        new_note.syntax_language = lang.parse().unwrap_or_default();
    }
    if let Some(wrap) = conflict_data.server_word_wrap {
        new_note.word_wrap = wrap;
    }
    new_note.color = conflict_data.server_color.clone();

    let new_note_id = new_note.id.clone();
    note_repo.create(&new_note, key)?;

    // Clear conflict status on the original note
    sync_repo.clear_conflict(&note_id)?;

    // Clear conflict state in app and reload notes
    clear_conflict_state(app);
    super::notes::load_notes(app)?;

    Ok(new_note_id)
}

/// Clear conflict resolution state in app
fn clear_conflict_state(app: &mut App) {
    use crate::ui::state::ViewMode;

    app.conflict_note_id = None;
    app.conflict_data = None;
    app.conflict_local_content.clear();
    app.conflict_server_content.clear();
    app.conflict_local_tags.clear();
    app.conflict_server_tags.clear();
    app.conflict_ancestor_content = None;
    app.conflict_ancestor_tags = None;
    app.conflict_local_scroll = 0;
    app.conflict_server_scroll = 0;
    app.conflict_focus_server = false;
    app.view_mode = ViewMode::NoteList;
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::sync::SyncMetadata;
    use chrono::{DateTime, TimeZone, Utc};
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex, OnceLock};

    /// Generated at runtime, as a key should be, and shared by the helpers below.
    fn key() -> &'static [u8; 32] {
        static KEY: OnceLock<[u8; 32]> = OnceLock::new();
        KEY.get_or_init(|| crate::crypto::CryptoService::new().generate_master_key())
    }

    /// The server's clock: deliberately nowhere near this device's.
    fn server_time(page: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2031, 1, 1, 12, 0, 0).unwrap() + chrono::Duration::minutes(page)
    }

    /// Answers pull requests the way the sync server does, one page at a time,
    /// and records what it was asked for.
    struct StubServer {
        endpoint: String,
        pulls: Arc<Mutex<Vec<Value>>>,
        pushes: Arc<Mutex<Vec<Value>>>,
        refuse: Arc<Mutex<std::collections::HashSet<String>>>,
    }

    fn reply(stream: &mut std::net::TcpStream, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    }

    impl StubServer {
        fn start(notes: Vec<Value>, versions: Vec<Value>) -> Self {
            Self::serve(notes, versions, vec![])
        }

        fn with_attachments(notes: Vec<Value>, attachments: Vec<Value>) -> Self {
            Self::serve(notes, vec![], attachments)
        }

        fn serve(notes: Vec<Value>, versions: Vec<Value>, attachments: Vec<Value>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let pulls = Arc::new(Mutex::new(Vec::new()));
            let recorded = pulls.clone();
            let pushes = Arc::new(Mutex::new(Vec::new()));
            let pushes_seen = pushes.clone();
            let refuse = Arc::new(Mutex::new(std::collections::HashSet::new()));
            let rejecting = refuse.clone();

            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let mut stream = stream.unwrap();
                    let body = read_body(&mut stream);
                    let request: Value = serde_json::from_slice(&body).unwrap();

                    // A push has notes of its own. Conflicts are the ones marked
                    // for the server to refuse.
                    if let Some(pushed) = request["notes"].as_array().cloned() {
                        let (rejected, accepted): (Vec<Value>, Vec<Value>) = pushed.iter()
                            .cloned()
                            .partition(|note| rejecting.lock().unwrap().contains(note["id"].as_str().unwrap_or("")));
                        let response = json!({
                            "accepted": accepted.iter().map(|note| json!({
                                "id": note["id"],
                                "syncedAt": server_time(0),
                                "serverVersion": 1,
                            })).collect::<Vec<_>>(),
                            "rejected": rejected.iter().map(|note| json!({
                                "id": note["id"],
                                "reason": "conflict",
                                "serverVersion": 2,
                                "serverModifiedAt": server_time(0),
                                "serverContent": note["content"],
                                "serverTags": [],
                                "serverAttachments": [],
                                "serverPinned": false,
                                "serverArchived": false,
                                "serverLocked": false,
                            })).collect::<Vec<_>>(),
                            "errors": [],
                        }).to_string();
                        pushes_seen.lock().unwrap().push(request);
                        reply(&mut stream, &response);
                        continue;
                    }

                    let offset = request["offset"].as_i64().unwrap_or(0);
                    let limit = request["limit"].as_i64().unwrap_or(100);
                    recorded.lock().unwrap().push(request);

                    let start = (offset as usize).min(notes.len());
                    let end = (start + limit as usize).min(notes.len());
                    let page = offset / limit;
                    let response = json!({
                        "notes": notes[start..end],
                        "deletions": [],
                        "attachments": attachments,
                        "versions": versions,
                        "syncedAt": server_time(page),
                        "hasMore": end < notes.len(),
                    })
                    .to_string();

                    reply(&mut stream, &response);
                }
            });

            Self { endpoint, pulls, pushes, refuse }
        }

        /// The note ids the server will refuse from now on.
        fn refuse(&self, id: &str) {
            self.refuse.lock().unwrap().insert(id.to_string());
        }

        fn last_push(&self) -> Value {
            self.pushes.lock().unwrap().last().unwrap().clone()
        }

        fn offsets(&self) -> Vec<i64> {
            self.pulls.lock().unwrap().iter().map(|p| p["offset"].as_i64().unwrap()).collect()
        }

        fn last_sync_at_of_last_pull(&self) -> Value {
            self.pulls.lock().unwrap().last().unwrap()["lastSyncAt"].clone()
        }
    }

    fn read_body(stream: &mut std::net::TcpStream) -> Vec<u8> {
        let mut received = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            received.extend_from_slice(&buffer[..read]);
            let Some(header_end) = received.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&received[..header_end]).to_lowercase();
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .map(|value| value.trim().parse().unwrap())
                .unwrap_or(0);
            if received.len() >= header_end + 4 + length {
                return received[header_end + 4..header_end + 4 + length].to_vec();
            }
        }
    }

    /// An unlocked app whose sync settings point at `endpoint`.
    fn app_connected_to(endpoint: &str) -> App {
        let dir = std::env::temp_dir().join(format!("jottery-sync-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::new(dir.join("test.db"), None).unwrap();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();

        let api_key = app.crypto.encrypt_text("device-api-key", key()).unwrap();
        let metadata = SyncMetadata {
            api_key: Some(serde_json::to_string(&api_key).unwrap()),
            sync_enabled: true,
            sync_endpoint: endpoint.to_string(),
            ..Default::default()
        };
        SyncRepository::new(db.connection()).update_metadata(&metadata).unwrap();

        app.db = Some(db);
        app.key = Some(*key());
        app
    }

    fn encrypted(text: &str) -> String {
        let crypto = crate::crypto::CryptoService::new();
        serde_json::to_string(&crypto.encrypt_text(text, key()).unwrap()).unwrap()
    }

    /// The id of the `index`th note from `remote_notes`.
    fn note_id(index: usize) -> String {
        format!("00000000-0000-4000-8000-{index:012}")
    }

    /// The id of the attachment these tests use.
    const ATTACHMENT_ID: &str = "0b9e4d6c-7a21-4f3e-8c55-1d2e3f4a5b6c";

    fn remote_notes(count: usize) -> Vec<Value> {
        (0..count)
            .map(|i| {
                json!({
                    "id": note_id(i),
                    "createdAt": "2020-01-01T00:00:00Z",
                    "modifiedAt": "2020-01-01T00:00:00Z",
                    "content": encrypted(&format!("Note {i}")),
                    "tags": [],
                    "attachments": [],
                    "pinned": false,
                    "archived": false,
                    "locked": false,
                    "deleted": false,
                    "version": 1,
                })
            })
            .collect()
    }

    fn stored_note_count(app: &App) -> usize {
        let db = app.db.as_ref().unwrap();
        NoteRepository::new(db.connection()).list(true, key()).unwrap().len()
    }

    fn sync_metadata(app: &App) -> SyncMetadata {
        let db = app.db.as_ref().unwrap();
        SyncRepository::new(db.connection()).get_metadata().unwrap().unwrap()
    }

    #[test]
    fn pull_fetches_every_page() {
        let server = StubServer::start(remote_notes(250), vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(stored_note_count(&app), 250);
        assert_eq!(server.offsets(), vec![0, 100, 200]);
    }

    #[test]
    fn force_sync_fetches_every_page() {
        let server = StubServer::start(remote_notes(250), vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, true).unwrap();

        assert_eq!(stored_note_count(&app), 250);
    }

    #[test]
    fn an_exact_multiple_of_the_page_size_ends_cleanly() {
        let server = StubServer::start(remote_notes(200), vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(stored_note_count(&app), 200);
        assert_eq!(server.offsets(), vec![0, 100]);
    }

    #[test]
    fn the_next_pull_starts_from_the_servers_time_at_the_first_page() {
        let server = StubServer::start(remote_notes(250), vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();
        assert_eq!(sync_metadata(&app).last_pull_at, Some(server_time(0)));

        perform_sync(&mut app, false).unwrap();
        let cursor = server.last_sync_at_of_last_pull();
        let cursor: DateTime<Utc> = cursor.as_str().unwrap().parse().unwrap();
        assert_eq!(cursor, server_time(0));
    }

    #[test]
    fn choosing_local_notes_to_push_stays_on_this_devices_clock() {
        let server = StubServer::start(remote_notes(1), vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let before = Utc::now();

        perform_sync(&mut app, false).unwrap();

        // The server's clock is years ahead. A local edit made just after this
        // sync must still count as newer than last_sync_at, or it'd never be pushed.
        let last_sync_at = sync_metadata(&app).last_sync_at.unwrap();
        assert!(last_sync_at >= before && last_sync_at <= Utc::now());
    }

    fn add_note_with_attachment(app: &App, filename: &str) -> Note {
        let db = app.db.as_ref().unwrap();
        let mut note = Note::new("Has an attachment".to_string());
        note.attachments.push(Attachment {
            id: ATTACHMENT_ID.to_string(),
            filename: filename.to_string(),
            mime_type: "text/plain".to_string(),
            size: 3,
            data: ATTACHMENT_ID.to_string(),
            thumbnail_data: None,
        });
        NoteRepository::new(db.connection()).create(&note, key()).unwrap();
        AttachmentRepository::new(db.connection())
            .store(ATTACHMENT_ID, filename, "text/plain", 3, b"abc", key())
            .unwrap();
        note
    }

    #[test]
    fn a_pushed_filename_is_encrypted() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        add_note_with_attachment(&app, "secret-plans.pdf");

        perform_sync(&mut app, false).unwrap();

        let request = server.last_push();
        let sent = request["notes"][0]["attachments"][0]["filename"].as_str().unwrap();
        assert!(!sent.contains("secret-plans"));
        let crypto = CryptoService::new();
        assert_eq!(filename_from_sync(&crypto, sent, key()).unwrap(), "secret-plans.pdf");
    }

    #[test]
    fn a_filename_in_a_version_is_encrypted_too() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = add_note_with_attachment(&app, "secret-plans.pdf");
        let db = app.db.as_ref().unwrap();
        NoteVersionRepository::new(db.connection())
            .create_version(&note, Utc::now(), VersionReason::Sync, key())
            .unwrap();

        perform_sync(&mut app, false).unwrap();

        let request = server.last_push();
        let sent = request["versions"][0]["attachments"][0]["filename"].as_str().unwrap();
        assert!(!sent.contains("secret-plans"));
    }

    #[test]
    fn a_filename_already_encrypted_is_not_encrypted_twice() {
        let crypto = CryptoService::new();
        let encrypted = filename_for_sync(&crypto, "photo.png", key()).unwrap();
        assert_eq!(filename_for_sync(&crypto, &encrypted, key()).unwrap(), encrypted);
    }

    #[test]
    fn a_plain_filename_from_an_older_client_is_kept() {
        let crypto = CryptoService::new();
        assert_eq!(filename_from_sync(&crypto, "日本語のファイル.png", key()).unwrap(), "日本語のファイル.png");
    }

    #[test]
    fn ciphertext_that_will_not_decrypt_is_not_a_filename() {
        let crypto = CryptoService::new();
        let other_key = crypto.generate_master_key();
        let encrypted = filename_for_sync(&crypto, "photo.png", &other_key).unwrap();
        assert!(filename_from_sync(&crypto, &encrypted, key()).is_err());
    }

    #[test]
    fn a_pulled_attachment_with_a_plain_filename_arrives() {
        use base64::Engine as _;
        let crypto = CryptoService::new();
        let blob = serde_json::to_vec(&crypto.encrypt_binary(b"abc", key()).unwrap()).unwrap();
        let mut notes = remote_notes(1);
        notes[0]["attachments"] = json!([{
            "id": ATTACHMENT_ID,
            "filename": "an older client's file.txt",
            "mimeType": "text/plain",
            "size": 3,
            "data": ATTACHMENT_ID,
        }]);
        let server = StubServer::with_attachments(notes, vec![json!({
            "id": ATTACHMENT_ID,
            "data": base64::engine::general_purpose::STANDARD.encode(blob),
        })]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).list(true, key()).unwrap();
        assert_eq!(stored[0].attachments[0].filename, "an older client's file.txt");
    }

    /// A note this device can't open, as a device on another key would have pushed.
    fn unreadable_note(id: &str) -> Value {
        let crypto = CryptoService::new();
        let other_key = crypto.generate_master_key();
        let content = serde_json::to_string(&crypto.encrypt_text("Elsewhere", &other_key).unwrap()).unwrap();
        let mut note = remote_notes(1).remove(0);
        note["id"] = json!(id);
        note["content"] = json!(content);
        note
    }

    #[test]
    fn one_unreadable_note_does_not_stop_the_pull() {
        let mut notes = remote_notes(3);
        notes.insert(1, unreadable_note(&note_id(99)));
        let server = StubServer::start(notes, vec![]);
        let mut app = app_connected_to(&server.endpoint);

        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.synced, 3);
        assert_eq!(outcome.skipped, 1);
        assert_eq!(stored_note_count(&app), 3);
    }

    /// A note that fails serde used to fail the whole response, so nothing on the page
    /// arrived, and the same page failed the same way on every sync after it.
    #[test]
    fn a_note_that_will_not_parse_does_not_stop_the_pull() {
        let mut notes = remote_notes(3);
        notes[1]["modifiedAt"] = json!("not a date");
        let server = StubServer::start(notes, vec![]);
        let mut app = app_connected_to(&server.endpoint);

        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.skipped, 1);
        assert_eq!(stored_note_count(&app), 2);
        assert_eq!(sync_metadata(&app).last_pull_at, Some(server_time(0)));
    }

    #[test]
    fn a_note_whose_id_is_not_a_uuid_never_reaches_the_database() {
        let mut notes = remote_notes(2);
        notes[0]["id"] = json!("../../not-a-note");
        let server = StubServer::start(notes, vec![]);
        let mut app = app_connected_to(&server.endpoint);

        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.skipped, 1);
        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).list(true, key()).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, note_id(1));
    }

    #[test]
    fn the_cursor_moves_past_a_note_that_cannot_be_read() {
        let server = StubServer::start(vec![unreadable_note(&note_id(99))], vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(sync_metadata(&app).last_pull_at, Some(server_time(0)));
    }

    #[test]
    fn notes_that_cannot_be_read_are_reported() {
        let server = StubServer::start(vec![unreadable_note(&note_id(99))], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(server.endpoint.clone());

        trigger_sync(&mut app);

        assert_eq!(app.error, Some(t!("sync.skipped_notes", count = 1).to_string()));
    }

    #[test]
    fn a_refused_note_is_counted() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = add_note_with_attachment(&app, "plain.txt");
        server.refuse(&note.id);

        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.rejected, 1);
    }

    #[test]
    fn an_emptied_bin_reaches_the_server_with_nothing_else_to_push() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let note = Note::new("Gone".to_string());
        note_repo.create(&note, key()).unwrap();
        note_repo.hard_delete(&note.id).unwrap();

        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.last_push()["deletions"][0]["id"], note.id);
    }

    /// The CLI runs this same sync. It used to push only, then mark everything as
    /// seen, so remote changes from before that moment were never pulled.
    #[test]
    fn a_headless_sync_pulls_as_well_as_pushes() {
        let server = StubServer::start(remote_notes(2), vec![]);
        let connected = app_connected_to(&server.endpoint);
        let path = connected.db_path.clone();
        let db = connected.db.unwrap();
        NoteRepository::new(db.connection()).create(&Note::new("Written here".to_string()), key()).unwrap();

        let mut app = App::headless(path, db, *key()).unwrap();
        super::super::notes::load_notes(&mut app).unwrap();
        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.synced, 3);
        assert_eq!(stored_note_count(&app), 3);
        assert_eq!(sync_metadata(&app).last_pull_at, Some(server_time(0)));
    }

    #[test]
    fn a_version_of_a_note_on_a_later_page_is_applied() {
        let version = json!({
            "versionKey": format!("{}:1", note_id(150)),
            "noteId": note_id(150),
            "version": 1,
            "createdAt": "2020-01-01T00:00:00Z",
            "syncedAt": "2020-01-01T00:00:00Z",
            "content": encrypted("Note 150, earlier"),
            "tags": [],
            "attachments": [],
            "reason": "sync",
        });
        let server = StubServer::start(remote_notes(250), vec![version]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let versions = NoteVersionRepository::new(db.connection());
        assert!(versions.get_version_by_key(&format!("{}:1", note_id(150))).unwrap().is_some());
    }
}
