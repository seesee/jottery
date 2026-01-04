//! Sync operations for bidirectional sync with server

use anyhow::{Context, Result};
use rust_i18n::t;
use std::time::Instant;

use crate::{
    models::{Attachment, Note, sync::{SyncPushRequest, SyncPullRequest, SyncNote, SyncPushResponse, SyncPullResponse, AttachmentRef, SyncAttachment, SyncNoteVersion}},
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
            let unit = if result == 1 { t!("sync.note").to_string() } else { t!("sync.notes").to_string() };
            app.sync_status = Some(t!("sync.complete", count = result, unit = unit).to_string());
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
            let unit = if result == 1 { t!("sync.note").to_string() } else { t!("sync.notes").to_string() };
            app.sync_status = Some(t!("sync.force_complete", count = result, unit = unit).to_string());
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
        note_repo.list(false, key)?
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
                deleted: note.deleted,
                deleted_at: note.deleted_at,
                version: note.version,
                word_wrap: Some(note.word_wrap),
                syntax_language: Some(note.syntax_language.to_string()),
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
                    reason: version.reason.to_string(),
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

        let push_request = SyncPushRequest {
            notes: sync_notes,
            attachments: sync_attachments?,
            versions: sync_versions,
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

                app.debug_log(&"Pull - Stored attachment in database".to_string());

                // Add to note's attachment array
                note_attachments.push(Attachment {
                    id: attachment_ref.id.clone(),
                    filename: filename.clone(),
                    mime_type: attachment_ref.mime_type.clone(),
                    size: attachment_ref.size,
                    data: attachment_ref.data.clone(),
                    thumbnail_data: None,
                });

                app.debug_log(&"Pull - Added attachment to note_attachments array".to_string());
            } else {
                app.debug_log(&format!("Pull - Attachment data NOT found in map for {}", attachment_ref.id));
            }
        }

        app.debug_log(&format!("Pull - Total attachments added to note: {}", note_attachments.len()));

        // Check if we have this note in the database (not just in-memory list)
        let existing_note = note_repo.get(&remote_note.id, key)?;

        if let Some(mut local_note) = existing_note {
            // Note exists in database - check if we should update it
            app.debug_log(&format!("Pull - Existing note found: {}", remote_note.id));
            app.debug_log(&format!("  Remote modified_at: {}", remote_note.modified_at));
            app.debug_log(&format!("  Local modified_at: {}", local_note.modified_at));
            app.debug_log(&format!("  Remote > Local? {}", remote_note.modified_at > local_note.modified_at));
            app.debug_log(&format!("  Local attachments: {}, Remote attachments: {}", local_note.attachments.len(), note_attachments.len()));

            // Conflict resolution: Last-Write-Wins, but also update if attachments differ
            let should_update = remote_note.modified_at > local_note.modified_at
                || note_attachments.len() != local_note.attachments.len();

            if should_update {
                if remote_note.modified_at > local_note.modified_at {
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
                local_note.pinned = remote_note.pinned;
                local_note.deleted = remote_note.deleted;
                local_note.deleted_at = remote_note.deleted_at;
                local_note.version = remote_note.version;
                local_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
                if let Some(lang_str) = &remote_note.syntax_language {
                    local_note.syntax_language = lang_str.parse().unwrap_or_default();
                }

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
            new_note.tags = decrypted_tags;
            new_note.attachments = note_attachments;
            new_note.pinned = remote_note.pinned;
            new_note.deleted = remote_note.deleted;
            new_note.deleted_at = remote_note.deleted_at;
            new_note.version = remote_note.version;
            new_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
            if let Some(lang_str) = &remote_note.syntax_language {
                new_note.syntax_language = lang_str.parse().unwrap_or_default();
            }

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
                reason,
            };

            // Store the version
            if let Err(e) = version_repo.insert_version_from_sync(&local_version, key) {
                app.debug_log(&format!("Pull - Failed to store version {}: {}", server_version.version_key, e));
            } else {
                app.debug_log(&format!("Pull - Stored version from server: {}", server_version.version_key));
            }
        }
    }

    // Handle deletions
    for deletion in pull_response.deletions {
        if let Some(pos) = app.notes.iter().position(|n| n.id == deletion.id) {
            note_repo.delete(&deletion.id)?;
            app.notes.remove(pos);
            sync_count += 1;
        }
    }

    // Update sync metadata
    metadata.last_sync_at = Some(Utc::now());
    metadata.last_pull_at = Some(Utc::now());
    sync_repo.update_metadata(&metadata)?;

    // Reload notes to ensure UI is up to date
    super::notes::load_notes(app)?;

    Ok(sync_count)
}

/// Check if auto-sync should run and trigger it if needed
/// Also handles auto-clearing sync status after timeout
/// Call this periodically (e.g., on Tick events) to enable background sync
pub fn check_auto_sync(app: &mut App) {
    // Auto-clear sync status after 5 seconds
    if let Some(set_at) = app.sync_status_set_at {
        let now = Instant::now();
        let elapsed = now.duration_since(set_at);
        if elapsed >= std::time::Duration::from_secs(5) {
            app.sync_status = None;
            app.sync_status_set_at = None;
        }
    }

    // Check if auto-sync is enabled
    if app.settings.auto_sync_interval_minutes <= 0 {
        return; // Auto-sync disabled
    }

    // Check if sync is configured
    if !app.settings.sync_enabled || app.settings.sync_endpoint.is_none() {
        return; // Sync not configured
    }

    // Check if we're unlocked (have database and key)
    if app.db.is_none() || app.key.is_none() {
        return; // Not unlocked, can't sync
    }

    // Check time since last auto-sync
    let now = Instant::now();
    let should_sync = match app.last_auto_sync {
        None => true, // Never synced, do it now
        Some(last) => {
            let elapsed = now.duration_since(last);
            let interval = std::time::Duration::from_secs(
                (app.settings.auto_sync_interval_minutes as u64) * 60
            );
            elapsed >= interval
        }
    };

    if should_sync {
        app.debug_log("Auto-sync: triggering scheduled sync");
        // Trigger sync (this will update sync_status)
        trigger_sync(app);
        // Update last auto-sync time
        app.last_auto_sync = Some(now);
    }
}
