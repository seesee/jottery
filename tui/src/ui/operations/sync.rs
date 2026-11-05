//! Sync operations for bidirectional sync with server
//!
//! `docs/sync-protocol.md` is the reference. The server stores ciphertext and isn't
//! trusted: `SyncPullResponse::parse` checks what it sends, and everything here acts on
//! what's left.

use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use rusqlite::Connection;
use rust_i18n::t;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

use crate::{
    crypto::{update_hash_chain, CryptoService, EncryptedData},
    db::Database,
    models::{
        char_prefix, Attachment, Note,
        sync::{
            AttachmentRef, AttachmentWarning, ConflictData, InboxItem, SyncAttachment, SyncDeletion, SyncMetadata,
            SyncNote, SyncNoteVersion, SyncPullRequest, SyncPullResponse, SyncPushRequest,
            SyncPushResponse, SyncSavedSearch,
        },
    },
    repository::{
        attachment::AttachmentRepository, sync::SyncRepository, NoteRepository, NoteVersionRepository,
        SavedSearchRepository, VersionReason,
    },
};

use super::super::app::App;

/// Changes to ask for per pull request.
const PULL_PAGE_SIZE: i64 = 100;

/// Notes per push. The server holds its write lock for the whole of one.
const PUSH_NOTES_PER_BATCH: usize = 100;

/// Roughly how much blob data goes in one push. A single larger blob still goes alone.
const PUSH_BYTES_PER_BATCH: usize = 8 * 1024 * 1024;

/// How long the server may take to accept a connection, and to answer a request. Long
/// enough for a push of blobs on a slow line, and short enough to give up on a dead one.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// More pages than any real vault needs, so a server that always says there's more
/// can't keep a sync going for ever.
const MAX_PULL_PAGES: usize = 100_000;

/// The longest a server can ask this device to wait.
const MAX_RETRY_AFTER_SECONDS: u64 = 3600;

/// A sync that would remove this many notes asks first.
const MASS_DELETION_THRESHOLD: usize = 10;

/// How far back notes that reached the bin from elsewhere count towards that.
const MASS_DELETION_WINDOW_HOURS: i64 = 24;

/// How long a note deleted elsewhere stays restorable in the bin here.
const REMOTE_DELETION_RETENTION_DAYS: i64 = 30;

/// How far ahead of its own clock the server accepts a date.
const SERVER_CLOCK_TOLERANCE_HOURS: i64 = 24;

/// How often the list on screen catches up with a sync that's still running.
const SYNC_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// How many missing files one sync goes looking for, so that files the server has lost
/// don't cost a request each on every sync.
const MAX_BLOB_FETCHES_PER_SYNC: usize = 50;

/// What a sync achieved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SyncOutcome {
    /// Notes pushed, or changed by the pull
    pub synced: usize,
    /// Notes in conflict with the server, whether refused by a push or found by a pull
    pub rejected: usize,
    /// Notes or attachments from the server that couldn't be read, and were skipped
    pub skipped: usize,
    /// The account is at its storage quota, so local changes are still waiting to go
    pub quota_exceeded: bool,
    /// Notes the server says to delete, waiting for the user to confirm
    pub deletions_held: usize,
    /// Changes dated too far ahead of the server's clock for it to take. They stay here,
    /// and go once this device's clock is right.
    pub clock_ahead: bool,
    /// The latest date on any note the server said nothing about. A server skips what's
    /// dated too far ahead without saying so, and only a pull says what time it is there.
    unanswered_until: Option<DateTime<Utc>>,
}

/// The server turned this device away, and retrying at once won't help.
#[derive(Debug, thiserror::Error)]
pub enum SyncRefused {
    /// The account is deactivated or not approved. The credentials stay, because
    /// reactivating the account brings the key back.
    #[error("{0}")]
    AccountDisabled(String),
    /// Too many requests, or a busy server: wait this long
    #[error("{message}")]
    Wait { seconds: u64, message: String },
    /// Nothing was written. Pushes that only delete still work.
    #[error("{0}")]
    QuotaExceeded(String),
}

/// A background sync was told to stop, because the vault locked or the account changed.
#[derive(Debug, thiserror::Error)]
#[error("Sync was cancelled")]
pub struct SyncCancelled;

/// How a background sync and the UI thread stay out of each other's way.
///
/// Each has its own database connection, and only the holder of the gate may change the
/// vault. The UI thread holds it for the whole of a key press, so what a handler reads
/// from memory is still true when it writes. A sync holds it to apply a page, and never
/// across a request to the server, so the wait is short.
#[derive(Clone, Default)]
pub struct VaultGate {
    gate: Arc<Mutex<()>>,
    /// A sync has changed the vault since the UI last looked
    changed: Arc<AtomicBool>,
    /// The note the user has open. Its text is in an input buffer, so a pull that
    /// replaced the note would be overwritten by the next save.
    open_note: Arc<Mutex<Option<String>>>,
}

impl VaultGate {
    /// Wait for the gate. A panic while it was held can't have left anything to protect
    /// but the database, whose transactions look after themselves.
    pub fn hold(&self) -> MutexGuard<'_, ()> {
        self.gate.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Take the gate if it's free.
    pub fn try_hold(&self) -> Option<MutexGuard<'_, ()>> {
        match self.gate.try_lock() {
            Ok(held) => Some(held),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }

    /// Say which note is open. Only call this holding the gate.
    pub fn set_open_note(&self, note_id: Option<&str>) {
        *self.open_note.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = note_id.map(String::from);
    }

    fn note_is_open(&self, note_id: &str) -> bool {
        self.open_note.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_deref() == Some(note_id)
    }

    fn mark_changed(&self) {
        self.changed.store(true, Ordering::SeqCst);
    }

    /// Whether a sync has changed the vault since this was last asked.
    pub fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::SeqCst)
    }
}

/// One sync's link to the UI thread.
#[derive(Clone, Default)]
pub(crate) struct SyncLink {
    gate: VaultGate,
    cancelled: Arc<AtomicBool>,
}

impl SyncLink {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Change the vault, unless this sync has been cancelled.
    ///
    /// Cancelling happens with the gate held, so after it this runs nothing. That's what
    /// lets the UI thread lock the vault, or change the account, without waiting for a
    /// sync that may be stuck on a slow server.
    pub(crate) fn write<T>(&self, change: impl FnOnce() -> Result<T>) -> Result<T> {
        let _held = self.gate.hold();
        if self.is_cancelled() {
            return Err(SyncCancelled.into());
        }

        let result = change();
        self.gate.mark_changed();
        result
    }
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

/// Tags as sent by the server: each one encrypted on its own, holding a JSON string, or
/// all of them in one JSON array. Writers differ, and old rows of both kinds exist on
/// the server, so every reader takes both.
fn decrypt_tags(crypto: &CryptoService, key: &[u8; 32], tags: &[String]) -> Vec<String> {
    tags.iter()
        .flat_map(|tag_json| {
            let encrypted_tag: EncryptedData = serde_json::from_str(tag_json).ok()?;
            let tag_json_str = crypto.decrypt_text(&encrypted_tag, key).ok()?;

            if let Ok(tag) = serde_json::from_str::<String>(&tag_json_str) {
                if !tag.trim().is_empty() {
                    return Some(vec![tag]);
                }
            }

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

/// Tags as they go to the server: one ciphertext each, the form notes use.
fn encrypt_tags(crypto: &CryptoService, key: &[u8; 32], tags: &[String]) -> Result<Vec<String>> {
    tags.iter()
        .map(|tag| {
            let tag_json = serde_json::to_string(tag)?;
            let encrypted_tag = crypto.encrypt_text(&tag_json, key)?;
            Ok(serde_json::to_string(&encrypted_tag)?)
        })
        .collect()
}

/// What a sync run works with. Nothing here belongs to the terminal.
struct Run<'a> {
    conn: &'a Connection,
    crypto: &'a CryptoService,
    key: &'a [u8; 32],
    client: Client,
    endpoint: String,
    api_key: String,
    log: &'a dyn Fn(&str),
    link: &'a SyncLink,
}

impl Run<'_> {
    fn note_repo(&self) -> NoteRepository<'_> {
        NoteRepository::new(self.conn)
    }

    fn version_repo(&self) -> NoteVersionRepository<'_> {
        NoteVersionRepository::new(self.conn)
    }

    fn attachment_repo(&self) -> AttachmentRepository<'_> {
        AttachmentRepository::new(self.conn)
    }

    fn sync_repo(&self) -> SyncRepository<'_> {
        SyncRepository::new(self.conn)
    }

    fn saved_search_repo(&self) -> SavedSearchRepository<'_> {
        SavedSearchRepository::new(self.conn)
    }

    /// POST to a device endpoint and return the body, turning a refusal into an error
    /// that says what to do about it.
    fn post<T: Serialize>(&self, path: &str, body: &T) -> Result<String> {
        let send = || {
            self.client
                .post(format!("{}{path}", self.endpoint))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(body)
                .send()
                .with_context(|| format!("Failed to reach {path}"))
        };

        // A timed-out request is safe to send again, because a push that fails writes
        // nothing, and one more try usually saves waiting for the next sync
        let mut response = send()?;
        if response.status() == reqwest::StatusCode::REQUEST_TIMEOUT {
            response = send()?;
        }

        let status = response.status();
        if status.is_success() {
            return response.text().context("Failed to read the server's response");
        }

        let retry_after = response.headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok());
        let body = response.text().unwrap_or_default();
        let wait = retry_after.unwrap_or(60).min(MAX_RETRY_AFTER_SECONDS);

        Err(match status.as_u16() {
            403 => SyncRefused::AccountDisabled(t!("sync.account_deactivated").to_string()).into(),
            429 => SyncRefused::Wait { seconds: wait, message: t!("sync.rate_limited", seconds = wait).to_string() }.into(),
            503 => SyncRefused::Wait { seconds: wait, message: t!("sync.server_busy", seconds = wait).to_string() }.into(),
            507 => SyncRefused::QuotaExceeded(t!("sync.quota_exceeded").to_string()).into(),
            401 => anyhow::anyhow!("{}", t!("sync.key_refused")),
            _ => anyhow::anyhow!("{} {}: {}", path, status, char_prefix(&body, 200)),
        })
    }
}

/// Connect to the server this vault is set up for.
fn open_run<'a>(
    conn: &'a Connection,
    crypto: &'a CryptoService,
    key: &'a [u8; 32],
    metadata: &SyncMetadata,
    log: &'a dyn Fn(&str),
    link: &'a SyncLink,
) -> Result<Run<'a>> {
    let encrypted_api_key = metadata.api_key.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No API key configured"))?;
    let api_key_encrypted: EncryptedData = serde_json::from_str(encrypted_api_key)
        .map_err(|e| anyhow::anyhow!("Invalid API key format: {}", e))?;
    let api_key = crypto.decrypt_text(&api_key_encrypted, key)?;

    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("Failed to set up the HTTP client")?;

    Ok(Run { conn, crypto, key, client, endpoint: metadata.sync_endpoint.clone(), api_key, log, link })
}

/// A blob as it goes to the server: encrypted, wrapped in JSON, in base64.
fn blob_for_sync(run: &Run<'_>, id: &str, data: &[u8]) -> Result<SyncAttachment> {
    let encrypted_blob = run.crypto.encrypt_binary(data, run.key)?;
    let payload = serde_json::to_vec(&encrypted_blob)?;
    Ok(SyncAttachment { id: id.to_string(), data: general_purpose::STANDARD.encode(payload) })
}

/// A note as it goes to the server.
fn note_for_sync(run: &Run<'_>, note: &Note) -> Result<SyncNote> {
    let encrypted_content = run.crypto.encrypt_text(&note.content, run.key)?;

    Ok(SyncNote {
        id: note.id.clone(),
        created_at: note.created_at,
        modified_at: note.modified_at,
        content: serde_json::to_string(&encrypted_content)?,
        tags: encrypt_tags(run.crypto, run.key, &note.tags)?,
        attachments: attachment_refs(run.crypto, &note.attachments, run.key)?,
        pinned: note.pinned,
        archived: note.archived,
        archived_at: note.archived_at,
        locked: note.locked,
        locked_at: note.locked_at,
        deleted: note.deleted,
        deleted_at: note.deleted_at,
        version: note.version,
        word_wrap: Some(note.word_wrap),
        show_preview: note.show_preview,
        syntax_language: Some(note.syntax_name()),
        color: note.color.clone(),
        // Hash chain fields for git-like conflict detection
        content_hash: note.content_hash.clone(),
        parent_hash: note.parent_hash.clone(),
        hash_chain: note.hash_chain.clone(),
    })
}

/// A version as it goes to the server.
fn version_for_sync(run: &Run<'_>, version: &crate::repository::NoteVersion) -> Result<SyncNoteVersion> {
    let encrypted_content = run.crypto.encrypt_text(&version.content, run.key)?;

    Ok(SyncNoteVersion {
        version_key: format!("{}:{}", version.note_id, version.version),
        note_id: version.note_id.clone(),
        version: version.version,
        created_at: version.created_at,
        synced_at: version.synced_at,
        content: serde_json::to_string(&encrypted_content)?,
        tags: encrypt_tags(run.crypto, run.key, &version.tags)?,
        attachments: attachment_refs(run.crypto, &version.attachments, run.key)?,
        syntax_language: version.syntax_language.as_ref().map(|s| s.to_string()),
        word_wrap: version.word_wrap,
        show_preview: version.show_preview,
        color: version.color.clone(),
        reason: version.reason.to_string(),
        // Hash chain fields
        content_hash: version.content_hash.clone(),
        parent_hash: version.parent_hash.clone(),
    })
}

/// A note written by an older build has no hash yet. It gets one before it goes, so the
/// server can tell this version from the next.
fn with_hash(run: &Run<'_>, mut note: Note) -> Result<Note> {
    if note.content_hash.is_none() {
        let hash = run.crypto.compute_content_hash(&note, run.key);
        let chain = update_hash_chain(&hash, note.hash_chain.as_deref());
        run.link.write(|| run.note_repo().set_first_hash(&note.id, &hash, &chain))?;
        note.hash_chain = Some(chain);
        note.content_hash = Some(hash);
    }
    Ok(note)
}

/// One push: a batch of notes, with the versions and blobs the server hasn't got.
#[derive(Default)]
struct PushBatch {
    notes: Vec<Note>,
    request: SyncPushRequest,
    version_keys: Vec<String>,
    bytes: usize,
}

impl PushBatch {
    fn is_full(&self) -> bool {
        self.notes.len() >= PUSH_NOTES_PER_BATCH || self.bytes >= PUSH_BYTES_PER_BATCH
    }

    /// Add a note and whatever of it the server still needs.
    fn add(&mut self, run: &Run<'_>, note: Note) -> Result<()> {
        let note = with_hash(run, note)?;
        self.request.notes.push(note_for_sync(run, &note)?);

        for version in run.version_repo().get_unpushed_versions(&note.id, run.key)? {
            self.version_keys.push(version.version_key.clone());
            self.request.versions.push(version_for_sync(run, &version)?);
        }

        // A missing blob doesn't stop the push: the note syncs without it, the server
        // answers with attachmentWarnings, and any device holding the blob heals the gap
        for attachment in &note.attachments {
            if run.attachment_repo().is_pushed(&attachment.id)?
                || self.request.attachments.iter().any(|blob| blob.id == attachment.id)
            {
                continue;
            }

            match run.attachment_repo().get(&attachment.id, run.key) {
                Ok(Some((_, _, _, data))) => {
                    let blob = blob_for_sync(run, &attachment.id, &data)?;
                    self.bytes += blob.data.len();
                    self.request.attachments.push(blob);
                }
                Ok(None) => tracing::warn!("Attachment {} has no local blob, so the note goes without it", attachment.id),
                Err(e) => tracing::warn!("Attachment {} could not be read ({e}), so the note goes without it", attachment.id),
            }
        }

        self.notes.push(note);
        Ok(())
    }
}

/// What the server said about a refused note, in the form the conflict screen reads.
fn conflict_from_rejection(rejected: crate::models::sync::SyncRejected) -> ConflictData {
    ConflictData {
        note_id: rejected.id,
        server_content: rejected.server_content,
        server_tags: rejected.server_tags,
        server_modified_at: rejected.server_modified_at,
        server_version: rejected.server_version,
        server_attachments: rejected.server_attachments,
        server_pinned: rejected.server_pinned,
        server_archived: rejected.server_archived,
        server_archived_at: rejected.server_archived_at,
        server_locked: rejected.server_locked,
        server_locked_at: rejected.server_locked_at,
        server_syntax_language: rejected.server_syntax_language,
        server_word_wrap: rejected.server_word_wrap,
        server_show_preview: rejected.server_show_preview,
        server_color: rejected.server_color,
        detected_at: Utc::now(),
        // Hash chain fields for git-like conflict detection
        server_content_hash: rejected.server_content_hash,
        server_parent_hash: rejected.server_parent_hash,
        server_hash_chain: rejected.server_hash_chain,
        // Ancestor data for 3-way merge
        ancestor_hash: rejected.ancestor_hash,
        ancestor_content: rejected.ancestor_content,
        ancestor_tags: rejected.ancestor_tags,
    }
}

/// A pulled note in the same form, for a conflict the pull found.
fn conflict_from_pull(local: &Note, remote: &SyncNote) -> ConflictData {
    let ancestor_hash = match (&local.hash_chain, &remote.hash_chain) {
        (Some(local_chain), Some(remote_chain)) => crate::crypto::find_common_ancestor(local_chain, remote_chain),
        _ => None,
    };

    ConflictData {
        note_id: remote.id.clone(),
        server_content: remote.content.clone(),
        server_tags: remote.tags.clone(),
        server_modified_at: remote.modified_at,
        server_version: remote.version,
        server_attachments: remote.attachments.clone(),
        server_pinned: remote.pinned,
        server_archived: remote.archived,
        server_archived_at: remote.archived_at,
        server_locked: remote.locked,
        server_locked_at: remote.locked_at,
        server_syntax_language: remote.syntax_language.clone(),
        server_word_wrap: remote.word_wrap,
        server_show_preview: remote.show_preview,
        server_color: remote.color.clone(),
        detected_at: Utc::now(),
        server_content_hash: remote.content_hash.clone(),
        server_parent_hash: remote.parent_hash.clone(),
        server_hash_chain: remote.hash_chain.clone(),
        ancestor_hash,
        ancestor_content: None,
        ancestor_tags: None,
    }
}

/// Send one batch and record what the server made of it.
fn push_batch(run: &Run<'_>, mut batch: PushBatch, deletions: Vec<SyncDeletion>, authoritative: bool, outcome: &mut SyncOutcome) -> Result<()> {
    batch.request.deletions = deletions;
    batch.request.attachments_authoritative = authoritative;

    let body = run.post("/api/v1/sync/push", &batch.request)?;
    let mut response = SyncPushResponse::parse(&body).context("Failed to parse push response")?;

    let attachment_warnings = std::mem::take(&mut response.attachment_warnings);
    run.link.write(|| record_push(run, &batch, response, outcome))?;

    repair_missing_blobs(run, &attachment_warnings);
    Ok(())
}

