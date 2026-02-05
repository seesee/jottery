//! Inbox operations — accept (encrypt into main corpus) or delete inbox items

use anyhow::Result;
use chrono::Utc;
use rust_i18n::t;
use uuid::Uuid;

use crate::models::{Note, SyntaxLanguage};
use crate::ui::app::App;
use crate::repository::NoteRepository;
use crate::repository::sync::SyncRepository;

/// Get sync credentials (endpoint + decrypted API key) from the app
fn get_sync_credentials(app: &App) -> Result<Option<(String, String)>> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let sync_repo = SyncRepository::new(db.connection());
    if let Some(metadata) = sync_repo.get_metadata()? {
        if let Some(ref encrypted_api_key) = metadata.api_key {
            let api_key_encrypted: crate::crypto::EncryptedData = serde_json::from_str(encrypted_api_key)?;
            let api_key = app.crypto.decrypt_text(&api_key_encrypted, key)?;
            return Ok(Some((metadata.sync_endpoint.clone(), api_key)));
        }
    }
    Ok(None)
}

/// Accept an inbox item — create an encrypted note and delete from server
pub fn accept_item(app: &mut App, index: usize) -> Result<()> {
    let item = app.inbox_items.get(index)
        .ok_or_else(|| anyhow::anyhow!("Inbox item not found"))?
        .clone();

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let note_repo = NoteRepository::new(db.connection());

    // Create a Note struct from the inbox item
    let now = Utc::now();
    let note = Note {
        id: Uuid::new_v4().to_string(),
        created_at: now,
        modified_at: now,
        synced_at: None,
        content: item.content.clone(),
        tags: item.tags.clone(),
        attachments: vec![],
        pinned: false,
        archived: false,
        archived_at: None,
        locked: false,
        locked_at: None,
        deleted: false,
        deleted_at: None,
        sync_hash: None,
        version: 1,
        word_wrap: true,
        syntax_language: SyntaxLanguage::default(),
        color: None,
        content_hash: None,
        parent_hash: None,
        hash_chain: None,
    };

    note_repo.create(&note, key)?;
    app.debug_log(&format!("Inbox - Accepted item {} as note {}", item.id, note.id));

    // Delete from server
    if let Some((endpoint, api_key)) = get_sync_credentials(app)? {
        if let Err(e) = crate::api::delete_inbox_item(&endpoint, &api_key, &item.id) {
            app.debug_log(&format!("Inbox - Failed to delete item from server: {}", e));
        }
    }

    // Remove from in-memory list
    app.inbox_items.retain(|i| i.id != item.id);
    if app.selected_inbox_item >= app.inbox_items.len() && !app.inbox_items.is_empty() {
        app.selected_inbox_item = app.inbox_items.len() - 1;
    }

    // Reload notes to show the new note
    super::notes::load_notes(app)?;

    app.sync_status = Some(t!("inbox.accepted").to_string());
    Ok(())
}

/// Delete an inbox item from the server
pub fn delete_item(app: &mut App, index: usize) -> Result<()> {
    let item = app.inbox_items.get(index)
        .ok_or_else(|| anyhow::anyhow!("Inbox item not found"))?
        .clone();

    // Delete from server
    if let Some((endpoint, api_key)) = get_sync_credentials(app)? {
        crate::api::delete_inbox_item(&endpoint, &api_key, &item.id)?;
    }

    // Remove from in-memory list
    app.inbox_items.retain(|i| i.id != item.id);
    if app.selected_inbox_item >= app.inbox_items.len() && !app.inbox_items.is_empty() {
        app.selected_inbox_item = app.inbox_items.len() - 1;
    }

    app.sync_status = Some(t!("inbox.deleted").to_string());
    Ok(())
}

/// Accept all inbox items
pub fn accept_all(app: &mut App) -> Result<()> {
    let count = app.inbox_items.len();
    while !app.inbox_items.is_empty() {
        accept_item(app, 0)?;
    }

    app.sync_status = Some(t!("inbox.accepted_all", count = count).to_string());
    Ok(())
}

/// Delete all inbox items
pub fn delete_all(app: &mut App) -> Result<()> {
    // Bulk delete from server
    if let Some((endpoint, api_key)) = get_sync_credentials(app)? {
        crate::api::delete_all_inbox_items(&endpoint, &api_key)?;
    }

    app.inbox_items.clear();
    app.selected_inbox_item = 0;
    app.sync_status = Some(t!("inbox.deleted_all").to_string());
    Ok(())
}
