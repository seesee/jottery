//! Sync operations for bidirectional sync with server

use anyhow::{Context, Result};
use rust_i18n::t;
use std::time::Instant;

use crate::{
    models::{Attachment, Note, sync::{SyncPushRequest, SyncPullRequest, SyncNote, SyncPushResponse, SyncPullResponse, AttachmentRef, SyncAttachment, SyncNoteVersion, SyncDeletion}},
    repository::{attachment::AttachmentRepository, NoteRepository, NoteVersionRepository, VersionReason, sync::SyncRepository},
};

use super::super::app::App;

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
        Ok(result) => {
            // Reload notes from database to pick up sync changes
            if let Err(e) = super::notes::load_notes(app) {
                app.error = Some(format!("Sync succeeded but failed to reload notes: {}", e));
            }
            // Refresh conflict cache after sync
            app.refresh_conflict_cache();
            let conflict_count = app.conflict_note_ids.len();

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
        }
        Err(e) => {
            app.error = Some(format!("Sync failed: {}", e));
            app.sync_status = Some(format!("Sync failed: {}", e));
            app.sync_status_set_at = Some(Instant::now());
        }
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
        Ok(result) => {
            // Reload notes from database to pick up sync changes
            if let Err(e) = super::notes::load_notes(app) {
                app.error = Some(format!("Sync succeeded but failed to reload notes: {}", e));
            }
            // Refresh conflict cache after sync
            app.refresh_conflict_cache();
            let conflict_count = app.conflict_note_ids.len();

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
        }
        Err(e) => {
            app.error = Some(format!("Force sync failed: {}", e));
            app.sync_status = Some(format!("Force sync failed: {}", e));
            app.sync_status_set_at = Some(Instant::now());
        }
    }
}