/// Record what the server made of a batch, all of it or none.
fn record_push(run: &Run<'_>, batch: &PushBatch, response: SyncPushResponse, outcome: &mut SyncOutcome) -> Result<()> {
    let note_repo = run.note_repo();
    let version_repo = run.version_repo();
    let attachment_repo = run.attachment_repo();
    let sync_repo = run.sync_repo();

    // Only what this batch sent counts. The server doesn't get to name anything else.
    let pushed: HashMap<&str, &Note> = batch.notes.iter().map(|note| (note.id.as_str(), note)).collect();
    let sent_blobs: HashSet<&str> = batch.request.attachments.iter().map(|blob| blob.id.as_str()).collect();

    let accepted_ids: Vec<String> = response.accepted.iter().map(|note| note.id.clone()).collect();
    let rejected_ids: Vec<String> = response.rejected.iter().map(|note| note.id.clone()).collect();

    let transaction = run.conn.unchecked_transaction()?;

    for accepted in &response.accepted {
        let Some(note) = pushed.get(accepted.id.as_str()) else {
            continue;
        };
        outcome.synced += 1;
        note_repo.mark_pushed(note, accepted.synced_at)?;

        for attachment in note.attachments.iter().filter(|a| sent_blobs.contains(a.id.as_str())) {
            attachment_repo.mark_pushed(&attachment.id)?;
        }

        if let Ok(Some(stored)) = note_repo.get(&accepted.id, run.key) {
            let _ = version_repo.create_version(&stored, accepted.synced_at, VersionReason::Sync, run.key);
        }
    }
    version_repo.mark_pushed(&batch.version_keys)?;

    for rejected in response.rejected {
        if !pushed.contains_key(rejected.id.as_str()) {
            continue;
        }
        (run.log)(&format!("Sync conflict for note {}: {}", rejected.id, rejected.reason));
        outcome.rejected += 1;
        let note_id = rejected.id.clone();
        sync_repo.set_conflict(&note_id, &conflict_from_rejection(rejected))?;
    }

    // Deleted for good after this edit. The ids only count for what this batch sent.
    for id in &response.skipped_deleted {
        if let Some(note) = pushed.get(id.as_str()) {
            (run.log)(&format!("Sync - Note {id} was deleted for good, so it stops being sent"));
            note_repo.hold_skipped_note(note)?;
        }
    }

    let answered: HashSet<&str> = accepted_ids.iter().chain(&rejected_ids).chain(&response.skipped_deleted)
        .map(String::as_str)
        .collect();
    for note in batch.notes.iter().filter(|note| !answered.contains(note.id.as_str())) {
        outcome.unanswered_until = outcome.unanswered_until.max(Some(latest_date(note)));
    }

    // A deletion the server refused stops being sent, or the note would be announced
    // to every device on each push. The note itself comes back through the pull.
    let refused: HashSet<&String> = response.rejected_deletions.iter().collect();
    let (forgotten, delivered): (Vec<String>, Vec<String>) = batch.request.deletions.iter()
        .map(|deletion| deletion.id.clone())
        .partition(|id| refused.contains(id));
    note_repo.forget_deletions(&forgotten)?;
    note_repo.mark_deletions_synced(&delivered)?;

    transaction.commit()?;
    Ok(())
}

/// The furthest date a note carries, any of which the server checks.
fn latest_date(note: &Note) -> DateTime<Utc> {
    [Some(note.created_at), Some(note.modified_at), note.deleted_at, note.archived_at, note.locked_at]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(note.modified_at)
}

/// Whether a server whose clock says `server_now` will have refused this date.
fn beyond_server_clock(date: DateTime<Utc>, server_now: DateTime<Utc>) -> bool {
    date > server_now + chrono::Duration::hours(SERVER_CLOCK_TOLERANCE_HOURS)
}

/// Re-push blobs the server says it's missing, in one attachments-only push. Failing
/// here isn't worth failing the sync: the warning comes back on the next push.
fn repair_missing_blobs(run: &Run<'_>, warnings: &[AttachmentWarning]) {
    let mut repairs = SyncPushRequest::default();

    for warning in warnings {
        for id in &warning.attachment_ids {
            match run.attachment_repo().get(id, run.key) {
                Ok(Some((_, _, _, data))) => match blob_for_sync(run, id, &data) {
                    Ok(blob) => {
                        tracing::warn!("Server is missing attachment {id} (note {}), so it's going again", warning.note_id);
                        repairs.attachments.push(blob);
                    }
                    Err(e) => tracing::warn!("Could not prepare attachment {id} for repair: {e}"),
                },
                _ => tracing::warn!("Neither the server nor this device has attachment {id} (note {})", warning.note_id),
            }
        }
    }

    if repairs.attachments.is_empty() {
        return;
    }

    match run.post("/api/v1/sync/push", &repairs) {
        Ok(_) => {
            (run.log)("[Sync] Re-uploaded attachments the server was missing");
            let recorded = run.link.write(|| {
                repairs.attachments.iter().try_for_each(|blob| run.attachment_repo().mark_pushed(&blob.id))
            });
            if let Err(e) = recorded {
                tracing::warn!("Could not record the re-uploaded attachments: {e}");
            }
        }
        Err(e) => tracing::warn!("Attachment repair push failed: {e}"),
    }
}

/// Send everything the server hasn't got: changed notes, in batches, and deletions.
///
/// A push may only claim to hold each note's whole attachment list if this device can
/// vouch for it, and the claim covers the whole request, so notes it can't vouch for
/// go in their own pushes.
fn push_changes(run: &Run<'_>, metadata: &mut SyncMetadata, outcome: &mut SyncOutcome) -> Result<()> {
    let note_repo = run.note_repo();

    // The first sync with an account pushes the whole vault
    if metadata.last_sync_at.is_none() {
        run.link.write(|| {
            note_repo.mark_all_pending()?;
            run.saved_search_repo().mark_all_pending()
        })?;
    }

    let (vouched, unproven): (Vec<Note>, Vec<Note>) = note_repo.get_pending(run.key)?.notes
        .into_iter()
        .partition(|note| note.attachments_complete);

    // Pending deletions (from emptying the bin) go whether or not any note changed,
    // or an emptied bin would wait for the next edit to reach the server.
    let mut deletions: Vec<SyncDeletion> = note_repo.get_pending_deletions()?
        .into_iter()
        .map(|(id, deleted_at)| SyncDeletion {
            id,
            deleted_at: deleted_at.parse().unwrap_or_else(|_| Utc::now()),
        })
        .collect();

    let anything_to_push = !vouched.is_empty() || !unproven.is_empty() || !deletions.is_empty();

    // These go first and on their own, because a push that only deletes still works at
    // the storage quota, and it's how the user makes room.
    if !deletions.is_empty() {
        push_batch(run, PushBatch::default(), std::mem::take(&mut deletions), false, outcome)?;
    }

    match push_notes(run, vouched, unproven, outcome) {
        Err(e) if matches!(e.downcast_ref::<SyncRefused>(), Some(SyncRefused::QuotaExceeded(_))) => {
            // The notes stay pending, and there's still everything to pull
            outcome.quota_exceeded = true;
        }
        result => result?,
    }

    if anything_to_push {
        metadata.last_push_at = Some(Utc::now());
    }
    Ok(())
}

/// Send changed notes in batches, those this device can vouch for apart from the rest.
fn push_notes(run: &Run<'_>, vouched: Vec<Note>, unproven: Vec<Note>, outcome: &mut SyncOutcome) -> Result<()> {
    for (notes, authoritative) in [(vouched, true), (unproven, false)] {
        let mut batch = PushBatch::default();
        for note in notes {
            batch.add(run, note)?;
            if batch.is_full() {
                push_batch(run, std::mem::take(&mut batch), Vec::new(), authoritative, outcome)?;
            }
        }
        if !batch.notes.is_empty() {
            push_batch(run, batch, Vec::new(), authoritative, outcome)?;
        }
    }

    Ok(())
}

/// What applying a page of pulled notes needs.
struct PullPage<'a> {
    run: &'a Run<'a>,
    attachment_data: &'a HashMap<String, Vec<u8>>,
    synced_at: DateTime<Utc>,
    force: bool,
}

/// Read a pulled note's attachment list, storing any blobs that came with it. Returns
/// the list and how many references couldn't be read.
///
/// The list keeps a reference even when no blob came, because the server leaves out
/// blobs this device already has, and a shorter list here would later tell the server
/// the attachment is gone.
fn read_note_attachments(page: &PullPage<'_>, remote_note: &SyncNote) -> Result<(Vec<Attachment>, usize)> {
    let mut note_attachments: Vec<Attachment> = Vec::new();
    let mut unreadable = 0;

    for attachment_ref in &remote_note.attachments {
        let filename = match filename_from_sync(page.run.crypto, &attachment_ref.filename, page.run.key) {
            Ok(filename) => filename,
            Err(e) => {
                (page.run.log)(&format!("Pull - Failed to read filename for {}: {}", attachment_ref.id, e));
                unreadable += 1;
                continue;
            }
        };

        if let Some(decrypted_data) = page.attachment_data.get(&attachment_ref.id) {
            let attachment_repo = page.run.attachment_repo();
            attachment_repo.store(
                &attachment_ref.id,
                &filename,
                &attachment_ref.mime_type,
                attachment_ref.size,
                decrypted_data,
                page.run.key,
            )?;
            attachment_repo.mark_pushed(&attachment_ref.id)?;
        }

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

/// What to do with a pulled note this device already has.
#[derive(Debug, PartialEq, Eq)]
enum PullAction {
    /// It's the note this device has
    Ignore,
    /// It's older than the local edit, which stays and gets pushed
    KeepLocal,
    /// It's newer, or there's nothing local to protect
    Take,
    /// The two have diverged
    Conflict,
}

/// Whether two notes with the same hash differ anyway. Not every client's hash covers
/// every field: `deleted` is in nobody's, and some don't recompute it for a pin or an
/// attachment. showPreview stays out, because each device keeps its own.
fn differs_despite_equal_hash(local: &Note, remote: &SyncNote) -> bool {
    let local_attachments: HashSet<&str> = local.attachments.iter().map(|a| a.id.as_str()).collect();
    let remote_attachments: HashSet<&str> = remote.attachments.iter().map(|a| a.id.as_str()).collect();

    local.deleted != remote.deleted
        || local.pinned != remote.pinned
        || local.archived != remote.archived
        || local.locked != remote.locked
        || local.color != remote.color
        || local_attachments != remote_attachments
}

/// Decide what a pulled note means, following "Applying a pulled note" in the protocol.
///
/// Any pull can deliver a note this device already has, so this has to be idempotent,
/// and it must never overwrite a local edit that hasn't been pushed.
fn decide_pull(local: &Note, remote: &SyncNote, force: bool) -> PullAction {
    let pending = local.needs_sync;

    if let (Some(local_hash), Some(remote_hash)) = (&local.content_hash, &remote.content_hash) {
        if local_hash == remote_hash {
            return if !pending && differs_despite_equal_hash(local, remote) {
                PullAction::Take
            } else {
                PullAction::Ignore
            };
        }

        if !pending {
            return PullAction::Take;
        }

        let local_has_seen_it = local.parent_hash.as_ref() == Some(remote_hash)
            || local.hash_chain.as_ref().is_some_and(|chain| chain.contains(remote_hash));
        if local_has_seen_it {
            return PullAction::KeepLocal;
        }

        let includes_local_edit = remote.hash_chain.as_ref().is_some_and(|chain| chain.contains(local_hash));
        return if includes_local_edit { PullAction::Take } else { PullAction::Conflict };
    }

    // Without hashes on both sides, the times are all there is to go on
    let remote_is_newer = remote.modified_at > local.modified_at;

    if pending {
        return if remote_is_newer { PullAction::Conflict } else { PullAction::KeepLocal };
    }

    let lists_differ = local.attachments.len() != remote.attachments.len();
    let gains_a_hash = local.content_hash.is_none() && remote.content_hash.is_some();

    if force || remote_is_newer || lists_differ || gains_a_hash {
        PullAction::Take
    } else {
        PullAction::Ignore
    }
}

/// What applying a note did.
#[derive(Debug, Default, PartialEq, Eq)]
struct Applied {
    changed: bool,
    conflict: bool,
    unreadable_attachments: usize,
}

/// Replace a local note with what the server sent.
fn take_remote_note(
    page: &PullPage<'_>,
    mut local_note: Note,
    remote_note: &SyncNote,
    mut attachments: Vec<Attachment>,
    unreadable: usize,
) -> Result<()> {
    let run = page.run;

    // Decrypt first: a note that can't be read leaves this device's copy alone
    let encrypted_content: EncryptedData = serde_json::from_str(&remote_note.content)?;
    let decrypted_content = run.crypto.decrypt_text(&encrypted_content, run.key)?;

    // A reference that couldn't be read keeps what this device knew about it
    if unreadable > 0 {
        for attachment_ref in &remote_note.attachments {
            if !attachments.iter().any(|a| a.id == attachment_ref.id) {
                if let Some(known) = local_note.attachments.iter().find(|a| a.id == attachment_ref.id) {
                    attachments.push(known.clone());
                }
            }
        }
    }
    let complete = attachments.len() == remote_note.attachments.len();

    // Capture local version BEFORE overwriting with remote
    let _ = run.version_repo().create_version(&local_note, page.synced_at, VersionReason::Sync, run.key);

    local_note.content = decrypted_content;
    local_note.tags = decrypt_tags(run.crypto, run.key, &remote_note.tags);
    local_note.attachments = attachments;
    local_note.attachments_complete = complete;
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
    local_note.show_preview = remote_note.show_preview;
    if let Some(lang_str) = &remote_note.syntax_language {
        local_note.set_syntax_name(lang_str);
    }
    local_note.color = remote_note.color.clone();
    local_note.content_hash = remote_note.content_hash.clone();
    local_note.parent_hash = remote_note.parent_hash.clone();
    local_note.hash_chain = remote_note.hash_chain.clone();
    local_note.needs_sync = false;

    run.note_repo().store(&local_note, run.key)?;
    if remote_note.deleted {
        run.note_repo().mark_arrived_in_bin(&remote_note.id)?;
    }
    Ok(())
}

/// Add a note this device hasn't seen.
fn create_remote_note(page: &PullPage<'_>, remote_note: &SyncNote, attachments: Vec<Attachment>) -> Result<()> {
    let run = page.run;

    let encrypted_content: EncryptedData = serde_json::from_str(&remote_note.content)?;
    let decrypted_content = run.crypto.decrypt_text(&encrypted_content, run.key)?;

    let mut new_note = Note::new(decrypted_content);
    new_note.id = remote_note.id.clone();
    new_note.created_at = remote_note.created_at;
    new_note.modified_at = remote_note.modified_at;
    new_note.synced_at = Some(page.synced_at); // Mark as synced
    new_note.tags = decrypt_tags(run.crypto, run.key, &remote_note.tags);
    new_note.attachments_complete = attachments.len() == remote_note.attachments.len();
    new_note.attachments = attachments;
    new_note.pinned = remote_note.pinned;
    new_note.archived = remote_note.archived;
    new_note.archived_at = remote_note.archived_at;
    new_note.locked = remote_note.locked;
    new_note.locked_at = remote_note.locked_at;
    new_note.deleted = remote_note.deleted;
    new_note.deleted_at = remote_note.deleted_at;
    new_note.version = remote_note.version;
    new_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
    new_note.show_preview = remote_note.show_preview;
    if let Some(lang_str) = &remote_note.syntax_language {
        new_note.set_syntax_name(lang_str);
    }
    new_note.color = remote_note.color.clone();
    new_note.content_hash = remote_note.content_hash.clone();
    new_note.parent_hash = remote_note.parent_hash.clone();
    new_note.hash_chain = remote_note.hash_chain.clone();
    new_note.needs_sync = false;

    run.note_repo().create(&new_note, run.key)?;
    if remote_note.deleted {
        run.note_repo().mark_arrived_in_bin(&remote_note.id)?;
    }
    Ok(())
}

/// Apply one pulled note.
fn apply_remote_note(page: &PullPage<'_>, remote_note: SyncNote) -> Result<Applied> {
    let run = page.run;
    let (attachments, unreadable) = read_note_attachments(page, &remote_note)?;
    let mut applied = Applied { unreadable_attachments: unreadable, ..Default::default() };

    let Some(local_note) = run.note_repo().get(&remote_note.id, run.key)? else {
        create_remote_note(page, &remote_note, attachments)?;
        applied.changed = true;
        return Ok(applied);
    };

    // A note waiting for the user stays as it is, whatever arrives. The pull in the
    // same sync as a refused push lands here too, so it can't replace the local edit.
    // If the server has moved on, the conflict screen should show where it is now.
    let sync_repo = run.sync_repo();
    if let Some(conflict) = sync_repo.get_note_sync(&remote_note.id)?.and_then(|sync| sync.conflict_data) {
        if conflict.server_modified_at < remote_note.modified_at {
            let mut newer = conflict_from_pull(&local_note, &remote_note);

            // A pull has no ancestor's text to offer, and the one a refused push brought
            // is still a common ancestor, which is what a three-way merge needs
            if newer.ancestor_hash.is_none() || newer.ancestor_hash == conflict.ancestor_hash {
                newer.ancestor_hash = conflict.ancestor_hash;
                newer.ancestor_content = conflict.ancestor_content;
                newer.ancestor_tags = conflict.ancestor_tags;
            }

            sync_repo.set_conflict(&remote_note.id, &newer)?;
        }
        return Ok(applied);
    }

    match decide_pull(&local_note, &remote_note, page.force) {
        PullAction::Ignore => {
            // A full pull that agrees with this device proves its list
            let same_list = !differs_despite_equal_hash(&local_note, &remote_note);
            if same_list && !local_note.attachments_complete && !local_note.needs_sync && unreadable == 0 {
                let mut proven = local_note;
                proven.attachments_complete = true;
                run.note_repo().store(&proven, run.key)?;
            }
        }
        PullAction::KeepLocal => {}
        PullAction::Conflict => {
            sync_repo.set_conflict(&remote_note.id, &conflict_from_pull(&local_note, &remote_note))?;
            applied.conflict = true;
        }
        // Its text is in an input buffer, and saving that would quietly undo this. It's
        // a conflict the moment the user types, so it's one now.
        PullAction::Take if run.link.gate.note_is_open(&remote_note.id) => {
            sync_repo.set_conflict(&remote_note.id, &conflict_from_pull(&local_note, &remote_note))?;
            applied.conflict = true;
        }
        PullAction::Take => {
            take_remote_note(page, local_note, &remote_note, attachments, unreadable)?;
            applied.changed = true;
        }
    }

    Ok(applied)
}

/// Store a version from the server, if it's new here and can be read.
fn apply_remote_version(run: &Run<'_>, server_version: &SyncNoteVersion) -> Result<()> {
    let version_repo = run.version_repo();

    if version_repo.get_version_by_key(&server_version.version_key)?.is_some() {
        return Ok(());
    }

    let encrypted_content: EncryptedData = serde_json::from_str(&server_version.content)
        .context("Version content isn't ciphertext")?;
    let content = run.crypto.decrypt_text(&encrypted_content, run.key)?;

    let attachments = server_version.attachments.iter()
        .filter_map(|att_ref| Some(Attachment {
            id: att_ref.id.clone(),
            filename: filename_from_sync(run.crypto, &att_ref.filename, run.key).ok()?,
            mime_type: att_ref.mime_type.clone(),
            size: att_ref.size,
            data: att_ref.data.clone(),
            thumbnail_data: None,
        }))
        .collect();

    let reason = if server_version.reason == "manual-sync" {
        VersionReason::ManualSync
    } else {
        VersionReason::Sync
    };

    version_repo.insert_version_from_sync(&crate::repository::NoteVersion {
        version_key: server_version.version_key.clone(),
        note_id: server_version.note_id.clone(),
        version: server_version.version,
        created_at: server_version.created_at,
        synced_at: server_version.synced_at,
        content,
        tags: decrypt_tags(run.crypto, run.key, &server_version.tags),
        attachments,
        syntax_language: server_version.syntax_language.as_ref().and_then(|s| s.parse().ok()),
        word_wrap: Some(server_version.word_wrap.unwrap_or(true)),
        show_preview: server_version.show_preview,
        color: server_version.color.clone(),
        reason,
        // Hash chain fields from server
        content_hash: server_version.content_hash.clone(),
        parent_hash: server_version.parent_hash.clone(),
    }, run.key)
}

/// Store the versions whose notes are here, returning the rest.
///
/// A page holds a range of changes, so a version can arrive before its note, whose own
/// change is further on. It waits for that page. If the note never comes, because it
/// couldn't be read, the version is dropped with it when the pull ends.
fn apply_remote_versions(run: &Run<'_>, versions: Vec<SyncNoteVersion>) -> Result<Vec<SyncNoteVersion>> {
    let note_repo = run.note_repo();
    let mut waiting = Vec::new();

    for version in versions {
        if !note_repo.exists(&version.note_id)? {
            waiting.push(version);
            continue;
        }

        if let Err(e) = apply_remote_version(run, &version) {
            (run.log)(&format!("Pull - Skipping version {}: {e}", version.version_key));
        }
    }

    Ok(waiting)
}

/// Queue notes the server says were permanently deleted. An edit made after the deletion
/// beats it, as it does on the server, so that note stays and the next push brings it
/// back. Nothing is removed until the whole pull is in: see `settle_remote_deletions`.
fn hold_remote_deletions(run: &Run<'_>, deletions: Vec<SyncDeletion>) -> Result<()> {
    let note_repo = run.note_repo();

    for deletion in deletions {
        if !note_repo.exists(&deletion.id)? {
            continue;
        }
        if note_repo.has_edit_after(&deletion.id, deletion.deleted_at)? {
            (run.log)(&format!("Pull - Keeping note {}: edited after it was deleted", deletion.id));
            continue;
        }
        note_repo.hold_deletion(&deletion.id)?;
    }

    Ok(())
}

/// Act on the deletions a pull found.
///
/// Nothing authenticates these, so a compromised server could list every note. What it
/// gets is bounded: notes only ever go to the bin, and a pull that would take away a lot
/// waits for the user to say whether that's expected.
fn settle_remote_deletions(run: &Run<'_>, outcome: &mut SyncOutcome) -> Result<()> {
    let note_repo = run.note_repo();
    let transaction = run.conn.unchecked_transaction()?;

    // What's gone lately counts, or a server could stay under the limit on every sync.
    // What's been asked about stays asked, or taking some of it away would free the rest.
    let held = note_repo.count_held_deletions()?;
    let window_start = Utc::now() - chrono::Duration::hours(MASS_DELETION_WINDOW_HOURS);
    let lately = note_repo.count_arrived_in_bin_since(window_start)?;

    if held > 0 && (note_repo.has_unanswered_deletions()? || held + lately >= MASS_DELETION_THRESHOLD) {
        note_repo.mark_held_deletions_asked()?;
        outcome.deletions_held = held;
    } else {
        outcome.synced += note_repo.apply_held_deletions()?;
    }

    note_repo.purge_remote_deletions(Utc::now() - chrono::Duration::days(REMOTE_DELETION_RETENTION_DAYS))?;
    transaction.commit()?;
    Ok(())
}

/// Store the saved searches of a pull. A live row has to open with this vault's key, as
/// one that doesn't came from something without it. A deleted row is never shown, and
/// older clients pushed some as plaintext, so a deletion applies whatever the row holds.
fn apply_remote_saved_searches(
    run: &Run<'_>,
    searches: Vec<SyncSavedSearch>,
    synced_at: DateTime<Utc>,
    outcome: &mut SyncOutcome,
) -> Result<()> {
    let repo = run.saved_search_repo();

    for search in searches {
        if !search.deleted && !repo.is_readable(&search, run.key) {
            (run.log)(&format!("Pull - Skipping saved search {}: it won't open", search.id));
            outcome.skipped += 1;
            continue;
        }

        repo.apply(&search, false, Some(synced_at))?;
    }

    Ok(())
}

/// Send the saved searches the server hasn't got. These go after the pull, so a newer
/// copy from another device has already won, and the server's clock is known.
fn push_saved_searches(run: &Run<'_>, server_now: Option<DateTime<Utc>>, outcome: &mut SyncOutcome) -> Result<()> {
    // The server skips these without saying so, and they'd look pushed
    let (ahead, searches): (Vec<_>, Vec<_>) = run.saved_search_repo().pending()?
        .into_iter()
        .partition(|search| server_now.is_some_and(|now| beyond_server_clock(search.modified_at, now)));
    outcome.clock_ahead |= !ahead.is_empty();

    if searches.is_empty() {
        return Ok(());
    }

    let request = SyncPushRequest { saved_searches: searches, ..Default::default() };

    match run.post("/api/v1/sync/push", &request) {
        Ok(_) => {}
        Err(e) if matches!(e.downcast_ref::<SyncRefused>(), Some(SyncRefused::QuotaExceeded(_))) => {
            outcome.quota_exceeded = true;
            return Ok(());
        }
        Err(e) => return Err(e),
    }

    let now = Utc::now();
    run.link.write(|| {
        let repo = run.saved_search_repo();
        request.saved_searches.iter().try_for_each(|search| repo.mark_pushed(search, now))
    })
}

/// Decrypt the blobs of a pull. One that fails is left out.
fn read_blobs(run: &Run<'_>, attachments: &[SyncAttachment]) -> HashMap<String, Vec<u8>> {
    attachments.iter()
        .filter_map(|attachment| {
            let decoded = general_purpose::STANDARD.decode(&attachment.data).ok()?;
            let encrypted: EncryptedData = serde_json::from_slice(&decoded).ok()?;
            let data = run.crypto.decrypt_binary(&encrypted, run.key).ok()?;
            Some((attachment.id.clone(), data))
        })
        .collect()
}

/// Fetch the files of attachments this device has only heard of.
///
/// A pull only sends the blobs of the notes in that page, so a page cut off part-way, an
/// older pull or a conflict's server copy can leave a note naming a file that isn't here.
/// Each sync fetches a few. One that can't be fetched stays listed, viewing it says so,
/// and the next sync tries again.
fn fetch_missing_blobs(run: &Run<'_>, limit: usize) {
    let missing = match run.attachment_repo().list_missing_blobs() {
        Ok(missing) => missing,
        Err(e) => {
            tracing::warn!("Could not list missing attachments: {e}");
            return;
        }
    };

    for attachment in missing.iter().take(limit) {
        // Nothing else stops this loop, and the key is in memory until the thread ends
        if run.link.is_cancelled() {
            return;
        }

        match fetch_blob(run, &attachment.id) {
            Ok(Some(data)) => {
                let stored = run.link.write(|| {
                    let attachment_repo = run.attachment_repo();
                    attachment_repo.store(&attachment.id, &attachment.filename, &attachment.mime_type, attachment.size, &data, run.key)?;
                    attachment_repo.mark_pushed(&attachment.id)
                });
                if let Err(e) = stored {
                    tracing::warn!("Could not store attachment {}: {e}", attachment.id);
                }
            }
            Ok(None) => tracing::warn!("The server has no usable file for attachment {}", attachment.id),
            // A server that isn't answering won't answer the next one either, and each
            // try could take as long as a request is allowed
            Err(e) => {
                tracing::warn!("Could not fetch attachment {}: {e}", attachment.id);
                break;
            }
        }
    }
}

/// One attachment's file from the server, if it has one that opens.
fn fetch_blob(run: &Run<'_>, id: &str) -> Result<Option<Vec<u8>>> {
    let response = run.client
        .get(format!("{}/api/v1/sync/attachments/{id}", run.endpoint))
        .header("Authorization", format!("Bearer {}", run.api_key))
        .send()?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let blob = response.error_for_status()?.json::<SyncAttachment>()?;
    if blob.id != id {
        return Ok(None);
    }

    Ok(read_blobs(run, std::slice::from_ref(&blob)).remove(id))
}

/// What a pull found that belongs to the running app, rather than the database.
struct Pulled {
    inbox_items: Vec<InboxItem>,
}

/// Get everything that changed on the server.
///
/// This follows "Moving a client to the cursor" in the protocol. It sends a cursor and
/// looks at every response to see which kind of server answered, because a server can
/// be rolled back, or be a different one. A newer server takes the cursor and sends one
/// range of its changes per page. An older one ignores it and pages by offset.
fn pull_changes(run: &Run<'_>, metadata: &mut SyncMetadata, force: bool, outcome: &mut SyncOutcome) -> Result<Pulled> {
    let stored_cursor = if force { None } else { metadata.pull_cursor.clone() };
    let last_sync_at = if force { None } else { metadata.last_pull_at.or(metadata.last_sync_at) };

    // Timestamp pulls can have skipped changes without anyone noticing, so the first
    // time a server turns out to have cursors, this device starts again from the top.
    let mut full_pull_owed = stored_cursor.is_none() && last_sync_at.is_some();

    let mut request = SyncPullRequest {
        cursor: Some(stored_cursor.unwrap_or_default()),
        last_sync_at,
        // The server ignores these, but older ones require them to be there
        known_note_ids: Vec::new(),
        known_attachment_ids: if force { Vec::new() } else { run.attachment_repo().list_ids()? },
        limit: Some(PULL_PAGE_SIZE),
        offset: Some(0),
    };

    let mut first_page_at = None;
    let mut inbox_items = Vec::new();
    let mut versions_waiting: Vec<SyncNoteVersion> = Vec::new();

    for _ in 0..MAX_PULL_PAGES {
        let body = run.post("/api/v1/sync/pull", &request)?;
        let response = SyncPullResponse::parse(&body).context("Failed to parse pull response")?;
        outcome.skipped += response.dropped;
        first_page_at.get_or_insert(response.synced_at);

        let cursor_mode = response.next_cursor.is_some();
        let page_note_count = response.notes.len() as i64;
        let moved = response.next_cursor != request.cursor;
        let is_last_page = !response.has_more
            || if cursor_mode { !moved } else { page_note_count == 0 };

        let attachment_data = read_blobs(run, &response.attachments);
        let page = PullPage { run, attachment_data: &attachment_data, synced_at: response.synced_at, force };

        let restarting = run.link.write(|| {
            // A page applies whole or not at all, along with the position it moves to
            let transaction = run.conn.unchecked_transaction()?;

            // A note that can't be read costs that note, not the sync. Stopping here
            // would hold the cursor back and block every later change for good.
            for remote_note in response.notes {
                let note_id = remote_note.id.clone();
                match apply_remote_note(&page, remote_note) {
                    Ok(applied) => {
                        outcome.synced += applied.changed as usize;
                        outcome.rejected += applied.conflict as usize;
                        outcome.skipped += applied.unreadable_attachments;
                    }
                    Err(e) => {
                        tracing::warn!("Skipping note {note_id} from the server: {e}");
                        (run.log)(&format!("Pull - Skipping note {note_id}: {e}"));
                        outcome.skipped += 1;
                    }
                }
            }

            // In cursor mode each page has its own share of these. An older server sends
            // them whole with every page, so they're applied once, when every note is in.
            if cursor_mode || is_last_page {
                versions_waiting.extend(response.versions);
                versions_waiting = apply_remote_versions(run, std::mem::take(&mut versions_waiting))?;
                hold_remote_deletions(run, response.deletions)?;
                apply_remote_saved_searches(run, response.saved_searches, response.synced_at, outcome)?;
            }

            if is_last_page {
                inbox_items = response.inbox_items.unwrap_or_default();
            }

            let restarting = cursor_mode && std::mem::take(&mut full_pull_owed);

            if restarting {
                transaction.commit()?;
                return Ok(true);
            }

            if cursor_mode {
                // Versions still waiting for their note only exist in memory, so the saved
                // position stays behind them until they've gone in
                if versions_waiting.is_empty() {
                    run.sync_repo().set_pull_cursor(response.next_cursor.as_deref())?;
                }
                metadata.pull_cursor = response.next_cursor.clone();
                request.cursor = response.next_cursor;
            } else {
                metadata.pull_cursor = None;
                request.offset = Some(request.offset.unwrap_or(0) + page_note_count);
            }

            transaction.commit()?;
            Ok(false)
        })?;

        if restarting {
            request.cursor = Some(String::new());
            request.last_sync_at = None;
            continue;
        }

        if is_last_page {
            break;
        }
    }

    // The server's time at the first page, so whatever changes during a long pull is
    // fetched next time. It's only used if the cursor ever can't be.
    metadata.last_pull_at = first_page_at;

    Ok(Pulled { inbox_items })
}

/// What a finished sync leaves for the running app.
pub struct SyncReport {
    pub outcome: SyncOutcome,
    pub inbox_items: Vec<InboxItem>,
    /// A password change still owes the server a new wrapping of the key
    pub server_key_stale: bool,
    /// What the user needs to hear about the server's copy of the key, if anything
    pub server_key_notice: Option<String>,
}

/// Push and pull everything, working only with the database.
fn sync_vault(
    conn: &Connection,
    crypto: &CryptoService,
    key: &[u8; 32],
    force: bool,
    log: &dyn Fn(&str),
    link: &SyncLink,
) -> Result<SyncReport> {
    let sync_repo = SyncRepository::new(conn);
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
    let run = open_run(conn, crypto, key, &metadata, log, link)?;

    let mut outcome = SyncOutcome::default();

    push_changes(&run, &mut metadata, &mut outcome)?;
    let pulled = pull_changes(&run, &mut metadata, force, &mut outcome)?;
    link.write(|| settle_remote_deletions(&run, &mut outcome))?;

    // The server's time at the start of the pull
    let server_now = metadata.last_pull_at;
    outcome.clock_ahead = outcome.unanswered_until.zip(server_now)
        .is_some_and(|(date, now)| beyond_server_clock(date, now));

    push_saved_searches(&run, server_now, &mut outcome)?;
    fetch_missing_blobs(&run, MAX_BLOB_FETCHES_PER_SYNC);

    // last_sync_at stays on this device's clock. The pull position in last_pull_at
    // is the server's.
    metadata.last_sync_at = Some(Utc::now());
    link.write(|| sync_repo.record_finished_sync(&metadata))?;

    Ok(SyncReport {
        outcome,
        inbox_items: pulled.inbox_items,
        server_key_stale: metadata.server_key_stale,
        server_key_notice: None,
    })
}

/// A sync for a background thread to run.
struct SyncJob {
    db_path: std::path::PathBuf,
    password: Zeroizing<String>,
    key: Zeroizing<[u8; 32]>,
    force: bool,
    /// Look at the server's copy of the key once the sync is done
    check_server_key: bool,
    link: SyncLink,
    debug_log: Option<Arc<Mutex<std::fs::File>>>,
}

impl SyncJob {
    /// Do the work, on a database connection of this thread's own.
    fn run(self) -> Result<SyncReport> {
        let db = Database::open_another(&self.db_path, &self.password)?;
        let crypto = CryptoService::new();
        let log = |message: &str| crate::ui::app::write_debug_log(&self.debug_log, message);

        let mut report = sync_vault(db.connection(), &crypto, &self.key, self.force, &log, &self.link)?;

        if self.check_server_key || report.server_key_stale {
            match super::envelope::check_server_key(db.connection(), &crypto, &self.key, &self.password, &self.link, &log) {
                Ok(notice) => report.server_key_notice = notice,
                Err(e) if e.is::<SyncCancelled>() => return Err(e),
                Err(e) => log(&format!("[Sync] Could not check the server's wrapped key: {e}")),
            }
        }

        Ok(report)
    }
}

/// A sync running on its own thread.
pub struct BackgroundSync {
    receiver: Receiver<Result<SyncReport>>,
    cancelled: Arc<AtomicBool>,
    force: bool,
    #[cfg(test)]
    thread: Option<std::thread::JoinHandle<()>>,
}

impl BackgroundSync {
    /// Wait up to `patience` for it to finish, dropping the result.
    pub fn wait(&self, patience: Duration) {
        let _ = self.receiver.recv_timeout(patience);
    }
}

/// Start a sync on its own thread, so a slow server can't hold up the screen.
fn start_background_sync(
    app: &mut App,
    force: bool,
    key: [u8; 32],
    password: Zeroizing<String>,
) -> Result<()> {
    let link = SyncLink { gate: app.sync_gate.clone(), cancelled: Arc::default() };
    let cancelled = link.cancelled.clone();
    let (sender, receiver) = mpsc::channel();

    let job = SyncJob {
        db_path: app.db_path.clone(),
        password,
        key: Zeroizing::new(key),
        force,
        check_server_key: app.server_key_check_owed,
        link,
        debug_log: app.debug_log.clone(),
    };

    let thread = std::thread::Builder::new()
        .name("sync".to_string())
        .spawn(move || {
            let _ = sender.send(job.run());
        })
        .context("Failed to start the sync thread")?;
    #[cfg(not(test))]
    drop(thread);

    app.sync_job = Some(BackgroundSync {
        receiver,
        cancelled,
        force,
        #[cfg(test)]
        thread: Some(thread),
    });
    Ok(())
}

/// Stop a running sync from touching the vault again, without waiting for it.
///
/// Call this with the gate held, so that it can't be part-way through a page. A request
/// it has in flight may still arrive, which is harmless: sending a push twice is safe.
pub fn cancel_background_sync(app: &mut App) {
    app.sync_requested = None;
    if let Some(job) = app.sync_job.take() {
        job.cancelled.store(true, Ordering::SeqCst);
    }
}

/// Bring what a background sync has done so far onto the screen. A key press works on
/// the list in memory, and holding the gate keeps this true until it's finished with it.
pub fn absorb_sync_changes(app: &mut App) {
    if app.sync_gate.take_changed() {
        reload_current_view(app);
        app.last_sync_absorb = Some(Instant::now());
    }
}

/// Deal with a background sync that's finished. Until then the list is refreshed every
/// now and then: reloading decrypts the vault, and a long sync has plenty of pages.
pub fn poll_background_sync(app: &mut App) {
    let refreshed_lately = app.last_sync_absorb.is_some_and(|at| at.elapsed() < SYNC_REFRESH_INTERVAL);
    if !refreshed_lately {
        absorb_sync_changes(app);
    }

    let Some(job) = app.sync_job.as_ref() else {
        return;
    };
    let force = job.force;

    let result = match job.receiver.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Err(anyhow::anyhow!("{}", t!("sync.stopped_unexpectedly"))),
    };

    app.sync_job = None;
    finish_background_sync(app, result, force);
}

/// Show what a background sync did, as a sync on this thread would have.
fn finish_background_sync(app: &mut App, result: Result<SyncReport>, force: bool) {
    match result {
        Ok(report) => {
            app.inbox_items = report.inbox_items;
            app.server_key_check_owed = false;
            report_finished_sync(app, &report.outcome, force);
            if let Some(notice) = report.server_key_notice {
                app.error = Some(notice);
            }
        }
        Err(e) if e.is::<SyncCancelled>() => {}
        Err(e) => report_failed_sync(app, &e, force),
    }
}

/// Start the sync that's been asked for, if nothing stands in its way.
pub fn start_requested_sync(app: &mut App) {
    if app.sync_job.is_some() || app.is_editing() {
        return;
    }
    if let Some(force) = app.sync_requested.take() {
        run_sync_from_ui(app, force);
    }
}

/// Trigger manual sync
pub fn trigger_sync(app: &mut App) {
    run_sync_from_ui(app, false);
}

/// Force full resync from server
pub fn force_full_sync(app: &mut App) {
    run_sync_from_ui(app, true);
}

/// Sync on the user's behalf and show the result.
fn run_sync_from_ui(app: &mut App, force: bool) {
    // Check if sync is configured
    if !app.settings.sync_enabled {
        app.notify(t!("sync.not_enabled").to_string());
        return;
    }

    if app.settings.sync_endpoint.is_none() {
        app.notify(t!("sync.endpoint_not_configured").to_string());
        return;
    }

    // One at a time. What this was asked to send goes when the running one finishes.
    if app.sync_job.is_some() {
        app.sync_requested = Some(force || app.sync_requested.unwrap_or(false));
        return;
    }

    app.notify(if force { t!("sync.force_syncing") } else { t!("status.syncing") }.to_string());

    if let (true, Some(key), Some(password)) = (app.sync_in_background, app.key, app.unlock_password.clone()) {
        if let Err(e) = start_background_sync(app, force, key, password) {
            report_failed_sync(app, &e, force);
        }
        return;
    }

    match perform_sync(app, force) {
        Ok(outcome) => report_finished_sync(app, &outcome, force),
        Err(e) => report_failed_sync(app, &e, force),
    }
}

/// Show what a finished sync did.
fn report_finished_sync(app: &mut App, outcome: &SyncOutcome, force: bool) {
    // Reload notes from database to pick up sync changes
    if let Err(e) = super::notes::load_current_view(app) {
        app.error = Some(format!("Sync succeeded but failed to reload notes: {}", e));
    }
    // Refresh conflict cache after sync
    app.refresh_conflict_cache();
    let conflict_count = app.conflict_note_ids.len();

    let result = outcome.synced;
    let unit = if result == 1 { t!("sync.note").to_string() } else { t!("sync.notes").to_string() };
    let summary = if force {
        t!("sync.force_complete", count = result, unit = unit).to_string()
    } else {
        t!("sync.complete", count = result, unit = unit).to_string()
    };

    let status_msg = if conflict_count > 0 {
        let conflict_word = if conflict_count == 1 {
            t!("conflict.count_singular").to_string()
        } else {
            t!("conflict.count_plural").to_string()
        };
        format!("{} ({} {})", summary, conflict_count, conflict_word)
    } else {
        summary
    };
    app.notify(status_msg);
    report_skipped(app, outcome);

    // A disabled account gets its stream back when it's turned on again
    if app.sse_gave_up {
        app.start_sse_if_enabled();
    }
}

/// Show why a sync failed, and hold off automatic syncs if the server asked for that.
fn report_failed_sync(app: &mut App, error: &anyhow::Error, force: bool) {
    reload_after_failed_sync(app);

    match error.downcast_ref::<SyncRefused>() {
        Some(SyncRefused::Wait { seconds, .. }) => {
            app.sync_paused_until = Some(Instant::now() + Duration::from_secs(*seconds));
        }
        Some(SyncRefused::AccountDisabled(_)) => {
            app.sync_paused_until = Some(Instant::now() + ACCOUNT_DISABLED_PAUSE);
        }
        Some(SyncRefused::QuotaExceeded(_)) | None => {}
    }

    let label = if force { "Force sync failed" } else { "Sync failed" };
    let message = format!("{label}: {error}");
    app.error = Some(message.clone());
    app.notify(message);
}

/// How long automatic syncs stop for an account that's been disabled. It can be turned
/// back on, so they don't stop for good, and a manual sync still tries.
const ACCOUNT_DISABLED_PAUSE: Duration = Duration::from_secs(15 * 60);

/// Tell the user about anything the sync had to leave behind.
fn report_skipped(app: &mut App, outcome: &SyncOutcome) {
    if outcome.skipped > 0 {
        app.error = Some(t!("sync.skipped_notes", count = outcome.skipped).to_string());
    }
    if outcome.quota_exceeded {
        app.error = Some(t!("sync.quota_exceeded").to_string());
    }
    if outcome.clock_ahead {
        app.error = Some(t!("sync.clock_ahead").to_string());
    }
    app.held_deletions = outcome.deletions_held;
}

/// Accept what the server said about a large number of deletions: they go to the bin.
pub fn apply_held_deletions(app: &mut App) -> Result<()> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let moved = NoteRepository::new(db.connection()).apply_held_deletions()?;

    app.held_deletions = 0;
    super::notes::load_notes(app)?;
    app.notify(t!("sync.deletions_applied", count = moved).to_string());
    Ok(())
}