/// Perform bidirectional sync with server
/// If force is true, pulls all notes from server regardless of last sync time
pub fn perform_sync(app: &mut App, force: bool) -> Result<usize> {
    use chrono::Utc;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let sync_repo = SyncRepository::new(db.connection());
    let note_repo = NoteRepository::new(db.connection());
    let version_repo = NoteVersionRepository::new(db.connection());

    // Get sync metadata
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();

    // Get API key
    let encrypted_api_key = metadata.api_key.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No API key configured"))?;
    let api_key_encrypted: crate::crypto::EncryptedData = serde_json::from_str(encrypted_api_key)?;
    let api_key = app.crypto.decrypt_text(&api_key_encrypted, key)?;

    let endpoint = metadata.sync_endpoint.clone();

    // PUSH: Send local changes to server
    let last_sync = metadata.last_sync_at;
    let notes_to_push = if let Some(last_sync) = last_sync {
        note_repo.get_modified_after(last_sync, key)?
    } else {
        // Include deleted notes in full sync so soft-deleted notes sync to other devices
        note_repo.list(true, key)?
    };

    let mut sync_count = 0;

    if !notes_to_push.is_empty() {
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
            let attachment_refs: Vec<AttachmentRef> = note.attachments.iter().map(|att| {
                attachment_ids_to_push.insert(att.id.clone());
                AttachmentRef {
                    id: att.id.clone(),
                    filename: att.filename.clone(), // Already encrypted in database
                    mime_type: att.mime_type.clone(),
                    size: att.size,
                    data: att.data.clone(),
                }
            }).collect();

            Ok(SyncNote {
                id: note.id.clone(),
                created_at: note.created_at,
                modified_at: note.modified_at,
                content: content_json,
                tags: encrypted_tags?,
                attachments: attachment_refs,
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

                // Build attachment references
                let attachment_refs: Vec<AttachmentRef> = version.attachments.iter().map(|att| {
                    AttachmentRef {
                        id: att.id.clone(),
                        filename: att.filename.clone(), // Already encrypted
                        mime_type: att.mime_type.clone(),
                        size: att.size,
                        data: att.data.clone(),
                    }
                }).collect();

                sync_versions.push(SyncNoteVersion {
                    version_key: format!("{}:{}", version.note_id, version.version),
                    note_id: version.note_id.clone(),
                    version: version.version,
                    created_at: version.created_at,
                    synced_at: version.synced_at,
                    content: content_json,
                    tags: encrypted_tags?,
                    attachments: attachment_refs,
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

        // Fetch binary data for all attachments that need to be pushed
        let attachment_repo = AttachmentRepository::new(db.connection());
        let sync_attachments: Result<Vec<SyncAttachment>> = attachment_ids_to_push.iter().map(|att_id| {
            // Get encrypted binary data from database
            let (_filename, _mime_type, _size, encrypted_data) = attachment_repo
                .get(att_id, key)?
                .context(format!("Attachment {} not found", att_id))?;

            // Re-encrypt and base64 encode for transmission
            let encrypted_blob = app.crypto.encrypt_binary(&encrypted_data, key)?;
            let base64_data = general_purpose::STANDARD.encode(serde_json::to_vec(&encrypted_blob)?);

            Ok(SyncAttachment {
                id: att_id.clone(),
                data: base64_data,
            })
        }).collect();

        // Collect pending deletions (from emptying trash)
        let pending_deletions = note_repo.get_pending_deletions()?;
        let sync_deletions: Vec<SyncDeletion> = pending_deletions
            .iter()
            .map(|(id, deleted_at)| SyncDeletion {
                id: id.clone(),
                deleted_at: deleted_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
            })
            .collect();

        let push_request = SyncPushRequest {
            notes: sync_notes,
            attachments: sync_attachments?,
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

        let push_response: SyncPushResponse = response.json()
            .context("Failed to parse push response")?;

        sync_count += push_response.accepted.len();

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

        (last_sync, known_ids, known_att_ids)
    };

    let pull_request = SyncPullRequest {
        last_sync_at: last_sync_for_pull,
        known_note_ids,
        known_attachment_ids,
    };

    let pull_url = format!("{}/api/v1/sync/pull", endpoint);
    let client = reqwest::blocking::Client::new();

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
    let pull_response: SyncPullResponse = serde_json::from_str(&response_text)
        .context("Failed to parse pull response")?;

    use base64::{Engine as _, engine::general_purpose};
    let attachment_repo = AttachmentRepository::new(db.connection());

    // Decrypt attachments and build a map for quick lookup
    let mut attachment_data_map: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
    for sync_attachment in &pull_response.attachments {
        if let Ok(decoded_data) = general_purpose::STANDARD.decode(&sync_attachment.data) {
            if let Ok(encrypted_blob) = serde_json::from_slice::<crate::crypto::EncryptedData>(&decoded_data) {
                if let Ok(decrypted_data) = app.crypto.decrypt_binary(&encrypted_blob, key) {
                    attachment_data_map.insert(sync_attachment.id.clone(), decrypted_data);
                }
            }
        }
    }

    for remote_note in pull_response.notes {
        // Decrypt content and tags from server (they're stored encrypted on server)
        let encrypted_content: crate::crypto::EncryptedData = serde_json::from_str(&remote_note.content)?;
        let decrypted_content = app.crypto.decrypt_text(&encrypted_content, key)?;

        let decrypted_tags: Vec<String> = remote_note.tags.iter()
            .flat_map(|tag_json| {
                // Parse and decrypt the tag
                let encrypted_tag: crate::crypto::EncryptedData = serde_json::from_str(tag_json).ok()?;
                let tag_json_str = app.crypto.decrypt_text(&encrypted_tag, key).ok()?;

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
            .collect();

        // Process attachments for this note
        let mut note_attachments: Vec<Attachment> = Vec::new();
        app.debug_log(&format!("Pull - Processing {} attachments for note {}", remote_note.attachments.len(), remote_note.id));

        for attachment_ref in &remote_note.attachments {
            app.debug_log(&format!("Pull - Processing attachment: {} ({})", attachment_ref.id, attachment_ref.mime_type));

            // Get decrypted binary data from our map
            if let Some(decrypted_data) = attachment_data_map.get(&attachment_ref.id) {
                app.debug_log(&format!("Pull - Found attachment data in map, size: {} bytes", decrypted_data.len()));

                // Decrypt the filename
                let encrypted_filename: crate::crypto::EncryptedData = match serde_json::from_str(&attachment_ref.filename) {
                    Ok(data) => data,
                    Err(e) => {
                        app.debug_log(&format!("Pull - Failed to parse filename as JSON for {}: {}, raw: {:?}", attachment_ref.id, e, &attachment_ref.filename[..100.min(attachment_ref.filename.len())]));
                        continue;
                    }
                };

                let decrypted_filename = match app.crypto.decrypt_text(&encrypted_filename, key) {
                    Ok(filename) => filename,
                    Err(e) => {
                        app.debug_log(&format!("Pull - Failed to decrypt filename for {}: {}", attachment_ref.id, e));
                        continue;
                    }
                };

                // Parse the filename - try JSON first (new format), fall back to plain string (legacy format)
                let filename: String = serde_json::from_str(&decrypted_filename)
                    .unwrap_or(decrypted_filename);

                app.debug_log(&format!("Pull - Decrypted filename: {}", filename));

                // Store in database
                attachment_repo.store(
                    &attachment_ref.id,
                    &filename,
                    &attachment_ref.mime_type,
                    attachment_ref.size,
                    decrypted_data,
                    key
                )?;

                app.debug_log("Pull - Stored attachment in database");

                // Add to note's attachment array
                note_attachments.push(Attachment {
                    id: attachment_ref.id.clone(),
                    filename: filename.clone(),
                    mime_type: attachment_ref.mime_type.clone(),
                    size: attachment_ref.size,
                    data: attachment_ref.data.clone(),
                    thumbnail_data: None,
                });

                app.debug_log("Pull - Added attachment to note_attachments array");
            } else {
                app.debug_log(&format!("Pull - Attachment data NOT found in map for {} - will try to preserve from local", attachment_ref.id));
                // Server didn't send data because we said we had it - mark for local preservation
            }
        }

        app.debug_log(&format!("Pull - Total attachments from server: {}", note_attachments.len()));

        // Check if we have this note in the database (not just in-memory list)
        let existing_note = note_repo.get(&remote_note.id, key)?;

        // If we have fewer attachments than the remote says we should have,
        // preserve any local attachments that weren't sent by the server
        if let Some(ref local_note) = existing_note {
            for attachment_ref in &remote_note.attachments {
                // If this attachment wasn't in the server data (we already had it)
                if !note_attachments.iter().any(|a| a.id == attachment_ref.id) {
                    // Try to find it in local attachments
                    if let Some(local_att) = local_note.attachments.iter().find(|a| a.id == attachment_ref.id) {
                        app.debug_log(&format!("Pull - Preserving local attachment: {} ({})", local_att.id, local_att.filename));
                        note_attachments.push(local_att.clone());
                    } else {
                        app.debug_log(&format!("Pull - WARNING: Attachment {} not found locally either!", attachment_ref.id));
                    }
                }
            }
            app.debug_log(&format!("Pull - Total attachments after preserving local: {}", note_attachments.len()));
        }

        if let Some(mut local_note) = existing_note {
            // Note exists in database - check if we should update it
            app.debug_log(&format!("Pull - Existing note found: {}", remote_note.id));
            app.debug_log(&format!("  Remote modified_at: {}", remote_note.modified_at));
            app.debug_log(&format!("  Local modified_at: {}", local_note.modified_at));
            app.debug_log(&format!("  Remote > Local? {}", remote_note.modified_at > local_note.modified_at));
            app.debug_log(&format!("  Local attachments: {}, Remote attachments: {}", local_note.attachments.len(), note_attachments.len()));

            // Conflict resolution: Last-Write-Wins, but also update if attachments differ
            // During force sync, always update to ensure we pull latest from server
            let should_update = if force {
                app.debug_log("  -> Force sync mode: will update regardless of timestamps");
                true
            } else {
                remote_note.modified_at > local_note.modified_at
                    || note_attachments.len() != local_note.attachments.len()
            };

            if should_update {
                if force {
                    app.debug_log("  -> Updating note (force sync)");
                } else if remote_note.modified_at > local_note.modified_at {
                    app.debug_log("  -> Updating note (remote is newer)");
                } else {
                    app.debug_log("  -> Updating note (attachments differ even though timestamps match)");
                }
                // Capture local version BEFORE overwriting with remote
                let _ = version_repo.create_version(&local_note, pull_response.synced_at, VersionReason::Sync, key);

                // Remote is newer, update local with decrypted content
                local_note.content = decrypted_content;
                local_note.tags = decrypted_tags;
                local_note.attachments = note_attachments.clone();
                local_note.modified_at = remote_note.modified_at;
                local_note.synced_at = Some(pull_response.synced_at); // Mark as synced
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

                note_repo.update(&local_note, key)?;

                // Also update in-memory list if present
                if let Some(mem_note) = app.notes.iter_mut().find(|n| n.id == remote_note.id) {
                    *mem_note = local_note;
                }

                sync_count += 1;
            } else {
                app.debug_log("  -> NOT updating note (local is same or newer) - ATTACHMENTS WILL BE LOST!");
            }
        } else {
            // New note from server, add it with decrypted content
            let mut new_note = Note::new(decrypted_content);
            new_note.id = remote_note.id.clone();
            new_note.created_at = remote_note.created_at;
            new_note.modified_at = remote_note.modified_at;
            new_note.synced_at = Some(pull_response.synced_at); // Mark as synced
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

            note_repo.create(&new_note, key)?;

            // Add to in-memory list only if not deleted
            if !new_note.deleted {
                app.notes.insert(0, new_note);
            }

            sync_count += 1;
        }
    }

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
            let version_attachments: Vec<Attachment> = server_version.attachments.iter().map(|att_ref| {
                Attachment {
                    id: att_ref.id.clone(),
                    filename: att_ref.filename.clone(),
                    mime_type: att_ref.mime_type.clone(),
                    size: att_ref.size,
                    data: att_ref.data.clone(),
                    thumbnail_data: None,
                }
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
            sync_count += 1;
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

    // Update sync metadata
    metadata.last_sync_at = Some(Utc::now());
    metadata.last_pull_at = Some(Utc::now());
    sync_repo.update_metadata(&metadata)?;

    // Reload notes to ensure UI is up to date
    super::notes::load_notes(app)?;

    Ok(sync_count)
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