/// Refuse a large number of deletions: the notes stay, and go back to the server.
pub fn refuse_held_deletions(app: &mut App) -> Result<()> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    NoteRepository::new(db.connection()).refuse_held_deletions()?;

    app.held_deletions = 0;
    if app.settings.sync_enabled {
        app.trigger_sync();
    }
    Ok(())
}

/// A failed pull rolls back its database changes but not the list in memory.
fn reload_after_failed_sync(app: &mut App) {
    reload_current_view(app);
}

fn reload_current_view(app: &mut App) {
    if let Err(e) = super::notes::load_current_view(app) {
        app.debug_log(&format!("Could not reload notes after a sync: {}", e));
    }
}

/// Perform bidirectional sync with server
/// If force is true, pulls all notes from server regardless of last sync time
pub fn perform_sync(app: &mut App, force: bool) -> Result<SyncOutcome> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let debug_log = app.debug_log.clone();
    let log = move |message: &str| crate::ui::app::write_debug_log(&debug_log, message);

    let report = sync_vault(db.connection(), &app.crypto, key, force, &log, &SyncLink::default())?;
    app.inbox_items = report.inbox_items;

    // Reload notes to ensure UI is up to date
    super::notes::load_notes(app)?;

    // The server is reachable, so a re-wrap a password change couldn't deliver can go now,
    // and the device has a sync behind it, so it can repair the server's copy of the key
    if report.server_key_stale || std::mem::take(&mut app.server_key_check_owed) {
        if let Some(password) = app.unlock_password.clone() {
            if let Err(e) = super::envelope::repair_server_key(app, &password) {
                app.debug_log(&format!("[Sync] Could not update the server's wrapped key: {}", e));
            }
        }
    }

    Ok(report.outcome)
}

/// Fetch every attachment file this vault is missing, returning how many arrived.
pub fn fetch_all_missing_blobs(app: &App) -> Result<usize> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let metadata = SyncRepository::new(db.connection()).get_metadata()?.unwrap_or_default();
    let link = SyncLink::default();
    let log = |_: &str| {};
    let run = open_run(db.connection(), &app.crypto, key, &metadata, &log, &link)?;

    let before = run.attachment_repo().list_missing_blobs()?.len();
    fetch_missing_blobs(&run, usize::MAX);
    Ok(before - run.attachment_repo().list_missing_blobs()?.len())
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
    let server_tags = decrypt_tags(&app.crypto, key, &conflict_data.server_tags);

    // Decrypt ancestor content if available (for 3-way merge)
    let ancestor_content = conflict_data.ancestor_content.as_ref().and_then(|encrypted_ancestor| {
        let encrypted: crate::crypto::EncryptedData = serde_json::from_str(encrypted_ancestor).ok()?;
        app.crypto.decrypt_text(&encrypted, key).ok()
    });

    // Decrypt ancestor tags if available
    let ancestor_tags = conflict_data.ancestor_tags.as_ref()
        .map(|encrypted_tags| decrypt_tags(&app.crypto, key, encrypted_tags))
        .filter(|tags| !tags.is_empty());

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

/// Make `note`'s local edit a new version on top of what the server has, so the next
/// push is a fast-forward rather than the same conflict again.
fn build_on_server_version(crypto: &CryptoService, key: &[u8; 32], note: &mut Note, conflict: &ConflictData) {
    let hash = crypto.compute_content_hash(note, key);

    // A chain starts with its note's own hash, but not one from an older client
    let mut server_chain = conflict.server_hash_chain.clone().unwrap_or_default();
    if let Some(server_hash) = &conflict.server_content_hash {
        if server_chain.first() != Some(server_hash) {
            server_chain.insert(0, server_hash.clone());
        }
    }

    note.hash_chain = Some(update_hash_chain(&hash, Some(&server_chain)));
    if conflict.server_content_hash.is_some() {
        note.parent_hash = conflict.server_content_hash.clone();
    }
    note.content_hash = Some(hash);
    note.modified_at = Utc::now();
    note.version = note.version.max(conflict.server_version) + 1;
    note.needs_sync = true;
}

/// The server's attachment list from a conflict, and whether all of it could be read.
fn server_attachments(crypto: &CryptoService, key: &[u8; 32], conflict: &ConflictData) -> (Vec<Attachment>, bool) {
    let attachments: Vec<Attachment> = conflict.server_attachments.iter()
        .filter_map(|reference| Some(Attachment {
            id: reference.id.clone(),
            filename: filename_from_sync(crypto, &reference.filename, key).ok()?,
            mime_type: reference.mime_type.clone(),
            size: reference.size,
            data: reference.data.clone(),
            thumbnail_data: None,
        }))
        .collect();

    let complete = attachments.len() == conflict.server_attachments.len();
    (attachments, complete)
}

/// Resolve conflict by keeping local version
/// Clears conflict status and marks note for sync
pub fn resolve_keep_local(app: &mut App) -> Result<()> {
    let note_id = app.conflict_note_id.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict being resolved"))?;

    let conflict_data = app.conflict_data.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict data available"))?;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let note_repo = NoteRepository::new(db.connection());
    let sync_repo = SyncRepository::new(db.connection());

    let transaction = db.connection().unchecked_transaction()?;
    if let Some(mut note) = note_repo.get(&note_id, key)? {
        build_on_server_version(&app.crypto, key, &mut note, &conflict_data);
        note_repo.store(&note, key)?;
    }
    sync_repo.clear_conflict(&note_id)?;
    transaction.commit()?;

    // Clear conflict state in app
    clear_conflict_state(app);
    super::notes::load_notes(app)?;

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

    let (attachments, complete) = server_attachments(&app.crypto, key, &conflict_data);

    let transaction = db.connection().unchecked_transaction()?;

    // Get the local note to save a version before overwriting
    if let Some(local_note) = note_repo.get(&note_id, key)? {
        // Create version snapshot of local before overwriting
        let _ = version_repo.create_version(&local_note, Utc::now(), VersionReason::Sync, key);

        // Create updated note with server content (already decrypted in conflict_server_content)
        let mut updated_note = local_note.clone();
        updated_note.content = app.conflict_server_content.clone();
        updated_note.tags = app.conflict_server_tags.clone();
        updated_note.attachments = attachments.clone();
        updated_note.attachments_complete = complete;
        updated_note.modified_at = conflict_data.server_modified_at;
        updated_note.synced_at = Some(Utc::now());
        updated_note.version = conflict_data.server_version;
        updated_note.pinned = conflict_data.server_pinned;
        updated_note.archived = conflict_data.server_archived;
        updated_note.archived_at = conflict_data.server_archived_at;
        updated_note.locked = conflict_data.server_locked;
        updated_note.locked_at = conflict_data.server_locked_at;
        if let Some(ref lang) = conflict_data.server_syntax_language {
            updated_note.set_syntax_name(lang);
        }
        if let Some(wrap) = conflict_data.server_word_wrap {
            updated_note.word_wrap = wrap;
        }
        updated_note.show_preview = conflict_data.server_show_preview;
        updated_note.color = conflict_data.server_color.clone();
        // It's the server's note now, history and all, so there's nothing to push
        updated_note.content_hash = conflict_data.server_content_hash.clone();
        updated_note.parent_hash = conflict_data.server_parent_hash.clone();
        updated_note.hash_chain = conflict_data.server_hash_chain.clone();
        updated_note.needs_sync = false;

        note_repo.store(&updated_note, key)?;
    }

    // Clear conflict status
    sync_repo.clear_conflict(&note_id)?;
    transaction.commit()?;

    // Clear conflict state in app and reload notes
    clear_conflict_state(app);
    super::notes::load_notes(app)?;

    Ok(())
}

/// Resolve conflict by keeping both versions.
///
/// The server's version becomes a new note and the local edit stays where it is, on top
/// of what the server has. Both are pending, so both reach every device.
pub fn resolve_keep_both(app: &mut App) -> Result<String> {
    let note_id = app.conflict_note_id.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict being resolved"))?;

    let conflict_data = app.conflict_data.clone()
        .ok_or_else(|| anyhow::anyhow!("No conflict data available"))?;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

    let note_repo = NoteRepository::new(db.connection());
    let sync_repo = SyncRepository::new(db.connection());

    let (attachments, complete) = server_attachments(&app.crypto, key, &conflict_data);

    // Create a new note from the server version
    let mut new_note = Note::new(app.conflict_server_content.clone());
    new_note.tags = app.conflict_server_tags.clone();
    new_note.attachments = attachments.clone();
    new_note.attachments_complete = complete;
    new_note.pinned = conflict_data.server_pinned;
    new_note.archived = conflict_data.server_archived;
    new_note.archived_at = conflict_data.server_archived_at;
    new_note.locked = conflict_data.server_locked;
    new_note.locked_at = conflict_data.server_locked_at;
    if let Some(ref lang) = conflict_data.server_syntax_language {
        new_note.set_syntax_name(lang);
    }
    if let Some(wrap) = conflict_data.server_word_wrap {
        new_note.word_wrap = wrap;
    }
    new_note.show_preview = conflict_data.server_show_preview;
    new_note.color = conflict_data.server_color.clone();

    let new_note_id = new_note.id.clone();

    let transaction = db.connection().unchecked_transaction()?;
    note_repo.create(&new_note, key)?;

    if let Some(mut local_note) = note_repo.get(&note_id, key)? {
        build_on_server_version(&app.crypto, key, &mut local_note, &conflict_data);
        note_repo.store(&local_note, key)?;
    }

    // Clear conflict status on the original note
    sync_repo.clear_conflict(&note_id)?;
    transaction.commit()?;

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
    use crate::test_support::{read_request, Request};
    use serde_json::{json, Value};
    use std::io::Write;
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

    /// What a scripted pull gets. A failure sits in the sequence with the pages, so it
    /// lands on an exact request with nothing having to win a race to set it up.
    enum Scripted {
        Page(Value),
        Failure(&'static str),
    }

    /// A request a test wants the stub to sit on, as a slow server would.
    struct Held {
        path: &'static str,
        reached: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
    }

    /// The test's end of a held request.
    struct HeldRequest {
        reached: std::sync::mpsc::Receiver<()>,
        release: std::sync::mpsc::Sender<()>,
    }

    impl HeldRequest {
        /// Wait until the request is at the server.
        fn wait_until_reached(&self) {
            self.reached.recv().unwrap();
        }

        /// Let the server answer.
        fn release(&self) {
            self.release.send(()).unwrap();
        }
    }

    /// Answers pull requests the way the sync server does, one page at a time,
    /// and records what it was asked for.
    struct StubServer {
        endpoint: String,
        pulls: Arc<Mutex<Vec<Value>>>,
        pushes: Arc<Mutex<Vec<Value>>>,
        refuse: Arc<Mutex<std::collections::HashSet<String>>>,
        /// Responses to give to the next pulls, in order, instead of the usual pages
        script: Arc<Mutex<std::collections::VecDeque<Scripted>>>,
        /// Merged into every push response
        push_reply: Arc<Mutex<Value>>,
        /// The next response's status line and extra headers, instead of 200
        fail_next: Arc<Mutex<Option<String>>>,
        /// The wrapped key the server holds
        wrapped_key: Arc<Mutex<Option<String>>>,
        /// Refuse any push with notes in it, as a full account does
        quota_full: Arc<Mutex<bool>>,
        /// The next request to this path doesn't get an answer until the test says
        held: Arc<Mutex<Option<Held>>>,
        /// Note ids that are neither accepted nor rejected, as a server skips a note
        passed_over: Arc<Mutex<std::collections::HashSet<String>>>,
        /// How many attachment files it's been asked for
        blob_requests: Arc<Mutex<usize>>,
    }

    fn reply(stream: &mut std::net::TcpStream, body: &str) {
        reply_with(stream, "200 OK", body);
    }

    fn reply_with(stream: &mut std::net::TcpStream, status: &str, body: &str) {
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
            let script = Arc::new(Mutex::new(std::collections::VecDeque::new()));
            let scripted: Arc<Mutex<std::collections::VecDeque<Scripted>>> = script.clone();
            let push_reply = Arc::new(Mutex::new(json!({})));
            let push_extras = push_reply.clone();
            let fail_next: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let failing = fail_next.clone();
            let wrapped_key: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let held_key = wrapped_key.clone();
            let quota_full = Arc::new(Mutex::new(false));
            let over_quota = quota_full.clone();
            let held: Arc<Mutex<Option<Held>>> = Arc::new(Mutex::new(None));
            let holding = held.clone();
            let blobs = attachments.clone();
            let passed_over = Arc::new(Mutex::new(std::collections::HashSet::new()));
            let passing_over: Arc<Mutex<std::collections::HashSet<String>>> = passed_over.clone();
            let blob_requests = Arc::new(Mutex::new(0));
            let counting_blobs = blob_requests.clone();

            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let mut stream = stream.unwrap();
                    let Some(Request { method, path, body, .. }) = read_request(&mut stream) else {
                        continue;
                    };

                    let hold = {
                        let mut holding = holding.lock().unwrap();
                        if holding.as_ref().is_some_and(|hold| path.ends_with(hold.path)) { holding.take() } else { None }
                    };
                    if let Some(hold) = hold {
                        hold.reached.send(()).unwrap();
                        // Long enough never to matter, and a sync on the test's own
                        // thread would otherwise wait here for good
                        let _ = hold.release.recv_timeout(std::time::Duration::from_secs(120));
                    }

                    if let Some(status) = failing.lock().unwrap().take() {
                        reply_with(&mut stream, &status, r#"{"error":"refused"}"#);
                        continue;
                    }

                    if path.ends_with("/auth/wrapped-key") {
                        if method == "PUT" {
                            let request: Value = serde_json::from_slice(&body).unwrap();
                            *held_key.lock().unwrap() = request["blob"].as_str().map(String::from);
                            reply(&mut stream, "{}");
                        } else {
                            match held_key.lock().unwrap().as_ref() {
                                Some(blob) => reply(&mut stream, &json!({
                                    "blob": blob, "kdfVersion": 1, "kdfIterations": 1_000_000,
                                }).to_string()),
                                None => reply_with(&mut stream, "404 Not Found", "{}"),
                            }
                        }
                        continue;
                    }

                    if method == "GET" {
                        *counting_blobs.lock().unwrap() += 1;
                        let id = path.rsplit('/').next().unwrap_or("");
                        match blobs.iter().find(|blob| blob["id"] == id) {
                            Some(blob) => reply(&mut stream, &blob.to_string()),
                            None => reply_with(&mut stream, "404 Not Found", "{}"),
                        }
                        continue;
                    }

                    let request: Value = serde_json::from_slice(&body).unwrap();

                    // A push has notes of its own. Conflicts are the ones marked
                    // for the server to refuse.
                    if let Some(pushed) = request["notes"].as_array().cloned() {
                        if !pushed.is_empty() && *over_quota.lock().unwrap() {
                            reply_with(&mut stream, "507 Insufficient Storage", r#"{"error":"full","code":"storage_quota_exceeded"}"#);
                            continue;
                        }

                        let (rejected, accepted): (Vec<Value>, Vec<Value>) = pushed.iter()
                            .filter(|note| !passing_over.lock().unwrap().contains(note["id"].as_str().unwrap_or("")))
                            .cloned()
                            .partition(|note| rejecting.lock().unwrap().contains(note["id"].as_str().unwrap_or("")));
                        let mut response = json!({
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
                                "serverContent": encrypted("The server's text"),
                                "serverTags": [],
                                "serverAttachments": [],
                                "serverPinned": false,
                                "serverArchived": false,
                                "serverLocked": false,
                                "serverContentHash": "server-hash",
                                "serverHashChain": ["server-hash", "older-hash"],
                            })).collect::<Vec<_>>(),
                            "errors": [],
                        });
                        for (field, value) in push_extras.lock().unwrap().as_object().unwrap() {
                            response[field] = value.clone();
                        }
                        pushes_seen.lock().unwrap().push(request);
                        reply(&mut stream, &response.to_string());
                        continue;
                    }

                    let next = scripted.lock().unwrap().pop_front();
                    if let Some(next) = next {
                        recorded.lock().unwrap().push(request);
                        match next {
                            Scripted::Page(page) => reply(&mut stream, &page.to_string()),
                            Scripted::Failure(status) => reply_with(&mut stream, status, r#"{"error":"refused"}"#),
                        }
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

            Self { endpoint, pulls, pushes, refuse, script, push_reply, fail_next, wrapped_key, quota_full, held, passed_over, blob_requests }
        }

        fn blob_requests(&self) -> usize {
            *self.blob_requests.lock().unwrap()
        }

        /// Leave this note out of every push response from now on.
        fn pass_over(&self, id: &str) {
            self.passed_over.lock().unwrap().insert(id.to_string());
        }

        /// Sit on the next request to a path ending in `path`.
        fn hold_next(&self, path: &'static str) -> HeldRequest {
            let (reached_sender, reached) = std::sync::mpsc::channel();
            let (release, release_receiver) = std::sync::mpsc::channel();
            *self.held.lock().unwrap() = Some(Held { path, reached: reached_sender, release: release_receiver });
            HeldRequest { reached, release }
        }

        /// Give these pull responses next, in order.
        fn script(&self, pages: Vec<Value>) {
            self.script.lock().unwrap().extend(pages.into_iter().map(Scripted::Page));
        }

        /// Fail the pull after those already scripted, with this status line.
        fn script_failure(&self, status: &'static str) {
            self.script.lock().unwrap().push_back(Scripted::Failure(status));
        }

        /// Add these fields to every push response from now on.
        fn add_to_push_replies(&self, fields: Value) {
            *self.push_reply.lock().unwrap() = fields;
        }

        /// Answer the next request with this status line, and any headers after it.
        fn fail_next(&self, status: &str) {
            *self.fail_next.lock().unwrap() = Some(status.to_string());
        }

        fn all_pushes(&self) -> Vec<Value> {
            self.pushes.lock().unwrap().clone()
        }

        fn all_pulls(&self) -> Vec<Value> {
            self.pulls.lock().unwrap().clone()
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

    /// An unlocked app syncing in the background, as the terminal app does. A second
    /// connection needs a database on disk.
    fn app_syncing_in_background(endpoint: &str) -> App {
        let dir = std::env::temp_dir().join(format!("jottery-sync-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        let password = uuid::Uuid::new_v4().to_string();

        let mut app = App::new(path.clone(), None).unwrap();
        let db = Database::open(&path, &password).unwrap();
        let api_key = app.crypto.encrypt_text("device-api-key", key()).unwrap();
        SyncRepository::new(db.connection()).update_metadata(&SyncMetadata {
            api_key: Some(serde_json::to_string(&api_key).unwrap()),
            sync_enabled: true,
            sync_endpoint: endpoint.to_string(),
            ..Default::default()
        }).unwrap();

        app.db = Some(db);
        app.key = Some(*key());
        app.unlock_password = Some(Zeroizing::new(password));
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(endpoint.to_string());
        app.state = crate::ui::state::AppState::NoteList;
        app.enable_background_sync();
        app
    }

    /// Wait for the thread to end, and deal with its result as a tick would.
    fn finish_background_sync_now(app: &mut App) {
        let job = app.sync_job.as_mut().expect("a sync is running");
        job.thread.take().unwrap().join().unwrap();
        poll_background_sync(app);
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
        let mut connected = app_connected_to(&server.endpoint);
        let path = connected.db_path.clone();
        let db = connected.db.take().unwrap();
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

    /// A pull response from a server with cursors.
    fn cursor_page(notes: Vec<Value>, next_cursor: &str, has_more: bool) -> Value {
        json!({
            "notes": notes,
            "deletions": [],
            "attachments": [],
            "versions": [],
            "syncedAt": server_time(0),
            "hasMore": has_more,
            "nextCursor": next_cursor,
        })
    }

    fn cursors_sent(server: &StubServer) -> Vec<Value> {
        server.all_pulls().iter().map(|pull| pull["cursor"].clone()).collect()
    }

    #[test]
    fn a_server_with_cursors_is_followed_page_by_page() {
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![
            cursor_page(remote_notes(1), "c1", true),
            cursor_page(vec![], "c2", true),
            cursor_page(vec![], "c3", false),
        ]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        // hasMore alone decides: the middle page holds no notes and there was still more
        assert_eq!(cursors_sent(&server), vec![json!(""), json!("c1"), json!("c2")]);
        assert_eq!(sync_metadata(&app).pull_cursor.as_deref(), Some("c3"));
        assert_eq!(stored_note_count(&app), 1);
    }

    #[test]
    fn the_next_sync_starts_from_the_stored_cursor() {
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![cursor_page(vec![], "c1", false), cursor_page(vec![], "c1", false)]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();
        perform_sync(&mut app, false).unwrap();

        assert_eq!(cursors_sent(&server), vec![json!(""), json!("c1")]);
    }

    #[test]
    fn an_older_server_is_paged_by_offset_and_no_cursor_is_kept() {
        let server = StubServer::start(remote_notes(150), vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.offsets(), vec![0, 100]);
        assert_eq!(sync_metadata(&app).pull_cursor, None);
        assert_eq!(stored_note_count(&app), 150);
    }

    /// A server can be rolled back, so the answer can't be remembered.
    #[test]
    fn a_server_that_stops_sending_cursors_goes_back_to_offsets() {
        let server = StubServer::start(remote_notes(1), vec![]);
        server.script(vec![cursor_page(vec![], "c1", false)]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();
        assert_eq!(sync_metadata(&app).pull_cursor.as_deref(), Some("c1"));

        perform_sync(&mut app, false).unwrap();

        assert_eq!(sync_metadata(&app).pull_cursor, None);
        assert_eq!(stored_note_count(&app), 1);
    }

    #[test]
    fn a_failed_page_leaves_the_cursor_on_the_last_page_applied() {
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![cursor_page(remote_notes(1), "c1", true)]);
        server.script_failure("500 Internal Server Error");
        let mut app = app_connected_to(&server.endpoint);

        let db = app.db.as_ref().unwrap();
        NoteRepository::new(db.connection()).create(&Note::new("Local".to_string()), key()).unwrap();

        let outcome = perform_sync(&mut app, false);

        assert!(outcome.is_err());
        assert_eq!(sync_metadata(&app).pull_cursor.as_deref(), Some("c1"));
        assert_eq!(stored_note_count(&app), 2);
    }

    #[test]
    fn a_slow_server_leaves_the_screen_free() {
        let server = StubServer::start(remote_notes(1), vec![]);
        let held = server.hold_next("/sync/pull");
        let mut app = app_syncing_in_background(&server.endpoint);

        trigger_sync(&mut app);
        held.wait_until_reached();

        // The server hasn't answered, and this thread is still here
        assert!(app.sync_job.is_some());
        app.on_tick();
        assert_eq!(stored_note_count(&app), 0);

        held.release();
        finish_background_sync_now(&mut app);

        assert_eq!(stored_note_count(&app), 1);
        assert_eq!(app.notes.len(), 1);
        assert!(app.sync_job.is_none());
    }

    /// The list on screen is what the next key press works on, so it can't wait for the
    /// end of a long sync.
    #[test]
    fn notes_show_as_their_pages_arrive() {
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![cursor_page(remote_notes(1), "c1", true), cursor_page(vec![], "c2", false)]);
        let mut app = app_syncing_in_background(&server.endpoint);
        let held = server.hold_next("/sync/pull");

        // The second pull only goes once the first page is in
        trigger_sync(&mut app);
        held.wait_until_reached();
        let second = server.hold_next("/sync/pull");
        held.release();
        second.wait_until_reached();

        absorb_sync_changes(&mut app);
        assert_eq!(app.notes.len(), 1);

        second.release();
        finish_background_sync_now(&mut app);
    }

    #[test]
    fn a_pin_made_during_a_background_push_still_goes() {
        let server = StubServer::start(vec![], vec![]);
        let held = server.hold_next("/sync/push");
        let mut app = app_syncing_in_background(&server.endpoint);
        let note = Note::new("Pinned while the push was out".to_string());
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();

        trigger_sync(&mut app);
        held.wait_until_reached();

        let repo_conn = app.db.as_ref().unwrap();
        let repo = NoteRepository::new(repo_conn.connection());
        let mut pinned = repo.get(&note.id, key()).unwrap().unwrap();
        pinned.pinned = true;
        repo.update(&pinned, key()).unwrap();

        held.release();
        finish_background_sync_now(&mut app);

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).get(&note.id, key()).unwrap().unwrap();
        assert!(stored.pinned);
        assert!(stored.needs_sync);
    }

    /// A note from an older build gets its hash during a push, and that used to write the
    /// whole note back, undoing whatever had just been saved.
    #[test]
    fn hashing_a_note_for_a_push_leaves_the_rest_of_it_alone() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let note = Note::new("From before hashes".to_string());
        let repo = NoteRepository::new(db.connection());
        repo.create(&note, key()).unwrap();
        db.connection().execute("UPDATE notes SET content_hash = NULL, hash_chain = NULL", []).unwrap();
        let stale = repo.get(&note.id, key()).unwrap().unwrap();

        let mut edited = stale.clone();
        edited.content = "Saved after the push read it".to_string();
        repo.store(&edited, key()).unwrap();

        let link = SyncLink::default();
        let log = |_: &str| {};
        let metadata = sync_metadata(&app);
        let crypto = CryptoService::new();
        let db = app.db.as_mut().unwrap();
        let run = open_run(db.connection(), &crypto, key(), &metadata, &log, &link).unwrap();
        with_hash(&run, stale).unwrap();

        let stored = NoteRepository::new(db.connection()).get(&note.id, key()).unwrap().unwrap();
        assert_eq!(stored.content, "Saved after the push read it");
        assert!(stored.content_hash.is_some());
    }

    /// Its text is in a buffer, so saving that would take the pulled edit away.
    #[test]
    fn a_pull_leaves_an_open_note_alone_and_says_so() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_syncing_in_background(&server.endpoint);
        let mut note = Note::new("As opened".to_string());
        note.id = note_id(0);
        note.needs_sync = false;
        note.content_hash = Some("local-hash".to_string());
        let db = app.db.as_ref().unwrap();
        let repo = NoteRepository::new(db.connection());
        repo.create(&note, key()).unwrap();
        repo.store(&note, key()).unwrap();

        let mut remote = remote_notes(1).remove(0);
        remote["content"] = json!(encrypted("Changed elsewhere"));
        remote["contentHash"] = json!("remote-hash");
        remote["hashChain"] = json!(["remote-hash", "local-hash"]);
        server.script(vec![cursor_page(vec![remote], "c1", false)]);

        app.sync_gate.set_open_note(Some(&note.id));
        trigger_sync(&mut app);
        finish_background_sync_now(&mut app);

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).get(&note.id, key()).unwrap().unwrap();
        assert_eq!(stored.content, "As opened");
        let sync = SyncRepository::new(db.connection()).get_note_sync(&note.id).unwrap().unwrap();
        assert!(sync.conflict_data.is_some());
    }

    #[test]
    fn automatic_syncs_wait_for_an_edit_to_finish() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_syncing_in_background(&server.endpoint);
        app.settings.auto_sync_interval_minutes = 5;

        app.state = crate::ui::state::AppState::NoteView;
        app.check_auto_sync();
        assert!(app.sync_job.is_none());

        app.state = crate::ui::state::AppState::NoteList;
        app.check_auto_sync();
        assert!(app.sync_job.is_some());

        finish_background_sync_now(&mut app);
    }

    #[test]
    fn a_notification_during_an_edit_syncs_when_it_is_over() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_syncing_in_background(&server.endpoint);
        app.sse_handle = Some(crate::sse::SseHandle::holding(vec![crate::sse::SseMessage::SyncNotification]));

        app.editing_note_id = Some(note_id(0));
        app.check_auto_sync();
        assert!(app.sync_job.is_none());

        app.editing_note_id = None;
        app.check_auto_sync();
        assert!(app.sync_job.is_some());

        finish_background_sync_now(&mut app);
    }

    /// A save during a sync has to go somewhere, as the running one has read its notes.
    #[test]
    fn a_sync_asked_for_during_one_runs_after_it() {
        let server = StubServer::start(vec![], vec![]);
        let held = server.hold_next("/sync/pull");
        let mut app = app_syncing_in_background(&server.endpoint);

        trigger_sync(&mut app);
        held.wait_until_reached();
        trigger_sync(&mut app);
        assert_eq!(app.sync_requested, Some(false));

        held.release();
        finish_background_sync_now(&mut app);
        assert!(app.sync_job.is_none());

        app.check_auto_sync();
        finish_background_sync_now(&mut app);

        assert_eq!(server.all_pulls().len(), 2);
        assert_eq!(app.sync_requested, None);
    }

    /// Locking can't wait for a slow server, so the sync has to stop by itself.
    #[test]
    fn a_sync_stops_changing_the_vault_once_it_is_locked() {
        let server = StubServer::start(remote_notes(1), vec![]);
        let held = server.hold_next("/sync/pull");
        let mut app = app_syncing_in_background(&server.endpoint);
        let path = app.db_path.clone();
        let password = app.unlock_password.clone().unwrap();

        trigger_sync(&mut app);
        held.wait_until_reached();

        let thread = app.sync_job.as_mut().unwrap().thread.take().unwrap();
        app.lock();
        assert!(app.sync_job.is_none());

        held.release();
        thread.join().unwrap();

        let db = Database::open(&path, &password).unwrap();
        assert_eq!(NoteRepository::new(db.connection()).list(true, key()).unwrap().len(), 0);
        let metadata = SyncRepository::new(db.connection()).get_metadata().unwrap().unwrap();
        assert_eq!(metadata.last_sync_at, None);
    }

    /// The thread holds a copy of the key until it ends, so it can't go on fetching.
    #[test]
    fn a_locked_vault_stops_a_sync_fetching_files() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_syncing_in_background(&server.endpoint);

        let db = app.db.as_ref().unwrap();
        let mut note = Note::new("Names two files it hasn't got".to_string());
        for id in [note_id(1), note_id(2)] {
            note.attachments.push(Attachment {
                id: id.clone(),
                filename: "a.txt".to_string(),
                mime_type: "text/plain".to_string(),
                size: 1,
                data: id,
                thumbnail_data: None,
            });
        }
        NoteRepository::new(db.connection()).create(&note, key()).unwrap();

        let held = server.hold_next(Box::leak(note_id(1).into_boxed_str()));
        trigger_sync(&mut app);
        held.wait_until_reached();

        let thread = app.sync_job.as_mut().unwrap().thread.take().unwrap();
        app.lock();
        held.release();
        thread.join().unwrap();

        assert_eq!(server.blob_requests(), 1);
    }

    /// The sync read these before it started, so writing them back would undo the change.
    #[test]
    fn what_changes_during_a_sync_survives_the_end_of_it() {
        let server = StubServer::start(vec![], vec![]);
        let held = server.hold_next("/sync/pull");
        let mut app = app_syncing_in_background(&server.endpoint);

        trigger_sync(&mut app);
        held.wait_until_reached();

        let mut metadata = sync_metadata(&app);
        metadata.server_key_stale = true;
        metadata.user_email = Some("changed@example.com".to_string());
        SyncRepository::new(app.db.as_ref().unwrap().connection()).update_metadata(&metadata).unwrap();

        held.release();
        finish_background_sync_now(&mut app);

        let metadata = sync_metadata(&app);
        assert!(metadata.server_key_stale);
        assert_eq!(metadata.user_email.as_deref(), Some("changed@example.com"));
        assert!(metadata.last_sync_at.is_some());
    }

    #[test]
    fn a_sync_that_fails_in_the_background_says_why() {
        let server = StubServer::start(vec![], vec![]);
        server.fail_next("500 Internal Server Error");
        let mut app = app_syncing_in_background(&server.endpoint);
        NoteRepository::new(app.db.as_ref().unwrap().connection())
            .create(&Note::new("Stays pending".to_string()), key())
            .unwrap();

        trigger_sync(&mut app);
        finish_background_sync_now(&mut app);

        assert!(app.error.as_deref().is_some_and(|error| error.contains("500")));
        assert!(app.sync_job.is_none());
    }

    /// What was missing a blob when it arrived gets it from a later sync, without anything
    /// on the screen waiting for the download.
    #[test]
    fn a_blob_this_device_only_has_a_reference_to_is_fetched() {
        use base64::Engine as _;
        let crypto = CryptoService::new();
        let payload = serde_json::to_vec(&crypto.encrypt_binary(b"abc", key()).unwrap()).unwrap();
        let server = StubServer::with_attachments(vec![], vec![json!({
            "id": ATTACHMENT_ID,
            "data": base64::engine::general_purpose::STANDARD.encode(payload),
        })]);
        let mut app = app_connected_to(&server.endpoint);
        add_note_with_attachment(&app, "reference-only.txt");
        let db = app.db.as_ref().unwrap();
        AttachmentRepository::new(db.connection()).delete(ATTACHMENT_ID).unwrap();

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let (_, _, _, data) = AttachmentRepository::new(db.connection()).get(ATTACHMENT_ID, key()).unwrap().unwrap();
        assert_eq!(data, b"abc");
    }

    fn wire_saved_search(id: usize, name: &str, query: &str) -> Value {
        json!({
            "id": note_id(id),
            "name": encrypted(name),
            "query": encrypted(query),
            "order": id,
            "createdAt": "2026-01-01T00:00:00Z",
            "modifiedAt": "2026-01-01T00:00:00Z",
            "deleted": false,
            "version": 1,
        })
    }

    fn saved_search_names(app: &App) -> Vec<String> {
        let db = app.db.as_ref().unwrap();
        SavedSearchRepository::new(db.connection()).list(key()).unwrap()
            .into_iter().map(|search| search.name).collect()
    }

    #[test]
    fn saved_searches_arrive_from_the_server() {
        let mut page = cursor_page(vec![], "c1", false);
        page["savedSearches"] = json!([wire_saved_search(1, "Work", "#work -done")]);
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![page]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(saved_search_names(&app), ["Work"]);
    }

    #[test]
    fn a_saved_search_this_vault_cannot_open_is_left_out_and_counted() {
        let crypto = CryptoService::new();
        let foreign = serde_json::to_string(&crypto.encrypt_text("Planted", &crypto.generate_master_key()).unwrap()).unwrap();
        let mut planted = wire_saved_search(2, "x", "y");
        planted["name"] = json!(foreign);
        planted["query"] = json!("not even ciphertext");

        let mut page = cursor_page(vec![], "c1", false);
        page["savedSearches"] = json!([planted, wire_saved_search(1, "Work", "#work")]);
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![page]);
        let mut app = app_connected_to(&server.endpoint);

        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.skipped, 1);
        assert_eq!(saved_search_names(&app), ["Work"]);
    }

    #[test]
    fn a_saved_search_deleted_elsewhere_goes_here_whatever_the_row_holds() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let repo = SavedSearchRepository::new(db.connection());
        repo.apply(
            &serde_json::from_value(wire_saved_search(1, "Going", "x")).unwrap(),
            false,
            None,
        ).unwrap();

        let mut gone = wire_saved_search(1, "", "");
        gone["name"] = json!("plaintext from an older client");
        gone["modifiedAt"] = json!("2026-06-01T00:00:00Z");
        gone["deleted"] = json!(true);
        let mut page = cursor_page(vec![], "c1", false);
        page["savedSearches"] = json!([gone]);
        server.script(vec![page]);

        perform_sync(&mut app, false).unwrap();

        assert!(saved_search_names(&app).is_empty());
    }

    #[test]
    fn a_saved_search_goes_up_encrypted_and_then_stops_going() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        SavedSearchRepository::new(db.connection()).create("Private name", "#private", key()).unwrap();

        perform_sync(&mut app, false).unwrap();

        let pushed = server.all_pushes().into_iter().find(|push| push.get("savedSearches").is_some()).unwrap();
        let sent = pushed["savedSearches"][0].clone();
        assert!(!sent["name"].as_str().unwrap().contains("Private"));
        assert!(!sent["query"].as_str().unwrap().contains("private"));

        let pushes = server.all_pushes().len();
        perform_sync(&mut app, false).unwrap();
        assert_eq!(server.all_pushes().len(), pushes);
    }

    /// An older server sends them whole on every page, so they're read once.
    #[test]
    fn saved_searches_from_an_older_server_are_read_from_the_last_page() {
        let server = StubServer::start(vec![], vec![]);
        let mut first = json!({
            "notes": remote_notes(1), "deletions": [], "attachments": [], "versions": [],
            "savedSearches": [wire_saved_search(1, "Work", "#work")],
            "syncedAt": server_time(0), "hasMore": true,
        });
        let mut last = first.clone();
        last["notes"] = json!([]);
        last["hasMore"] = json!(false);
        first["savedSearches"] = json!([]);
        server.script(vec![first, last]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(saved_search_names(&app), ["Work"]);
    }

    /// A note held for a conflict can be clean: a pull raises one on a note that's open.
    /// Its local side is the only copy, so that has to count as an edit.
    fn note_with_a_pending_conflict(app: &App, needs_sync: bool) -> Note {
        let mut note = Note::new("The only copy of this side".to_string());
        note.id = note_id(0);
        note.needs_sync = needs_sync;
        note.content_hash = Some("local-hash".to_string());
        let db = app.db.as_ref().unwrap();
        let repo = NoteRepository::new(db.connection());
        repo.create(&note, key()).unwrap();
        repo.store(&note, key()).unwrap();

        let earlier = remote_notes(1).remove(0);
        let mut conflict = conflict_from_pull(&note, &serde_json::from_value(earlier).unwrap());
        conflict.ancestor_hash = Some("ancestor-hash".to_string());
        conflict.ancestor_content = Some(encrypted("The ancestor"));
        SyncRepository::new(db.connection()).set_conflict(&note.id, &conflict).unwrap();
        note
    }

    fn newer_copy_of_note_zero() -> Value {
        let mut remote = remote_notes(1).remove(0);
        remote["content"] = json!(encrypted("Moved on again"));
        remote["modifiedAt"] = json!("2026-06-01T00:00:00Z");
        remote["contentHash"] = json!("remote-hash");
        // A chain that holds the local hash would otherwise read as "includes the edit"
        remote["hashChain"] = json!(["remote-hash", "local-hash"]);
        remote
    }

    #[test]
    fn a_pending_conflict_counts_as_an_unsent_edit() {
        for needs_sync in [false, true] {
            let server = StubServer::start(vec![], vec![]);
            let mut app = app_connected_to(&server.endpoint);
            let note = note_with_a_pending_conflict(&app, needs_sync);
            server.script(vec![cursor_page(vec![newer_copy_of_note_zero()], "c1", false)]);

            perform_sync(&mut app, false).unwrap();

            assert_eq!(stored_note(&app, &note.id).content, "The only copy of this side");
        }
    }

    /// The conflict screen should show where the server is now, and a three-way merge
    /// still needs the ancestor that the refused push brought.
    #[test]
    fn a_pending_conflict_follows_the_server_and_keeps_its_ancestor() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = note_with_a_pending_conflict(&app, true);
        server.script(vec![cursor_page(vec![newer_copy_of_note_zero()], "c1", false)]);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let conflict = SyncRepository::new(db.connection()).get_note_sync(&note.id).unwrap().unwrap().conflict_data.unwrap();
        assert_eq!(conflict.server_content_hash.as_deref(), Some("remote-hash"));
        assert_eq!(conflict.ancestor_hash.as_deref(), Some("ancestor-hash"));
        assert!(conflict.ancestor_content.is_some());
    }

    /// A compromised server could send every note deleted decades ago.
    #[test]
    fn a_note_pulled_into_the_bin_starts_its_time_there_now() {
        let mut gone = remote_notes(1).remove(0);
        gone["deleted"] = json!(true);
        gone["deletedAt"] = json!("2001-01-01T00:00:00Z");
        let server = StubServer::start(vec![gone], vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let repo = NoteRepository::new(db.connection());
        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        repo.purge_expired_bin(thirty_days_ago).unwrap();
        repo.purge_remote_deletions(thirty_days_ago).unwrap();

        assert!(repo.get(&note_id(0), key()).unwrap().is_some());
    }

    /// A pending note, as the server's skip list would name it.
    fn pending_note(app: &App, text: &str) -> Note {
        let note = Note::new(text.to_string());
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();
        note
    }

    fn stored_note(app: &App, id: &str) -> Note {
        NoteRepository::new(app.db.as_ref().unwrap().connection()).get(id, key()).unwrap().unwrap()
    }

    #[test]
    fn a_note_deleted_for_good_stops_being_sent_and_goes_to_the_bin() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = pending_note(&app, "Deleted elsewhere after this edit");
        server.pass_over(&note.id);
        server.add_to_push_replies(json!({ "skippedDeleted": [note.id] }));

        perform_sync(&mut app, false).unwrap();

        let stored = stored_note(&app, &note.id);
        assert!(stored.deleted);
        assert!(!stored.needs_sync);

        let pushes = server.all_pushes().len();
        perform_sync(&mut app, false).unwrap();
        assert_eq!(server.all_pushes().len(), pushes);
    }

    /// An edit later than the deletion beats it, so that one has to go.
    #[test]
    fn a_skipped_note_that_changed_since_stays_and_goes_again() {
        let server = StubServer::start(vec![], vec![]);
        let held = server.hold_next("/sync/push");
        let mut app = app_syncing_in_background(&server.endpoint);
        let note = pending_note(&app, "As sent");
        server.pass_over(&note.id);
        server.add_to_push_replies(json!({ "skippedDeleted": [note.id] }));

        trigger_sync(&mut app);
        held.wait_until_reached();

        let mut edited = stored_note(&app, &note.id);
        edited.content = "Edited while the push was out".to_string();
        edited.touch();
        NoteRepository::new(app.db.as_ref().unwrap().connection()).update(&edited, key()).unwrap();

        held.release();
        finish_background_sync_now(&mut app);

        let stored = stored_note(&app, &note.id);
        assert!(!stored.deleted);
        assert!(stored.needs_sync);
    }

    /// The list comes from the server, so it can't reach a note this push didn't send.
    #[test]
    fn a_server_cannot_skip_a_note_it_was_not_sent() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let sent = pending_note(&app, "Sent");
        let mut other = Note::new("Not sent".to_string());
        other.needs_sync = false;
        let db = app.db.as_ref().unwrap();
        let repo = NoteRepository::new(db.connection());
        repo.create(&other, key()).unwrap();
        repo.store(&other, key()).unwrap();
        SyncRepository::new(db.connection()).update_last_sync(Utc::now()).unwrap();
        server.add_to_push_replies(json!({ "skippedDeleted": [other.id] }));

        perform_sync(&mut app, false).unwrap();

        assert!(!stored_note(&app, &other.id).deleted);
        assert!(!stored_note(&app, &sent.id).deleted);
    }

    /// An older server sends no such list, so a note it passes over stays pending.
    #[test]
    fn a_note_an_older_server_says_nothing_about_stays_pending() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = pending_note(&app, "Passed over");
        server.pass_over(&note.id);

        let outcome = perform_sync(&mut app, false).unwrap();

        let stored = stored_note(&app, &note.id);
        assert!(stored.needs_sync);
        assert!(!stored.deleted);
        assert!(!outcome.clock_ahead);
    }

    /// The stub's clock is `server_time`, so a device a few years behind it isn't ahead.
    fn note_dated(app: &App, date: DateTime<Utc>) -> Note {
        let mut note = Note::new("Dated".to_string());
        note.modified_at = date;
        note.created_at = date;
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();
        note
    }

    #[test]
    fn a_note_dated_past_the_servers_clock_says_the_clock_may_be_wrong() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(server.endpoint.clone());
        let note = note_dated(&app, server_time(0) + chrono::Duration::hours(25));
        server.pass_over(&note.id);

        trigger_sync(&mut app);

        assert_eq!(app.error, Some(t!("sync.clock_ahead").to_string()));
        assert!(stored_note(&app, &note.id).needs_sync);
    }

    #[test]
    fn a_note_within_a_day_of_the_servers_clock_is_not_the_clocks_fault() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = note_dated(&app, server_time(0) + chrono::Duration::hours(23));
        server.pass_over(&note.id);

        let outcome = perform_sync(&mut app, false).unwrap();

        assert!(!outcome.clock_ahead);
    }

    /// Dates on a note the server took don't matter, whatever they say.
    #[test]
    fn an_accepted_note_says_nothing_about_the_clock() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        note_dated(&app, server_time(0) + chrono::Duration::days(30));

        let outcome = perform_sync(&mut app, false).unwrap();

        assert!(!outcome.clock_ahead);
    }

    #[test]
    fn a_saved_search_dated_past_the_servers_clock_stays_pending() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let repo = SavedSearchRepository::new(db.connection());
        let mut row: SyncSavedSearch = serde_json::from_value(wire_saved_search(1, "Ahead", "x")).unwrap();
        row.modified_at = server_time(0) + chrono::Duration::hours(25);
        repo.apply(&row, true, None).unwrap();

        let outcome = perform_sync(&mut app, false).unwrap();

        assert!(outcome.clock_ahead);
        let db = app.db.as_ref().unwrap();
        assert_eq!(SavedSearchRepository::new(db.connection()).pending().unwrap().len(), 1);
    }

    #[test]
    fn a_repair_fetches_what_the_server_has_and_counts_it() {
        use base64::Engine as _;
        let crypto = CryptoService::new();
        let payload = serde_json::to_vec(&crypto.encrypt_binary(b"abc", key()).unwrap()).unwrap();
        let server = StubServer::with_attachments(vec![], vec![json!({
            "id": ATTACHMENT_ID,
            "data": base64::engine::general_purpose::STANDARD.encode(payload),
        })]);
        let app = app_connected_to(&server.endpoint);
        add_note_with_attachment(&app, "reference-only.txt");
        let db = app.db.as_ref().unwrap();
        AttachmentRepository::new(db.connection()).delete(ATTACHMENT_ID).unwrap();

        assert_eq!(fetch_all_missing_blobs(&app).unwrap(), 1);
        assert_eq!(fetch_all_missing_blobs(&app).unwrap(), 0);
    }

    /// Without this, a server that always says there's more would keep a sync running.
    #[test]
    fn a_page_that_goes_nowhere_ends_the_pull() {
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![cursor_page(vec![], "c1", true), cursor_page(vec![], "c1", true)]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(cursors_sent(&server), vec![json!(""), json!("c1")]);
    }

    #[test]
    fn versions_and_deletions_are_applied_from_every_cursor_page() {
        let doomed = Note::new("Doomed".to_string());
        let version = json!({
            "versionKey": format!("{}:1", note_id(0)),
            "noteId": note_id(0),
            "version": 1,
            "createdAt": "2020-01-01T00:00:00Z",
            "syncedAt": "2020-01-01T00:00:00Z",
            "content": encrypted("Earlier"),
            "tags": [],
            "attachments": [],
            "reason": "sync",
        });
        let mut first = cursor_page(vec![], "c1", true);
        first["versions"] = json!([version]);
        first["deletions"] = json!([{ "id": doomed.id, "deletedAt": "2031-01-01T00:00:00Z" }]);

        let server = StubServer::start(vec![], vec![]);
        server.script(vec![first, cursor_page(remote_notes(1), "c2", false)]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let mut synced = doomed.clone();
        synced.needs_sync = false;
        NoteRepository::new(db.connection()).create(&synced, key()).unwrap();
        SyncRepository::new(db.connection()).update_last_sync(Utc::now()).unwrap();

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        assert!(NoteRepository::new(db.connection()).get(&doomed.id, key()).unwrap().unwrap().deleted);
        let versions = NoteVersionRepository::new(db.connection());
        assert!(versions.get_version_by_key(&format!("{}:1", note_id(0))).unwrap().is_some());
    }

    /// The cursor can't pass a version that's only held in memory, or a crash here
    /// would lose it for good.
    #[test]
    fn the_cursor_waits_for_a_version_whose_note_is_on_a_later_page() {
        let version = json!({
            "versionKey": format!("{}:1", note_id(0)),
            "noteId": note_id(0),
            "version": 1,
            "createdAt": "2020-01-01T00:00:00Z",
            "syncedAt": "2020-01-01T00:00:00Z",
            "content": encrypted("Earlier"),
            "tags": [],
            "attachments": [],
            "reason": "sync",
        });
        let mut first = cursor_page(vec![], "c1", true);
        first["versions"] = json!([version]);

        let server = StubServer::start(vec![], vec![]);
        server.script(vec![first]);
        // The note's page never arrives
        server.script_failure("500 Internal Server Error");
        let mut app = app_connected_to(&server.endpoint);

        assert!(perform_sync(&mut app, false).is_err());

        assert_eq!(sync_metadata(&app).pull_cursor, None);
    }

    #[test]
    fn a_version_whose_note_never_arrives_does_not_stop_the_pull() {
        let version = json!({
            "versionKey": format!("{}:1", note_id(7)),
            "noteId": note_id(7),
            "version": 1,
            "createdAt": "2020-01-01T00:00:00Z",
            "syncedAt": "2020-01-01T00:00:00Z",
            "content": encrypted("Earlier"),
            "tags": [],
            "attachments": [],
            "reason": "sync",
        });
        let mut only = cursor_page(vec![], "c1", false);
        only["versions"] = json!([version]);

        let server = StubServer::start(vec![], vec![]);
        server.script(vec![only]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(sync_metadata(&app).pull_cursor.as_deref(), Some("c1"));
    }

    /// Timestamp pulls can have skipped changes, and nothing can detect that afterwards.
    #[test]
    fn the_first_cursor_after_timestamps_starts_again_from_the_top() {
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![cursor_page(vec![], "c1", false), cursor_page(remote_notes(1), "c9", false)]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let repo = SyncRepository::new(db.connection());
        let mut metadata = repo.get_metadata().unwrap().unwrap();
        metadata.last_sync_at = Some(Utc::now());
        metadata.last_pull_at = Some(server_time(-5));
        repo.update_metadata(&metadata).unwrap();

        perform_sync(&mut app, false).unwrap();

        let pulls = server.all_pulls();
        assert_eq!(pulls.len(), 2);
        assert_eq!(pulls[0]["cursor"], json!(""));
        assert!(pulls[0]["lastSyncAt"].is_string());
        assert_eq!(pulls[1]["cursor"], json!(""));
        assert!(pulls[1]["lastSyncAt"].is_null());
        assert_eq!(sync_metadata(&app).pull_cursor.as_deref(), Some("c9"));
        assert_eq!(stored_note_count(&app), 1);
    }

    #[test]
    fn force_sync_starts_from_nothing_whatever_is_stored() {
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![cursor_page(vec![], "c5", false)]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let repo = SyncRepository::new(db.connection());
        let mut metadata = repo.get_metadata().unwrap().unwrap();
        metadata.pull_cursor = Some("c4".to_string());
        metadata.last_pull_at = Some(server_time(-5));
        metadata.last_sync_at = Some(Utc::now());
        repo.update_metadata(&metadata).unwrap();

        perform_sync(&mut app, true).unwrap();

        let pulls = server.all_pulls();
        assert_eq!(pulls[0]["cursor"], json!(""));
        assert!(pulls[0]["lastSyncAt"].is_null());
    }

    #[test]
    fn known_attachments_come_from_the_blobs_this_device_holds() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        add_note_with_attachment(&app, "plain.txt");

        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.all_pulls()[0]["knownAttachmentIds"], json!([ATTACHMENT_ID]));
    }

    #[test]
    fn a_refused_deletion_stops_being_sent() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let note = Note::new("Edited elsewhere".to_string());
        note_repo.create(&note, key()).unwrap();
        note_repo.hard_delete(&note.id).unwrap();
        server.add_to_push_replies(json!({ "rejectedDeletions": [note.id] }));

        perform_sync(&mut app, false).unwrap();
        assert_eq!(server.last_push()["deletions"][0]["id"], note.id);

        let db = app.db.as_ref().unwrap();
        assert!(NoteRepository::new(db.connection()).get_pending_deletions().unwrap().is_empty());

        let pushes_so_far = server.all_pushes().len();
        perform_sync(&mut app, false).unwrap();
        assert_eq!(server.all_pushes().len(), pushes_so_far);
    }

    #[test]
    fn a_response_without_rejected_deletions_means_none_were() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let note = Note::new("Gone".to_string());
        note_repo.create(&note, key()).unwrap();
        note_repo.hard_delete(&note.id).unwrap();

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        assert!(NoteRepository::new(db.connection()).get_pending_deletions().unwrap().is_empty());
    }

    #[test]
    fn a_push_only_claims_lists_this_device_can_vouch_for() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());

        let written_here = Note::new("Written here".to_string());
        note_repo.create(&written_here, key()).unwrap();

        // What the migration leaves an existing note as
        let mut from_before = Note::new("From an older build".to_string());
        from_before.attachments_complete = false;
        note_repo.create(&from_before, key()).unwrap();

        perform_sync(&mut app, false).unwrap();

        let pushes = server.all_pushes();
        assert_eq!(pushes.len(), 2);
        assert_eq!(pushes[0]["attachmentsAuthoritative"], json!(true));
        assert_eq!(pushes[0]["notes"][0]["id"], json!(written_here.id));
        assert!(pushes[1]["attachmentsAuthoritative"].is_null());
        assert_eq!(pushes[1]["notes"][0]["id"], json!(from_before.id));
    }

    #[test]
    fn a_list_taken_from_the_server_can_be_vouched_for() {
        let mut notes = remote_notes(1);
        notes[0]["contentHash"] = json!("server-hash");
        let server = StubServer::start(notes, vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let mut stale = Note::new("Stale".to_string());
        stale.id = note_id(0);
        stale.needs_sync = false;
        stale.attachments_complete = false;
        NoteRepository::new(db.connection()).create(&stale, key()).unwrap();
        SyncRepository::new(db.connection()).update_last_sync(Utc::now()).unwrap();

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let note = NoteRepository::new(db.connection()).get(&note_id(0), key()).unwrap().unwrap();
        assert!(note.attachments_complete);
    }

    /// The server leaves out blobs this device has, and a reference with no blob here
    /// still has to stay on the list, or a later push would tell the server it's gone.
    #[test]
    fn a_reference_with_no_blob_stays_on_the_list() {
        let mut notes = remote_notes(1);
        notes[0]["attachments"] = json!([{
            "id": ATTACHMENT_ID, "filename": encrypted("kept.txt"), "mimeType": "text/plain", "size": 3, "data": ATTACHMENT_ID,
        }]);
        let server = StubServer::start(notes, vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).get(&note_id(0), key()).unwrap().unwrap();
        assert_eq!(stored.attachments.len(), 1);
        assert!(stored.attachments_complete);
    }

    #[test]
    fn a_list_with_a_reference_that_cannot_be_read_is_not_vouched_for() {
        let crypto = CryptoService::new();
        let other_key = crypto.generate_master_key();
        let unreadable = serde_json::to_string(&crypto.encrypt_text("secret.txt", &other_key).unwrap()).unwrap();
        let mut notes = remote_notes(1);
        notes[0]["attachments"] = json!([{
            "id": ATTACHMENT_ID, "filename": unreadable, "mimeType": "text/plain", "size": 3, "data": ATTACHMENT_ID,
        }]);
        let server = StubServer::start(notes, vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).get(&note_id(0), key()).unwrap().unwrap();
        assert!(stored.attachments.is_empty());
        assert!(!stored.attachments_complete);
    }

    /// A local note and a pulled note with these hashes.
    fn pair(local: (&str, Option<&str>, &[&str], bool), remote: (&str, &[&str])) -> (Note, SyncNote) {
        let (hash, parent, chain, pending) = local;
        let mut note = Note::new("Local".to_string());
        note.content_hash = Some(hash.to_string());
        note.parent_hash = parent.map(String::from);
        note.hash_chain = Some(chain.iter().map(|h| h.to_string()).collect());
        note.needs_sync = pending;

        let (remote_hash, remote_chain) = remote;
        let mut pulled: SyncNote = serde_json::from_value(remote_notes(1).remove(0)).unwrap();
        pulled.content_hash = Some(remote_hash.to_string());
        pulled.hash_chain = Some(remote_chain.iter().map(|h| h.to_string()).collect());
        (note, pulled)
    }

    #[test]
    fn the_same_note_is_left_alone_whether_or_not_an_edit_is_pending() {
        let (local, remote) = pair(("a", None, &["a"], true), ("a", &["a"]));
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Ignore);

        let (local, remote) = pair(("a", None, &["a"], false), ("a", &["a"]));
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Ignore);
    }

    #[test]
    fn a_copy_older_than_the_local_edit_keeps_the_edit() {
        // The local parent
        let (local, remote) = pair(("b", Some("a"), &["b", "a"], true), ("a", &["a"]));
        assert_eq!(decide_pull(&local, &remote, false), PullAction::KeepLocal);

        // Further back along the local chain
        let (local, remote) = pair(("c", Some("b"), &["c", "b", "a"], true), ("a", &["a"]));
        assert_eq!(decide_pull(&local, &remote, false), PullAction::KeepLocal);
    }

    #[test]
    fn a_copy_that_includes_the_local_edit_is_taken() {
        let (local, remote) = pair(("b", Some("a"), &["b", "a"], true), ("c", &["c", "b", "a"]));
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Take);
    }

    /// Sharing an ancestor doesn't make the pulled note newer. Every note's chain starts
    /// with its own hash, so this is what two edits from one parent look like.
    #[test]
    fn two_edits_from_one_parent_are_a_conflict() {
        let (local, remote) = pair(("b", Some("a"), &["b", "a"], true), ("c", &["c", "a"]));
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Conflict);
    }

    #[test]
    fn with_no_edit_pending_a_different_note_is_taken() {
        let (local, remote) = pair(("a", None, &["a"], false), ("b", &["b"]));
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Take);
    }

    /// Not every client's hash covers these, so an equal hash doesn't settle it.
    #[test]
    fn a_state_change_with_an_equal_hash_still_arrives() {
        let (local, mut remote) = pair(("a", None, &["a"], false), ("a", &["a"]));
        remote.deleted = true;
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Take);

        let (local, mut remote) = pair(("a", None, &["a"], false), ("a", &["a"]));
        remote.pinned = true;
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Take);

        // A pending edit pushes its own state instead
        let (local, mut remote) = pair(("a", None, &["a"], true), ("a", &["a"]));
        remote.deleted = true;
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Ignore);
    }

    #[test]
    fn without_hashes_the_times_protect_a_pending_edit() {
        let (mut local, mut remote) = pair(("a", None, &[], true), ("b", &[]));
        local.content_hash = None;
        remote.content_hash = None;
        remote.modified_at = local.modified_at - chrono::Duration::days(1);
        assert_eq!(decide_pull(&local, &remote, false), PullAction::KeepLocal);

        remote.modified_at = local.modified_at + chrono::Duration::days(1);
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Conflict);

        local.needs_sync = false;
        assert_eq!(decide_pull(&local, &remote, false), PullAction::Take);
    }

    #[test]
    fn force_sync_does_not_overwrite_a_pending_edit() {
        let (local, remote) = pair(("b", Some("a"), &["b", "a"], true), ("a", &["a"]));
        assert_eq!(decide_pull(&local, &remote, true), PullAction::KeepLocal);
    }

    /// A note this device has already seen, as a server would resend it.
    fn resent(note: &Note, text: &str) -> Value {
        let mut value = remote_notes(1).remove(0);
        value["id"] = json!(note.id);
        value["content"] = json!(encrypted(text));
        value["modifiedAt"] = json!((note.modified_at + chrono::Duration::days(1)).to_rfc3339());
        value["contentHash"] = json!("server-hash");
        value["hashChain"] = json!(["server-hash"]);
        value
    }

    /// The push is refused and the pull in the same sync delivers the server's copy.
    /// That used to replace the local edit, so the conflict screen compared two identical
    /// notes and the edit was gone.
    #[test]
    fn a_refused_note_is_not_overwritten_by_the_pull_after_it() {
        let note = Note::new("My edit".to_string());
        let server = StubServer::start(vec![resent(&note, "The server's text")], vec![]);
        server.refuse(&note.id);
        let mut app = app_connected_to(&server.endpoint);
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();

        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.rejected, 1);
        let db = app.db.as_ref().unwrap();
        let kept = NoteRepository::new(db.connection()).get(&note.id, key()).unwrap().unwrap();
        assert_eq!(kept.content, "My edit");
        assert!(kept.needs_sync);

        open_conflict_resolution(&mut app, &note.id).unwrap();
        assert_eq!(app.conflict_local_content, "My edit");
        assert_eq!(app.conflict_server_content, "The server's text");
    }

    #[test]
    fn a_note_in_conflict_is_not_pushed_until_it_is_resolved() {
        let note = Note::new("My edit".to_string());
        let server = StubServer::start(vec![], vec![]);
        server.refuse(&note.id);
        let mut app = app_connected_to(&server.endpoint);
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();

        perform_sync(&mut app, false).unwrap();
        let pushes = server.all_pushes().len();
        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.all_pushes().len(), pushes);
    }

    #[test]
    fn a_pull_finds_a_conflict_a_push_never_tried() {
        let note = Note::new("My edit".to_string());
        let server = StubServer::start(vec![resent(&note, "The server's text")], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let mut local = note.clone();
        local.content_hash = Some("local-hash".to_string());
        local.hash_chain = Some(vec!["local-hash".to_string()]);
        NoteRepository::new(db.connection()).create(&local, key()).unwrap();

        // The note goes up after the pull has started, in the sense that it's still
        // pending when the server's copy lands: this server accepts nothing
        server.add_to_push_replies(json!({ "accepted": [] }));
        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.rejected, 1);
        let db = app.db.as_ref().unwrap();
        assert_eq!(NoteRepository::new(db.connection()).get(&note.id, key()).unwrap().unwrap().content, "My edit");
        assert_eq!(SyncRepository::new(db.connection()).get_conflict_note_ids().unwrap(), vec![note.id.clone()]);
    }

    #[test]
    fn pulling_the_same_note_twice_changes_nothing_the_second_time() {
        let server = StubServer::start(vec![], vec![]);
        let mut notes = remote_notes(1);
        notes[0]["contentHash"] = json!("server-hash");
        notes[0]["hashChain"] = json!(["server-hash"]);
        server.script(vec![cursor_page(notes.clone(), "c1", false), cursor_page(notes, "c1", false)]);
        let mut app = app_connected_to(&server.endpoint);

        let first = perform_sync(&mut app, false).unwrap();
        let second = perform_sync(&mut app, false).unwrap();

        assert_eq!(first.synced, 1);
        assert_eq!(second.synced, 0);
        let db = app.db.as_ref().unwrap();
        assert_eq!(NoteVersionRepository::new(db.connection()).count_versions_for_note(&note_id(0)).unwrap(), 0);
    }

    #[test]
    fn a_pulled_note_keeps_its_hashes_and_show_preview_through_an_edit() {
        let mut notes = remote_notes(1);
        notes[0]["contentHash"] = json!("server-hash");
        notes[0]["parentHash"] = json!("older-hash");
        notes[0]["hashChain"] = json!(["server-hash", "older-hash"]);
        notes[0]["showPreview"] = json!(true);
        let server = StubServer::start(notes, vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let mut note = note_repo.get(&note_id(0), key()).unwrap().unwrap();
        assert_eq!(note.content_hash.as_deref(), Some("server-hash"));
        assert_eq!(note.show_preview, Some(true));

        note.content = "Edited here".to_string();
        note.touch();
        note_repo.update(&note, key()).unwrap();
        perform_sync(&mut app, false).unwrap();

        let pushed = &server.last_push()["notes"][0];
        assert_eq!(pushed["showPreview"], json!(true));
        assert_eq!(pushed["parentHash"], json!("server-hash"));
        assert_eq!(pushed["hashChain"][1], json!("server-hash"));
        assert_eq!(pushed["hashChain"][2], json!("older-hash"));
        assert_ne!(pushed["contentHash"], json!("server-hash"));
    }

    #[test]
    fn a_pushed_hash_gives_the_server_nothing_to_guess_at() {
        use sha2::{Digest, Sha256};
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = Note::new("My PIN is 1234".to_string());
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();

        perform_sync(&mut app, false).unwrap();

        let pushed = server.last_push()["notes"][0]["contentHash"].as_str().unwrap().to_string();
        let unkeyed = {
            let mut hasher = Sha256::new();
            hasher.update(note.content.as_bytes());
            general_purpose::STANDARD.encode(hasher.finalize())
        };
        assert_ne!(pushed, unkeyed);
        assert_ne!(pushed, CryptoService::new().compute_content_hash(&note, &[0u8; 32]));
    }

    #[test]
    fn a_note_without_a_change_is_not_pushed_again() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = Note::new("Once".to_string());
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();

        perform_sync(&mut app, false).unwrap();
        let pushes = server.all_pushes().len();
        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.all_pushes().len(), pushes);
        let db = app.db.as_ref().unwrap();
        assert!(!NoteRepository::new(db.connection()).get(&note.id, key()).unwrap().unwrap().needs_sync);
    }

    /// Modified_at used to pick what to push, so a change saved with an older timestamp
    /// never went. Keep Both was one such change.
    #[test]
    fn a_change_with_an_old_timestamp_is_still_pushed() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        perform_sync(&mut app, false).unwrap();

        let mut note = Note::new("Old date".to_string());
        note.modified_at = Utc.with_ymd_and_hms(2001, 1, 1, 0, 0, 0).unwrap();
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();

        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.last_push()["notes"][0]["id"], json!(note.id));
    }

    #[test]
    fn a_blob_goes_up_once() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = add_note_with_attachment(&app, "plain.txt");

        perform_sync(&mut app, false).unwrap();
        assert_eq!(server.last_push()["attachments"].as_array().unwrap().len(), 1);

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let mut edited = note_repo.get(&note.id, key()).unwrap().unwrap();
        edited.content = "Edited".to_string();
        edited.touch();
        note_repo.update(&edited, key()).unwrap();
        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.last_push()["attachments"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn a_missing_blob_the_server_warns_about_goes_again() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = add_note_with_attachment(&app, "plain.txt");
        server.add_to_push_replies(json!({
            "attachmentWarnings": [{ "noteId": note.id, "attachmentIds": [ATTACHMENT_ID] }],
        }));

        perform_sync(&mut app, false).unwrap();

        let repair = server.last_push();
        assert_eq!(repair["notes"], json!([]));
        assert_eq!(repair["attachments"][0]["id"], json!(ATTACHMENT_ID));
    }

    #[test]
    fn a_version_goes_up_once() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let note = add_note_with_attachment(&app, "plain.txt");
        let db = app.db.as_ref().unwrap();
        NoteVersionRepository::new(db.connection())
            .create_version(&note, Utc::now(), VersionReason::Sync, key())
            .unwrap();

        perform_sync(&mut app, false).unwrap();
        assert_eq!(server.last_push()["versions"].as_array().unwrap().len(), 1);

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let mut edited = note_repo.get(&note.id, key()).unwrap().unwrap();
        edited.content = "Edited".to_string();
        edited.touch();
        note_repo.update(&edited, key()).unwrap();
        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.last_push()["versions"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn a_large_vault_is_pushed_in_batches() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        for i in 0..250 {
            note_repo.create(&Note::new(format!("Note {i}")), key()).unwrap();
        }

        perform_sync(&mut app, false).unwrap();

        let sizes: Vec<usize> = server.all_pushes().iter()
            .map(|push| push["notes"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, vec![100, 100, 50]);
    }

    #[test]
    fn a_failed_batch_leaves_later_notes_pending() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        for i in 0..101 {
            note_repo.create(&Note::new(format!("Note {i}")), key()).unwrap();
        }
        server.fail_next("500 Internal Server Error");

        assert!(perform_sync(&mut app, false).is_err());

        let db = app.db.as_ref().unwrap();
        assert_eq!(NoteRepository::new(db.connection()).get_pending(key()).unwrap().notes.len(), 101);
    }

    /// Writers differ: web and Android push all of a version's tags in one ciphertext.
    #[test]
    fn a_version_keeps_its_tags_in_either_wire_format() {
        let as_one = encrypted(r#"["alpha","beta"]"#);
        let each_alone = [encrypted(r#""gamma""#), encrypted(r#""delta""#)];
        let version = |number: i32, tags: Value| json!({
            "versionKey": format!("{}:{number}", note_id(0)),
            "noteId": note_id(0),
            "version": number,
            "createdAt": "2020-01-01T00:00:00Z",
            "syncedAt": "2020-01-01T00:00:00Z",
            "content": encrypted("Earlier"),
            "tags": tags,
            "attachments": [],
            "reason": "sync",
        });
        let server = StubServer::start(remote_notes(1), vec![version(1, json!([as_one])), version(2, json!(each_alone))]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let versions = NoteVersionRepository::new(db.connection());
        assert_eq!(versions.get_version(&note_id(0), 1, key()).unwrap().unwrap().tags, vec!["alpha", "beta"]);
        assert_eq!(versions.get_version(&note_id(0), 2, key()).unwrap().unwrap().tags, vec!["gamma", "delta"]);
    }

    /// A deletion that lost: the local edit is newer, so it stays and comes back.
    #[test]
    fn an_edit_after_a_deletion_survives_it() {
        let note = Note::new("Edited after".to_string());
        let mut page = cursor_page(vec![], "c1", false);
        page["deletions"] = json!([{
            "id": note.id,
            "deletedAt": (note.modified_at - chrono::Duration::days(1)).to_rfc3339(),
        }]);
        let server = StubServer::start(vec![], vec![]);
        server.script(vec![page]);
        server.add_to_push_replies(json!({ "accepted": [] }));
        let mut app = app_connected_to(&server.endpoint);
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(&note, key()).unwrap();

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        assert!(NoteRepository::new(db.connection()).get(&note.id, key()).unwrap().is_some());
    }

    #[test]
    fn being_told_to_wait_holds_off_automatic_syncs() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(server.endpoint.clone());
        app.settings.auto_sync_interval_minutes = 1;
        server.fail_next("429 Too Many Requests\r\nRetry-After: 900");

        // Nothing to push, so the pull meets the 429
        trigger_sync(&mut app);

        let until = app.sync_paused_until.expect("should be told to wait");
        let wait = until.saturating_duration_since(Instant::now());
        assert!(wait > Duration::from_secs(800) && wait <= Duration::from_secs(900), "{wait:?}");

        app.last_auto_sync = None;
        let requests_before = server.all_pulls().len();
        app.check_auto_sync();
        assert_eq!(server.all_pulls().len(), requests_before);
    }

    #[test]
    fn a_server_cannot_hold_this_device_off_for_ever() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(server.endpoint.clone());
        server.fail_next("429 Too Many Requests\r\nRetry-After: 999999999");

        trigger_sync(&mut app);

        let wait = app.sync_paused_until.unwrap().saturating_duration_since(Instant::now());
        assert!(wait <= Duration::from_secs(MAX_RETRY_AFTER_SECONDS));
    }

    /// 403 means the account is disabled, not that the device is gone. Reactivating the
    /// account brings the key back, so it has to survive.
    #[test]
    fn a_disabled_account_keeps_its_credentials_and_stops_syncing() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(server.endpoint.clone());
        server.fail_next("403 Forbidden");

        trigger_sync(&mut app);

        assert!(app.error.as_ref().unwrap().contains(t!("sync.account_deactivated").as_ref()));
        assert!(sync_metadata(&app).api_key.is_some());
        assert!(app.sync_paused_until.is_some());
    }

    #[test]
    fn a_sync_delivers_a_rewrap_it_still_owes() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (uuid::Uuid::new_v4().to_string(), uuid::Uuid::new_v4().to_string());
        let user_id = "b3c1f5a2-8f0e-4c55-9d52-2f6a1c0e7a11";
        let wrapping_key = crypto.derive_wrapping_key(&old_password, user_id).unwrap();
        let old_blob = serde_json::to_string(&crypto.wrap_master_key(&wrapping_key, key()).unwrap()).unwrap();

        let server = StubServer::start(vec![], vec![]);
        *server.wrapped_key.lock().unwrap() = Some(old_blob.clone());
        let mut app = app_connected_to(&server.endpoint);
        let db = app.db.as_ref().unwrap();
        let repo = SyncRepository::new(db.connection());
        let mut metadata = repo.get_metadata().unwrap().unwrap();
        metadata.user_id = Some(user_id.to_string());
        metadata.last_sync_at = Some(Utc::now());
        metadata.server_key_seen = Some(crypto.hash(&old_blob));
        metadata.server_key_stale = true;
        repo.update_metadata(&metadata).unwrap();
        app.unlock_password = Some(zeroize::Zeroizing::new(new_password.clone()));

        perform_sync(&mut app, false).unwrap();

        let metadata = sync_metadata(&app);
        assert!(!metadata.server_key_stale);
        let blob = server.wrapped_key.lock().unwrap().clone().unwrap();
        assert_ne!(blob, old_blob);
        let wrapped: EncryptedData = serde_json::from_str(&blob).unwrap();
        let wrapping_key = crypto.derive_wrapping_key(&new_password, user_id).unwrap();
        assert_eq!(&crypto.unwrap_master_key(&wrapping_key, &wrapped).unwrap(), key());
    }

    /// A conflict set up the way a refused push leaves one, and opened for resolving.
    fn app_resolving_a_conflict(server: &StubServer, note: &Note) -> App {
        server.refuse(&note.id);
        let mut app = app_connected_to(&server.endpoint);
        NoteRepository::new(app.db.as_ref().unwrap().connection()).create(note, key()).unwrap();
        perform_sync(&mut app, false).unwrap();
        server.refuse.lock().unwrap().clear();
        open_conflict_resolution(&mut app, &note.id).unwrap();
        app
    }

    #[test]
    fn keeping_mine_pushes_the_edit_on_top_of_the_server_version() {
        let note = Note::new("My edit".to_string());
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_resolving_a_conflict(&server, &note);

        resolve_keep_local(&mut app).unwrap();
        perform_sync(&mut app, false).unwrap();

        let pushed = &server.last_push()["notes"][0];
        assert_eq!(pushed["id"], json!(note.id));
        assert_eq!(pushed["parentHash"], json!("server-hash"));
        assert_eq!(pushed["hashChain"][1], json!("server-hash"));
        assert_eq!(pushed["hashChain"][2], json!("older-hash"));
    }

    #[test]
    fn keeping_the_servers_copy_takes_its_attachments_and_hashes_and_pushes_nothing() {
        let note = Note::new("My edit".to_string());
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_resolving_a_conflict(&server, &note);
        let conflict = app.conflict_data.as_mut().unwrap();
        conflict.server_attachments = vec![AttachmentRef {
            id: ATTACHMENT_ID.to_string(),
            filename: encrypted("server.txt"),
            mime_type: "text/plain".to_string(),
            size: 3,
            data: ATTACHMENT_ID.to_string(),
        }];

        resolve_keep_server(&mut app).unwrap();

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).get(&note.id, key()).unwrap().unwrap();
        assert_eq!(stored.content, "The server's text");
        assert_eq!(stored.attachments[0].filename, "server.txt");
        assert_eq!(stored.content_hash.as_deref(), Some("server-hash"));
        assert!(!stored.needs_sync);
        assert!(SyncRepository::new(db.connection()).get_conflict_note_ids().unwrap().is_empty());

        let pushes = server.all_pushes().len();
        perform_sync(&mut app, false).unwrap();
        assert_eq!(server.all_pushes().len(), pushes);
    }

    /// Keep Both is for both versions to reach every device. The copy used to take the
    /// server's timestamp, so it never went, and the local edit stayed behind too.
    #[test]
    fn keeping_both_sends_both_versions_to_the_server() {
        let note = Note::new("My edit".to_string());
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_resolving_a_conflict(&server, &note);
        app.conflict_data.as_mut().unwrap().server_attachments = vec![AttachmentRef {
            id: ATTACHMENT_ID.to_string(),
            filename: encrypted("server.txt"),
            mime_type: "text/plain".to_string(),
            size: 3,
            data: ATTACHMENT_ID.to_string(),
        }];

        let copy_id = resolve_keep_both(&mut app).unwrap();
        perform_sync(&mut app, false).unwrap();

        let pushed = server.last_push();
        let pushed_ids: Vec<&str> = pushed["notes"].as_array().unwrap().iter()
            .map(|note| note["id"].as_str().unwrap())
            .collect();
        assert!(pushed_ids.contains(&note.id.as_str()));
        assert!(pushed_ids.contains(&copy_id.as_str()));

        let copy = pushed["notes"].as_array().unwrap().iter().find(|note| note["id"] == json!(copy_id)).unwrap();
        assert_eq!(copy["attachments"][0]["id"], json!(ATTACHMENT_ID));

        let original = pushed["notes"].as_array().unwrap().iter().find(|n| n["id"] == json!(note.id)).unwrap();
        assert_eq!(original["parentHash"], json!("server-hash"));

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        assert_eq!(note_repo.get(&copy_id, key()).unwrap().unwrap().content, "The server's text");
        assert_eq!(note_repo.get(&note.id, key()).unwrap().unwrap().content, "My edit");
    }

    #[test]
    fn a_timed_out_request_is_tried_again() {
        let server = StubServer::start(remote_notes(1), vec![]);
        server.fail_next("408 Request Timeout");
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(stored_note_count(&app), 1);
    }

    #[test]
    fn a_busy_server_holds_off_automatic_syncs_for_as_long_as_it_asks() {
        let server = StubServer::start(vec![], vec![]);
        let mut app = app_connected_to(&server.endpoint);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(server.endpoint.clone());
        server.fail_next("503 Service Unavailable\r\nRetry-After: 300");

        trigger_sync(&mut app);

        let wait = app.sync_paused_until.expect("should be told to wait").saturating_duration_since(Instant::now());
        assert!(wait > Duration::from_secs(250) && wait <= Duration::from_secs(300), "{wait:?}");
        assert!(app.error.as_ref().unwrap().contains(t!("sync.server_busy", seconds = 300).as_ref()));
    }

    /// A full account can still delete, which is how the user makes room, and it can
    /// still hear about everything other devices did.
    #[test]
    fn a_full_account_still_deletes_and_pulls_and_says_so() {
        let server = StubServer::start(remote_notes(1), vec![]);
        *server.quota_full.lock().unwrap() = true;
        let mut app = app_connected_to(&server.endpoint);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(server.endpoint.clone());

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let binned = Note::new("Binned".to_string());
        note_repo.create(&binned, key()).unwrap();
        note_repo.hard_delete(&binned.id).unwrap();
        let waiting = Note::new("Waiting for room".to_string());
        note_repo.create(&waiting, key()).unwrap();

        trigger_sync(&mut app);

        assert_eq!(server.all_pushes()[0]["deletions"][0]["id"], json!(binned.id));
        assert_eq!(stored_note_count(&app), 2);
        assert_eq!(app.error, Some(t!("sync.quota_exceeded").to_string()));

        let db = app.db.as_ref().unwrap();
        assert!(NoteRepository::new(db.connection()).get(&waiting.id, key()).unwrap().unwrap().needs_sync);
        assert!(NoteRepository::new(db.connection()).get_pending_deletions().unwrap().is_empty());
    }

    /// `count` notes this device has, all synced, and a server listing them as deleted.
    fn app_with_deletions_from_the_server(count: usize) -> (App, StubServer, Vec<Note>) {
        let notes: Vec<Note> = (0..count).map(|i| {
            let mut note = Note::new(format!("Note {i}"));
            note.needs_sync = false;
            note
        }).collect();

        let mut page = cursor_page(vec![], "c1", false);
        page["deletions"] = json!(notes.iter()
            .map(|note| json!({ "id": note.id, "deletedAt": "2031-01-01T00:00:00Z" }))
            .collect::<Vec<_>>());

        let server = StubServer::start(vec![], vec![]);
        server.script(vec![page]);
        let mut app = app_connected_to(&server.endpoint);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(server.endpoint.clone());

        let db = app.db.as_ref().unwrap();
        for note in &notes {
            NoteRepository::new(db.connection()).create(note, key()).unwrap();
        }
        SyncRepository::new(db.connection()).update_last_sync(Utc::now()).unwrap();

        (app, server, notes)
    }

    fn bin_count(app: &App) -> usize {
        let db = app.db.as_ref().unwrap();
        NoteRepository::new(db.connection()).get_deleted(key()).unwrap().len()
    }

    /// Nothing proves a deletion came from one of the user's devices, so a compromised
    /// server must not be able to destroy this device's copy.
    #[test]
    fn a_deletion_from_the_server_only_reaches_the_bin() {
        let (mut app, _server, notes) = app_with_deletions_from_the_server(2);

        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.deletions_held, 0);
        assert_eq!(bin_count(&app), 2);

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).get(&notes[0].id, key()).unwrap().unwrap();
        assert_eq!(stored.content, "Note 0");
        assert!(!stored.needs_sync);
    }

    #[test]
    fn a_note_deleted_elsewhere_can_be_restored_and_goes_back_to_the_server() {
        let (mut app, server, notes) = app_with_deletions_from_the_server(1);
        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let mut note = note_repo.get(&notes[0].id, key()).unwrap().unwrap();
        note.restore();
        note_repo.update(&note, key()).unwrap();
        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.last_push()["notes"][0]["id"], json!(notes[0].id));
        assert_eq!(server.last_push()["notes"][0]["deleted"], json!(false));
    }

    #[test]
    fn a_pull_that_deletes_a_lot_waits_to_be_asked() {
        let (mut app, _server, _notes) = app_with_deletions_from_the_server(MASS_DELETION_THRESHOLD);

        trigger_sync(&mut app);

        assert_eq!(app.held_deletions, MASS_DELETION_THRESHOLD);
        assert_eq!(bin_count(&app), 0);
        assert_eq!(stored_note_count(&app), MASS_DELETION_THRESHOLD);
    }

    #[test]
    fn saying_yes_to_a_large_deletion_moves_the_notes_to_the_bin() {
        let (mut app, _server, _notes) = app_with_deletions_from_the_server(MASS_DELETION_THRESHOLD);
        trigger_sync(&mut app);

        apply_held_deletions(&mut app).unwrap();

        assert_eq!(app.held_deletions, 0);
        assert_eq!(bin_count(&app), MASS_DELETION_THRESHOLD);
    }

    #[test]
    fn saying_no_to_a_large_deletion_keeps_the_notes_and_pushes_them_back() {
        let (mut app, server, notes) = app_with_deletions_from_the_server(MASS_DELETION_THRESHOLD);
        trigger_sync(&mut app);

        refuse_held_deletions(&mut app).unwrap();

        assert_eq!(app.held_deletions, 0);
        assert_eq!(bin_count(&app), 0);
        assert_eq!(server.last_push()["notes"].as_array().unwrap().len(), notes.len());
    }

    /// The question doesn't go away by being ignored.
    #[test]
    fn an_unanswered_deletion_is_asked_again_after_the_next_sync() {
        let (mut app, server, _notes) = app_with_deletions_from_the_server(MASS_DELETION_THRESHOLD);
        trigger_sync(&mut app);

        server.script(vec![cursor_page(vec![], "c1", false)]);
        app.held_deletions = 0;
        trigger_sync(&mut app);

        assert_eq!(app.held_deletions, MASS_DELETION_THRESHOLD);
    }

    #[test]
    fn a_note_deleted_elsewhere_leaves_the_bin_after_the_retention_period() {
        let (mut app, _server, notes) = app_with_deletions_from_the_server(1);
        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        assert_eq!(note_repo.purge_remote_deletions(Utc::now() - chrono::Duration::days(1)).unwrap(), 0);

        assert_eq!(note_repo.purge_remote_deletions(Utc::now() + chrono::Duration::days(1)).unwrap(), 1);
        assert!(note_repo.get(&notes[0].id, key()).unwrap().is_none());
    }

    /// A page naming these notes as deleted, as a server can send as often as it likes.
    fn deletions_page(notes: &[Note], cursor: &str) -> Value {
        let mut page = cursor_page(vec![], cursor, false);
        page["deletions"] = json!(notes.iter()
            .map(|note| json!({ "id": note.id, "deletedAt": "2031-01-01T00:00:00Z" }))
            .collect::<Vec<_>>());
        page
    }

    /// A pulled copy of `note`, newer than the one here, in the bin since long ago.
    fn binned_long_ago(note: &Note) -> Value {
        let mut pulled = remote_notes(1).remove(0);
        pulled["id"] = json!(note.id);
        pulled["modifiedAt"] = json!("2040-01-01T00:00:00Z");
        pulled["deleted"] = json!(true);
        pulled["deletedAt"] = json!("2001-01-01T00:00:00Z");
        pulled
    }

    /// When this device says a note reached the bin from elsewhere.
    fn bin_clock(app: &App, id: &str) -> Option<String> {
        app.db.as_ref().unwrap().connection()
            .query_row("SELECT remote_deleted_at FROM notes WHERE id = ?1", [id], |row| row.get(0))
            .unwrap()
    }

    /// Synced notes on an app whose server has nothing queued for it.
    fn app_with_synced_notes(count: usize) -> (App, StubServer, Vec<Note>) {
        let (mut app, server, _) = app_with_deletions_from_the_server(0);
        perform_sync(&mut app, false).unwrap();

        let notes: Vec<Note> = (0..count).map(|i| {
            let mut note = Note::new(format!("Note {i}"));
            note.needs_sync = false;
            note
        }).collect();
        let db = app.db.as_ref().unwrap();
        for note in &notes {
            NoteRepository::new(db.connection()).create(note, key()).unwrap();
        }

        (app, server, notes)
    }

    /// Web lost notes this way: the first pull could only reach the bin, and a second
    /// naming the same notes found them there and took them out of it.
    #[test]
    fn a_server_repeating_a_deletion_cannot_take_a_note_out_of_the_bin() {
        let (mut app, server, notes) = app_with_deletions_from_the_server(3);
        perform_sync(&mut app, false).unwrap();
        assert_eq!(bin_count(&app), 3);
        let clock = bin_clock(&app, &notes[0].id);
        assert!(clock.is_some());

        for cursor in ["c2", "c3"] {
            server.script(vec![deletions_page(&notes, cursor)]);
            perform_sync(&mut app, false).unwrap();
        }

        assert_eq!(bin_count(&app), 3);
        assert_eq!(bin_clock(&app, &notes[0].id), clock);

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        assert_eq!(note_repo.purge_remote_deletions(thirty_days_ago).unwrap(), 0);
        assert_eq!(note_repo.purge_expired_bin(thirty_days_ago).unwrap(), 0);
    }

    #[test]
    fn a_held_deletion_stays_held_however_often_the_server_repeats_it() {
        let (mut app, server, notes) = app_with_deletions_from_the_server(MASS_DELETION_THRESHOLD);
        assert_eq!(perform_sync(&mut app, false).unwrap().deletions_held, MASS_DELETION_THRESHOLD);

        for cursor in ["c2", "c3"] {
            server.script(vec![deletions_page(&notes, cursor)]);
            let outcome = perform_sync(&mut app, false).unwrap();
            assert_eq!(outcome.deletions_held, MASS_DELETION_THRESHOLD);
        }

        assert_eq!(bin_count(&app), 0);
        assert_eq!(stored_note_count(&app), MASS_DELETION_THRESHOLD);
    }

    /// The limit would mean nothing if a server could stay under it on each sync.
    #[test]
    fn a_few_deletions_at_a_time_do_not_get_past_the_limit() {
        let batch = MASS_DELETION_THRESHOLD - 1;
        let (mut app, server, notes) = app_with_synced_notes(batch * 2);

        server.script(vec![deletions_page(&notes[..batch], "c2")]);
        assert_eq!(perform_sync(&mut app, false).unwrap().deletions_held, 0);
        assert_eq!(bin_count(&app), batch);

        server.script(vec![deletions_page(&notes[batch..], "c3")]);
        assert_eq!(perform_sync(&mut app, false).unwrap().deletions_held, batch);
        assert_eq!(bin_count(&app), batch);
    }

    /// Taking some of a held batch away another way mustn't drop what's left under the
    /// limit, or the rest would go without the question ever being answered.
    #[test]
    fn a_held_deletion_stays_held_when_some_of_it_goes_another_way() {
        let (mut app, server, notes) = app_with_deletions_from_the_server(MASS_DELETION_THRESHOLD);
        perform_sync(&mut app, false).unwrap();

        let pulled: Vec<Value> = notes[..2].iter().map(binned_long_ago).collect();
        server.script(vec![cursor_page(pulled, "c2", false)]);
        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.deletions_held, MASS_DELETION_THRESHOLD - 2);
        assert_eq!(bin_count(&app), 2);
    }

    /// Deleting a few here doesn't answer the question about the rest. Nothing has reached
    /// the bin from elsewhere, so only having been asked keeps these held.
    #[test]
    fn a_held_deletion_stays_held_when_the_user_deletes_some_of_it() {
        let (mut app, _server, notes) = app_with_deletions_from_the_server(MASS_DELETION_THRESHOLD);
        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        note_repo.delete(&notes[0].id).unwrap();
        note_repo.delete(&notes[1].id).unwrap();

        let outcome = perform_sync(&mut app, false).unwrap();

        assert_eq!(outcome.deletions_held, MASS_DELETION_THRESHOLD - 2);
        assert_eq!(bin_count(&app), 2);
    }

    /// Once it's been answered, it's over.
    #[test]
    fn an_answered_deletion_does_not_hold_up_the_next_one() {
        let (mut app, server, notes) = app_with_deletions_from_the_server(MASS_DELETION_THRESHOLD);
        perform_sync(&mut app, false).unwrap();
        refuse_held_deletions(&mut app).unwrap();

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        assert_eq!(note_repo.count_held_deletions().unwrap(), 0);
        assert!(!note_repo.has_unanswered_deletions().unwrap());

        // One note on its own is under the limit, so it goes to the bin as usual
        server.script(vec![deletions_page(&notes[..1], "c2")]);
        let outcome = perform_sync(&mut app, false).unwrap();
        assert_eq!(outcome.deletions_held, 0);
    }

    /// The first pull of a note can't be the only way in: one already here goes the same.
    #[test]
    fn a_note_here_that_a_pull_puts_in_the_bin_starts_its_time_now() {
        let (mut app, server, notes) = app_with_synced_notes(1);
        server.script(vec![cursor_page(vec![binned_long_ago(&notes[0])], "c2", false)]);

        perform_sync(&mut app, false).unwrap();

        assert_eq!(bin_count(&app), 1);
        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        assert_eq!(note_repo.purge_expired_bin(thirty_days_ago).unwrap(), 0);
        assert_eq!(note_repo.purge_remote_deletions(thirty_days_ago).unwrap(), 0);
        assert!(note_repo.get_pending_deletions().unwrap().is_empty());
    }

    /// A live note can carry a date from a pull. It says nothing about when it was deleted.
    #[test]
    fn a_date_left_on_a_live_note_does_not_date_its_deletion() {
        let (mut app, server, notes) = app_with_synced_notes(1);
        let mut pulled = binned_long_ago(&notes[0]);
        pulled["deleted"] = json!(false);
        server.script(vec![cursor_page(vec![pulled], "c2", false)]);
        perform_sync(&mut app, false).unwrap();

        server.script(vec![deletions_page(&notes, "c3")]);
        perform_sync(&mut app, false).unwrap();

        let stored = stored_note(&app, &notes[0].id);
        assert!(stored.deleted);
        assert!(stored.deleted_at.unwrap() > Utc::now() - chrono::Duration::days(1));
    }

    /// A note restored from the bin isn't deleted any more, whatever the old mark said.
    #[test]
    fn a_restored_note_is_not_purged() {
        let (mut app, _server, notes) = app_with_deletions_from_the_server(1);
        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let mut note = note_repo.get(&notes[0].id, key()).unwrap().unwrap();
        note.restore();
        note_repo.update(&note, key()).unwrap();

        assert_eq!(note_repo.purge_remote_deletions(Utc::now() + chrono::Duration::days(1)).unwrap(), 0);
    }

    /// A TypeScript note from the web used to come back from an edit here as Markdown.
    #[test]
    fn a_note_type_this_client_does_not_know_survives_an_edit() {
        let mut notes = remote_notes(1);
        notes[0]["syntaxLanguage"] = json!("rust");
        let server = StubServer::start(notes, vec![]);
        let mut app = app_connected_to(&server.endpoint);

        perform_sync(&mut app, false).unwrap();

        let db = app.db.as_ref().unwrap();
        let note_repo = NoteRepository::new(db.connection());
        let mut note = note_repo.get(&note_id(0), key()).unwrap().unwrap();
        assert_eq!(note.syntax_language, crate::models::SyntaxLanguage::Plain);

        note.content = "Edited here".to_string();
        note.touch();
        note_repo.update(&note, key()).unwrap();
        perform_sync(&mut app, false).unwrap();

        assert_eq!(server.last_push()["notes"][0]["syntaxLanguage"], json!("rust"));
    }
}
