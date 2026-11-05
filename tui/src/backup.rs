//! Encrypted backups, in the format the web app defines
//!
//! This writes v3.0: batches of records, each a JSON array that's compressed and
//! then encrypted as a JWE token. It reads v3.0, and v2.x with one JWE per record.
//! Web, iOS and the TUI each restore what the others write, so what's in a record
//! follows the web app's storage format, not this client's own.

use anyhow::{Context, Result, anyhow};
use tracing::warn;
use base64::{Engine as _, engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}};
use chrono::{DateTime, Utc};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use crate::{
    crypto::{CryptoService, EncryptedData},
    db::Database,
    models::{Attachment, Note, SyntaxLanguage, UserSettings},
    repository::{
        attachment::AttachmentRepository, EncryptionRepository, NoteRepository, NoteVersion,
        NoteVersionRepository, SettingsRepository, VersionReason,
    },
    ui::operations::sync::filename_from_sync,
    vault,
};

const BACKUP_VERSION: &str = "3.0";
const BACKUP_TYPE: &str = "jottery-encrypted-backup";

// Batch sizes, as the web app uses them
const NOTE_BATCH_SIZE: usize = 250;
const ATTACHMENT_BATCH_SIZE: usize = 50;
const VERSION_BATCH_SIZE: usize = 250;
const SAVED_SEARCH_BATCH_SIZE: usize = 250;

/// Most a batch may inflate to. Far above any real batch, and it stops a damaged or
/// hostile file from filling memory.
const MAX_BATCH_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// JWE header for direct encryption with AES-256-GCM
#[derive(Debug, Serialize, Deserialize)]
struct JweHeader {
    alg: String,
    enc: String,
}

/// Backup file structure. `data` holds v2.x records and `batches` holds v3.0's.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupData {
    pub version: String,
    #[serde(rename = "type")]
    pub backup_type: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub encryption: BackupEncryption,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<String>, // JWE compact serialization strings
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batches: Vec<BackupBatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<BackupSummary>,
    /// Records this vault held but couldn't put in the backup. Not part of the file.
    #[serde(skip)]
    pub skipped: usize,
}

/// One batch of records in a v3.0 backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupBatch {
    #[serde(rename = "type")]
    pub batch_type: String,
    pub index: usize,
    pub count: usize,
    pub data: String, // JWE compact serialization
}

/// What a v3.0 backup holds
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub notes: usize,
    pub attachments: usize,
    pub versions: usize,
    pub saved_searches: usize,
}

impl BackupData {
    /// How many records or batches the file holds
    pub fn record_count(&self) -> usize {
        self.data.len() + self.batches.len()
    }
}

/// How to get from the password to the key the records are encrypted with.
/// Web, iOS and the TUI all write and read this header, so keep them in step.
///
/// An envelope vault encrypts with a random master key, so the backup carries it
/// wrapped with the password, as the vault stores it. A legacy vault's key comes
/// straight from the password, salt and iterations.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupEncryption {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_salt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_wrapped_master: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapping_kdf_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterations: Option<i64>,
    pub algorithm: String,
}

impl BackupEncryption {
    /// Whether the header leads from a password to the key. Backups of envelope
    /// vaults made before it carried the wrapped key have an empty salt, and
    /// only open with the vault's own master key.
    pub fn names_its_key(&self) -> bool {
        self.envelope_version.is_some()
            || self.salt.as_deref().is_some_and(|salt| !salt.is_empty())
    }
}

/// A note as a backup holds it, which is how the web app stores one. Its encrypted
/// fields are strings of JSON. iOS writes `tags` and `attachments` as one string of
/// JSON each, and older TUI backups leave attachments empty, so those stay loose.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupNote {
    id: String,
    created_at: String,
    modified_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    synced_at: Option<String>,
    content: String,
    #[serde(default)]
    tags: Value,
    #[serde(default)]
    attachments: Value,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    deleted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    deleted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sync_hash: Option<String>,
    #[serde(default = "first_version")]
    version: i32,
    #[serde(default)]
    word_wrap: Option<bool>,
    #[serde(default)]
    syntax_language: Option<String>,
    #[serde(default)]
    show_preview: bool,
    #[serde(default)]
    archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    archived_at: Option<String>,
    #[serde(default)]
    locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    locked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hash_chain: Option<Vec<String>>,
}

fn first_version() -> i32 {
    1
}

/// A note version as a backup holds it
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupVersion {
    version_key: String,
    note_id: String,
    version: i32,
    created_at: String,
    synced_at: String,
    content: String,
    #[serde(default)]
    tags: Value,
    #[serde(default)]
    attachments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    syntax_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    word_wrap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    show_preview: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default = "sync_reason")]
    reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_hash: Option<String>,
}

fn sync_reason() -> String {
    "sync".to_string()
}

/// An attachment's data: the blob as the web app stores it, base64 encoded
#[derive(Debug, Serialize, Deserialize)]
struct BackupAttachment {
    id: String,
    blob: String,
}

/// An attachment reference, in a note or a version
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupAttachmentRef {
    id: String,
    filename: String,
    mime_type: String,
    size: i64,
    data: String,
}

/// Progress callback for backup operations
pub type ProgressCallback = Box<dyn Fn(usize, usize, &str)>;

/// Encrypt bytes as JWE compact serialization.
///
/// RFC 7516 authenticates the protected header: the additional data is the header's
/// base64url text. Web and iOS check it, so a token without it won't open there.
fn encrypt_as_jwe(plaintext: &[u8], key: &[u8; 32]) -> Result<String> {
    let crypto = CryptoService::new();

    let header = JweHeader {
        alg: "dir".to_string(),
        enc: "A256GCM".to_string(),
    };
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_string(&header)?.as_bytes());

    let encrypted = crypto.encrypt_binary_with_aad(plaintext, header_b64.as_bytes(), key)?;

    // BASE64URL(header).BASE64URL(encrypted_key).BASE64URL(IV).BASE64URL(ciphertext).BASE64URL(tag)
    // For "dir" the encrypted key is empty. EncryptedData is standard base64 with the
    // GCM tag on the end of the ciphertext, where JWE wants them apart.
    let nonce_bytes = STANDARD.decode(&encrypted.nonce).context("Failed to decode nonce")?;
    let ciphertext_bytes = STANDARD.decode(&encrypted.ciphertext).context("Failed to decode ciphertext")?;
    let (ct, tag) = ciphertext_bytes.split_at(ciphertext_bytes.len().saturating_sub(16));

    Ok(format!(
        "{}..{}.{}.{}",
        header_b64,
        URL_SAFE_NO_PAD.encode(&nonce_bytes),
        URL_SAFE_NO_PAD.encode(ct),
        URL_SAFE_NO_PAD.encode(tag),
    ))
}

/// Decrypt a JWE compact serialization to its plaintext
fn decrypt_jwe(jwe: &str, key: &[u8; 32]) -> Result<Vec<u8>> {
    let crypto = CryptoService::new();

    // Parse JWE compact serialization
    let parts: Vec<&str> = jwe.split('.').collect();
    if parts.len() != 5 {
        return Err(anyhow!("Invalid JWE format: expected 5 parts, got {}", parts.len()));
    }

    // Decode header and verify algorithm
    let header_bytes = URL_SAFE_NO_PAD.decode(parts[0])
        .context("Failed to decode JWE header")?;
    let header: JweHeader = serde_json::from_slice(&header_bytes)
        .context("Failed to parse JWE header")?;

    if header.alg != "dir" || header.enc != "A256GCM" {
        return Err(anyhow!("Unsupported JWE algorithms: alg={}, enc={}", header.alg, header.enc));
    }

    // parts[1] is encrypted_key, should be empty for "dir"
    let nonce_bytes = URL_SAFE_NO_PAD.decode(parts[2]).context("Failed to decode IV")?;
    let ciphertext_bytes = URL_SAFE_NO_PAD.decode(parts[3]).context("Failed to decode ciphertext")?;
    let tag_bytes = URL_SAFE_NO_PAD.decode(parts[4]).context("Failed to decode tag")?;

    // Combine ciphertext and tag (GCM expects them together)
    let mut combined = ciphertext_bytes;
    combined.extend_from_slice(&tag_bytes);

    // Reconstruct EncryptedData with base64 STANDARD encoding (as expected by CryptoService)
    let encrypted = EncryptedData {
        nonce: STANDARD.encode(&nonce_bytes),
        ciphertext: STANDARD.encode(&combined),
        tag: String::new(), // Tag is included in ciphertext for GCM
    };

    // Older TUI backups left the header unauthenticated, and those must stay readable
    crypto.decrypt_binary_with_aad(&encrypted, parts[0].as_bytes(), key)
        .or_else(|_| crypto.decrypt_binary(&encrypted, key))
}

/// JSON array → compress → JWE, as a batch is stored
fn encrypt_batch(items: &[Value], key: &[u8; 32]) -> Result<String> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    serde_json::to_writer(&mut encoder, items)?;
    encrypt_as_jwe(&encoder.finish()?, key)
}

/// JWE → inflate → JSON array
fn decrypt_batch(jwe: &str, key: &[u8; 32]) -> Result<Vec<Value>> {
    let compressed = decrypt_jwe(jwe, key)?;

    let mut json = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .take(MAX_BATCH_BYTES + 1)
        .read_to_end(&mut json)
        .context("Failed to decompress batch")?;
    if json.len() as u64 > MAX_BATCH_BYTES {
        return Err(anyhow!("Batch is too large"));
    }

    serde_json::from_slice(&json).context("Failed to parse batch")
}

/// A field encrypted the way the web app stores it: EncryptedData as a string of JSON
fn encrypted_string(crypto: &CryptoService, text: &str, key: &[u8; 32]) -> Result<String> {
    Ok(serde_json::to_string(&crypto.encrypt_text(text, key)?)?)
}

/// Tags as the web app stores them: one blob holding the JSON array
fn tags_for_backup(crypto: &CryptoService, tags: &[String], key: &[u8; 32]) -> Result<Value> {
    if tags.is_empty() {
        return Ok(json!([]));
    }
    let blob = serde_json::to_string(&crypto.encrypt_json(&tags, key)?)?;
    Ok(json!([blob]))
}

/// Attachment references with encrypted filenames, as the web app stores them
fn attachments_for_backup(crypto: &CryptoService, attachments: &[Attachment], key: &[u8; 32]) -> Result<Value> {
    let refs = attachments.iter()
        .map(|attachment| Ok(BackupAttachmentRef {
            id: attachment.id.clone(),
            filename: encrypted_string(crypto, &attachment.filename, key)?,
            mime_type: attachment.mime_type.clone(),
            size: attachment.size,
            data: attachment.id.clone(),
        }))
        .collect::<Result<Vec<_>>>()?;
    Ok(serde_json::to_value(refs)?)
}

fn note_for_backup(crypto: &CryptoService, note: &Note, key: &[u8; 32]) -> Result<Value> {
    Ok(serde_json::to_value(BackupNote {
        id: note.id.clone(),
        created_at: note.created_at.to_rfc3339(),
        modified_at: note.modified_at.to_rfc3339(),
        synced_at: note.synced_at.map(|dt| dt.to_rfc3339()),
        content: encrypted_string(crypto, &note.content, key)?,
        tags: tags_for_backup(crypto, &note.tags, key)?,
        attachments: attachments_for_backup(crypto, &note.attachments, key)?,
        pinned: note.pinned,
        deleted: note.deleted,
        deleted_at: note.deleted_at.map(|dt| dt.to_rfc3339()),
        sync_hash: note.sync_hash.clone(),
        version: note.version,
        word_wrap: Some(note.word_wrap),
        syntax_language: Some(note.syntax_name()),
        show_preview: false,
        archived: note.archived,
        archived_at: note.archived_at.map(|dt| dt.to_rfc3339()),
        locked: note.locked,
        locked_at: note.locked_at.map(|dt| dt.to_rfc3339()),
        color: note.color.clone(),
        content_hash: note.content_hash.clone(),
        parent_hash: note.parent_hash.clone(),
        hash_chain: note.hash_chain.clone(),
    })?)
}

fn version_for_backup(crypto: &CryptoService, version: &NoteVersion, key: &[u8; 32]) -> Result<Value> {
    Ok(serde_json::to_value(BackupVersion {
        version_key: version.version_key.clone(),
        note_id: version.note_id.clone(),
        version: version.version,
        created_at: version.created_at.to_rfc3339(),
        synced_at: version.synced_at.to_rfc3339(),
        content: encrypted_string(crypto, &version.content, key)?,
        tags: tags_for_backup(crypto, &version.tags, key)?,
        attachments: attachments_for_backup(crypto, &version.attachments, key)?,
        syntax_language: version.syntax_language.as_ref().map(|s| s.to_string()),
        word_wrap: version.word_wrap,
        show_preview: version.show_preview,
        color: version.color.clone(),
        reason: version.reason.to_string(),
        content_hash: version.content_hash.clone(),
        parent_hash: version.parent_hash.clone(),
    })?)
}

/// Settings under the names the web app reads. `theme` means something else there,
/// so this client's scheme goes alongside it. Nothing sync-related belongs in here:
/// a restored vault has to register as a device of its own.
fn settings_for_backup(settings: &UserSettings) -> Value {
    let scheme = settings.theme.scheme_name();
    let mut item = json!({
        // "auto" is this client's alone, and a backup goes to all of them
        "language": if settings.language == crate::models::AUTO_LANGUAGE { "en-GB" } else { settings.language.as_str() },
        "theme": if scheme.contains("light") { "light" } else { "dark" },
        "tuiTheme": scheme,
        "sortOrder": settings.sort_order,
        "autoLockTimeout": settings.auto_lock_timeout,
    });
    if let Some(palette) = &settings.color_palette {
        item["colorPalette"] = json!(palette);
    }
    if let Some(tag_colors) = &settings.tag_colors {
        item["tagColors"] = json!(tag_colors);
    }
    item
}

/// Group items into batches of `batch_type`
fn add_batches(
    batches: &mut Vec<BackupBatch>,
    batch_type: &str,
    items: &[Value],
    size: usize,
    key: &[u8; 32],
) -> Result<()> {
    for (index, chunk) in items.chunks(size).enumerate() {
        batches.push(BackupBatch {
            batch_type: batch_type.to_string(),
            index,
            count: chunk.len(),
            data: encrypt_batch(chunk, key)?,
        });
    }
    Ok(())
}

/// Create an encrypted backup of all data.
///
/// Anything the vault holds but can't read is counted in `skipped`, so the caller
/// can say the backup is incomplete.
pub fn create_backup(
    db: &Database,
    key: &[u8; 32],
    on_progress: Option<ProgressCallback>,
) -> Result<BackupData> {
    let note_repo = NoteRepository::new(db.connection());
    let version_repo = NoteVersionRepository::new(db.connection());
    let attachment_repo = AttachmentRepository::new(db.connection());
    let encryption_repo = EncryptionRepository::new(db.connection());
    let settings_repo = SettingsRepository::new(db.connection());
    let crypto = CryptoService::new();

    let report = |current: usize, total: usize, item: &str| {
        if let Some(callback) = &on_progress {
            callback(current, total, item);
        }
    };

    // Get encryption metadata
    let encryption_meta = encryption_repo.get()?
        .ok_or_else(|| anyhow!("No encryption metadata found"))?;

    let listed = note_repo.list_everything(key)?; // Include deleted notes
    let mut skipped = listed.unreadable;
    let notes = listed.notes;

    let mut note_items = Vec::with_capacity(notes.len());
    let mut version_items = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        note_items.push(note_for_backup(&crypto, note, key)?);
        for version in version_repo.get_versions_for_note(&note.id, key)? {
            version_items.push(version_for_backup(&crypto, &version, key)?);
        }
        report(i + 1, notes.len(), "notes");
    }

    let attachment_ids = attachment_repo.list_ids()?;
    let mut attachment_items = Vec::with_capacity(attachment_ids.len());
    for (i, id) in attachment_ids.iter().enumerate() {
        match attachment_repo.get(id, key) {
            Ok(Some((_filename, _mime_type, _size, data))) => {
                let blob = serde_json::to_vec(&crypto.encrypt_binary(&data, key)?)?;
                attachment_items.push(serde_json::to_value(BackupAttachment {
                    id: id.clone(),
                    blob: STANDARD.encode(blob),
                })?);
            }
            Ok(None) => {}
            Err(e) => {
                warn!("[Backup] Attachment {id} can't be read: {e}");
                skipped += 1;
            }
        }
        report(i + 1, attachment_ids.len(), "attachments");
    }

    let mut batches = Vec::new();
    add_batches(&mut batches, "notes", &note_items, NOTE_BATCH_SIZE, key)?;
    add_batches(&mut batches, "attachments", &attachment_items, ATTACHMENT_BATCH_SIZE, key)?;
    add_batches(&mut batches, "versions", &version_items, VERSION_BATCH_SIZE, key)?;

    let saved_search_items = crate::repository::SavedSearchRepository::new(db.connection()).list_stored()?
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    add_batches(&mut batches, "saved_searches", &saved_search_items, SAVED_SEARCH_BATCH_SIZE, key)?;

    let settings = settings_for_backup(&settings_repo.get()?);
    add_batches(&mut batches, "settings", &[settings], 1, key)?;
    report(1, 1, "settings");

    // Build backup data
    Ok(BackupData {
        version: BACKUP_VERSION.to_string(),
        backup_type: BACKUP_TYPE.to_string(),
        created_at: Utc::now().to_rfc3339(),
        encryption: if encryption_meta.envelope_version.is_some() {
            BackupEncryption {
                envelope_version: encryption_meta.envelope_version,
                device_salt: encryption_meta.device_salt.clone(),
                local_wrapped_master: encryption_meta.local_wrapped_master.clone(),
                wrapping_kdf_version: encryption_meta.wrapping_kdf_version,
                salt: None,
                iterations: None,
                algorithm: encryption_meta.algorithm.clone(),
            }
        } else {
            BackupEncryption {
                envelope_version: None,
                device_salt: None,
                local_wrapped_master: None,
                wrapping_kdf_version: None,
                salt: encryption_meta.salt.as_deref().map(|salt| STANDARD.encode(salt)),
                iterations: Some(encryption_meta.iterations.unwrap_or(600_000) as i64),
                algorithm: "PBKDF2".to_string(),
            }
        },
        data: Vec::new(),
        batches,
        summary: Some(BackupSummary {
            notes: note_items.len(),
            attachments: attachment_items.len(),
            versions: version_items.len(),
            saved_searches: saved_search_items.len(),
        }),
        skipped,
    })
}

/// Write backup to file
pub fn write_backup<P: AsRef<Path>>(backup: &BackupData, path: P) -> Result<()> {
    let file = File::create(path.as_ref())
        .context("Failed to create backup file")?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer(&mut writer, backup)?;
    writer.flush()?;
    Ok(())
}

/// Validate a backup file
pub fn validate_backup<P: AsRef<Path>>(path: P) -> Result<BackupData> {
    let file = File::open(path.as_ref())
        .context("Failed to open backup file")?;
    let reader = BufReader::new(file);

    let backup: BackupData = serde_json::from_reader(reader)
        .context("Failed to parse backup file")?;

    // Validate structure
    if backup.backup_type != BACKUP_TYPE {
        return Err(anyhow!("Invalid backup type: {}", backup.backup_type));
    }

    // Check version compatibility
    let version_parts: Vec<&str> = backup.version.split('.').collect();
    if version_parts.len() < 2 {
        return Err(anyhow!("Invalid version format: {}", backup.version));
    }
    let major: i32 = version_parts[0].parse().unwrap_or(0);
    let minor: i32 = version_parts[1].parse().unwrap_or(0);

    if major > 3 || (major == 3 && minor > 0) {
        return Err(anyhow!("Backup version {} is not supported. Please update Jottery.", backup.version));
    }

    // Validate JWE format (5 parts separated by dots)
    let tokens = backup.data.iter().chain(backup.batches.iter().map(|batch| &batch.data));
    for (i, jwe) in tokens.enumerate() {
        let parts = jwe.split('.').count();
        if parts != 5 {
            return Err(anyhow!("Invalid JWE token at index {}: expected 5 parts, got {}", i, parts));
        }
    }

    Ok(backup)
}

/// Verify password against a backup by trying to decrypt first record
pub fn verify_backup_password(backup: &BackupData, password: &str) -> Result<[u8; 32]> {
    let crypto = CryptoService::new();
    let encryption = &backup.encryption;

    let key = if encryption.envelope_version.is_some() {
        let device_salt = encryption.device_salt.as_deref()
            .ok_or_else(|| anyhow!("Backup is missing its device salt"))?;
        let device_salt = STANDARD.decode(device_salt)
            .context("Failed to decode device salt from backup")?;
        let wrapped = encryption.local_wrapped_master.as_deref()
            .ok_or_else(|| anyhow!("Backup is missing its wrapped key"))?;
        let wrapped: EncryptedData = serde_json::from_str(wrapped)
            .context("Failed to read wrapped key from backup")?;

        let device_key = crypto.derive_key(password, &device_salt, crate::crypto::DEFAULT_ITERATIONS)?;
        crypto.unwrap_master_key(&device_key, &wrapped)
            .context("Incorrect password")?
    } else {
        let salt = encryption.salt.as_deref()
            .ok_or_else(|| anyhow!("Backup is missing its salt"))?;
        let salt = STANDARD.decode(salt)
            .context("Failed to decode salt from backup")?;
        let iterations = encryption.iterations
            .ok_or_else(|| anyhow!("Backup is missing its iteration count"))?;
        crypto.derive_key(password, &salt, iterations as u32)?
    };

    verify_backup_key(backup, &key).context("Incorrect password")?;
    Ok(key)
}

/// Check a key against a backup by decrypting its first record.
pub fn verify_backup_key(backup: &BackupData, key: &[u8; 32]) -> Result<()> {
    if let Some(first) = backup.data.first().or(backup.batches.first().map(|batch| &batch.data)) {
        decrypt_jwe(first, key)?;
    }
    Ok(())
}

/// The keys a restore works with.
#[derive(Debug)]
pub struct RestoreKeys {
    /// What the backup's contents are encrypted with
    pub backup_key: [u8; 32],
    /// What they end up encrypted with
    pub vault_key: [u8; 32],
    /// The password for a vault this restore creates. Absent when there's one already.
    new_vault_password: Option<String>,
}

/// Work out where a backup's contents come from and where they're going.
///
/// A vault that already exists keeps its key, its metadata and its password, and the
/// backup moves into it, so its notes stay readable. With no vault, the backup's key
/// is wrapped with `local_password`, which unlocks the database from then on.
pub fn restore_keys(
    db: &Database,
    backup: &BackupData,
    password: &str,
    local_password: &str,
) -> Result<RestoreKeys> {
    let has_vault = EncryptionRepository::new(db.connection()).get()?.is_some();

    if !has_vault {
        if !backup.encryption.names_its_key() {
            anyhow::bail!("{}", rust_i18n::t!("backup.key_not_in_backup"));
        }
        if local_password != password {
            vault::check_new_password(local_password)?;
        }
        let backup_key = verify_backup_password(backup, password)?;
        return Ok(RestoreKeys {
            backup_key,
            vault_key: backup_key,
            new_vault_password: Some(local_password.to_string()),
        });
    }

    let vault_key = vault::unlock(db.connection(), &CryptoService::new(), local_password)?;

    // An older backup names no key, so only the vault it came from can open it
    let backup_key = if backup.encryption.names_its_key() {
        verify_backup_password(backup, password)?
    } else {
        verify_backup_key(backup, &vault_key)
            .map_err(|_| anyhow!("{}", rust_i18n::t!("backup.key_not_in_backup")))?;
        vault_key
    };

    Ok(RestoreKeys { backup_key, vault_key, new_vault_password: None })
}

/// What a restore did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RestoreSummary {
    pub notes: usize,
    pub versions: usize,
    pub attachments: usize,
    pub saved_searches: usize,
    /// Records that were in the backup but couldn't be restored
    pub failed: usize,
}

/// A backup's contents, decrypted and sorted by type.
#[derive(Default)]
struct BackupContents {
    notes: Vec<Value>,
    versions: Vec<Value>,
    attachments: Vec<Value>,
    saved_searches: Vec<Value>,
    settings: Option<Value>,
    failed: usize,
}

impl BackupContents {
    /// Sort one item in. Sync metadata has nowhere to go here: a restored device
    /// registers for itself.
    fn add(&mut self, item_type: &str, item: Value) {
        match item_type {
            "note" | "notes" => self.notes.push(item),
            "version" | "versions" => self.versions.push(item),
            "attachment" | "attachments" => self.attachments.push(item),
            "saved_search" | "saved_searches" => self.saved_searches.push(item),
            "settings" => self.settings = Some(item),
            _ => {}
        }
    }
}

/// Decrypt every record, whichever version of the format the backup uses.
fn read_contents(backup: &BackupData, key: &[u8; 32]) -> BackupContents {
    #[derive(Deserialize)]
    struct Record {
        #[serde(rename = "type")]
        record_type: String,
        data: Value,
    }

    let mut contents = BackupContents::default();

    for (i, jwe) in backup.data.iter().enumerate() {
        let record = decrypt_jwe(jwe, key)
            .and_then(|plaintext| Ok(serde_json::from_slice::<Record>(&plaintext)?));
        match record {
            Ok(record) => contents.add(&record.record_type, record.data),
            Err(e) => {
                warn!("[Backup] Skipping unreadable record at index {i}: {e}");
                contents.failed += 1;
            }
        }
    }

    for batch in &backup.batches {
        match decrypt_batch(&batch.data, key) {
            Ok(items) => {
                for item in items {
                    contents.add(&batch.batch_type, item);
                }
            }
            Err(e) => {
                warn!("[Backup] Skipping unreadable {} batch {}: {e}", batch.batch_type, batch.index);
                contents.failed += batch.count.max(1);
            }
        }
    }

    contents
}

/// Read tags in any form a client has written them, returning them with how many
/// couldn't be decrypted.
///
/// The web app stores one blob holding a JSON array, iOS writes that blob as a bare
/// string, and older TUI backups have one entry per tag holding its plain text.
fn read_tags(crypto: &CryptoService, tags: &Value, key: &[u8; 32]) -> (Vec<String>, usize) {
    let entries: Vec<&str> = match tags {
        Value::String(blob) => vec![blob.as_str()],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };

    let mut read = Vec::new();
    let mut failed = 0;
    for entry in entries {
        let decrypted = serde_json::from_str::<EncryptedData>(entry)
            .map_err(anyhow::Error::from)
            .and_then(|encrypted| crypto.decrypt_text(&encrypted, key));

        match decrypted {
            Ok(text) => match serde_json::from_str::<Vec<String>>(&text) {
                Ok(tags) => read.extend(tags),
                Err(_) => read.push(serde_json::from_str(&text).unwrap_or(text)),
            },
            Err(e) => {
                warn!("[Backup] Skipping undecryptable tag: {e}");
                failed += 1;
            }
        }
    }

    (read, failed)
}

/// Read attachment references, returning them with plain filenames and how many
/// couldn't be read.
fn read_attachment_refs(crypto: &CryptoService, attachments: &Value, key: &[u8; 32]) -> (Vec<Attachment>, usize) {
    let refs: Vec<BackupAttachmentRef> = match attachments {
        Value::String(json) => serde_json::from_str(json).unwrap_or_default(),
        Value::Array(_) => serde_json::from_value(attachments.clone()).unwrap_or_default(),
        _ => Vec::new(),
    };

    let mut read = Vec::with_capacity(refs.len());
    let mut failed = 0;
    for attachment in refs {
        match filename_from_sync(crypto, &attachment.filename, key) {
            Ok(filename) => read.push(Attachment {
                id: attachment.id,
                filename,
                mime_type: attachment.mime_type,
                size: attachment.size,
                data: attachment.data,
                thumbnail_data: None,
            }),
            Err(e) => {
                warn!("[Backup] Skipping attachment {} with an unreadable name: {e}", attachment.id);
                failed += 1;
            }
        }
    }

    (read, failed)
}

fn read_content(crypto: &CryptoService, content: &str, key: &[u8; 32]) -> Result<String> {
    let encrypted: EncryptedData = serde_json::from_str(content)?;
    crypto.decrypt_text(&encrypted, key)
}

fn optional_time(text: Option<String>) -> Option<DateTime<Utc>> {
    text.and_then(|t| t.parse().ok())
}

/// Turn a note from a backup into a Note, returning how many of its parts were lost.
/// A saved search from a backup, in this vault's key. A backup from an older web client
/// can hold plaintext, so text that isn't ciphertext is taken as that, and ciphertext that
/// won't open is an error. A deleted row is never shown, so it stays as it is.
fn read_saved_search(crypto: &CryptoService, item: Value, keys: &RestoreKeys) -> Result<crate::models::sync::SyncSavedSearch> {
    let mut search: crate::models::sync::SyncSavedSearch = serde_json::from_value(item)?;
    if !crate::models::validation::is_uuid(&search.id) {
        anyhow::bail!("Saved search has no usable id");
    }

    let moved = |stored: &str| -> Result<String> {
        let plain = match serde_json::from_str::<EncryptedData>(stored) {
            Ok(encrypted) => crypto.decrypt_text(&encrypted, &keys.backup_key)?,
            Err(_) => stored.to_string(),
        };
        Ok(serde_json::to_string(&crypto.encrypt_text(&plain, &keys.vault_key)?)?)
    };

    match (moved(&search.name), moved(&search.query)) {
        (Ok(name), Ok(query)) => {
            search.name = name;
            search.query = query;
        }
        _ if search.deleted => {}
        (Err(e), _) | (_, Err(e)) => return Err(e),
    }

    Ok(search)
}

fn read_note(crypto: &CryptoService, item: Value, key: &[u8; 32]) -> Result<(Note, usize)> {
    let note: BackupNote = serde_json::from_value(item)?;

    let content = read_content(crypto, &note.content, key)?;
    let (tags, lost_tags) = read_tags(crypto, &note.tags, key);
    let (attachments, lost_attachments) = read_attachment_refs(crypto, &note.attachments, key);

    let (syntax_language, syntax_unknown) = note.syntax_language.as_deref()
        .map(SyntaxLanguage::from_name)
        .unwrap_or_default();

    Ok((Note {
        created_at: note.created_at.parse()?,
        modified_at: note.modified_at.parse()?,
        synced_at: optional_time(note.synced_at),
        deleted_at: optional_time(note.deleted_at),
        archived_at: optional_time(note.archived_at),
        locked_at: optional_time(note.locked_at),
        id: note.id,
        content,
        tags,
        attachments,
        pinned: note.pinned,
        deleted: note.deleted,
        sync_hash: note.sync_hash,
        version: note.version,
        word_wrap: note.word_wrap.unwrap_or(true),
        syntax_language,
        syntax_unknown,
        archived: note.archived,
        locked: note.locked,
        color: note.color,
        content_hash: note.content_hash,
        parent_hash: note.parent_hash,
        hash_chain: note.hash_chain,
        show_preview: None,
        // Settled when the restore finishes, by comparing with the last sync
        needs_sync: false,
        // A backup from an older build can have lost a note's list
        attachments_complete: false,
    }, lost_tags + lost_attachments))
}

/// Turn a version from a backup into a NoteVersion, returning how many of its parts
/// were lost.
fn read_version(crypto: &CryptoService, item: Value, key: &[u8; 32]) -> Result<(NoteVersion, usize)> {
    let version: BackupVersion = serde_json::from_value(item)?;

    let content = read_content(crypto, &version.content, key)?;
    let (tags, lost_tags) = read_tags(crypto, &version.tags, key);
    let (attachments, lost_attachments) = read_attachment_refs(crypto, &version.attachments, key);

    Ok((NoteVersion {
        created_at: version.created_at.parse()?,
        synced_at: version.synced_at.parse()?,
        version_key: version.version_key,
        note_id: version.note_id,
        version: version.version,
        content,
        tags,
        attachments,
        syntax_language: version.syntax_language.and_then(|s| s.parse().ok()),
        word_wrap: version.word_wrap,
        show_preview: version.show_preview,
        color: version.color,
        reason: version.reason.parse().unwrap_or(VersionReason::Sync),
        content_hash: version.content_hash,
        parent_hash: version.parent_hash,
    }, lost_tags + lost_attachments))
}

/// Apply settings from a backup, in either client's spelling. Only what describes
/// how the app looks and behaves: sync belongs to this device.
fn restore_settings(settings_repo: &SettingsRepository<'_>, item: &Value) -> Result<()> {
    let field = |camel: &str, snake: &str| item.get(camel).or_else(|| item.get(snake)).cloned();
    let mut settings = settings_repo.get()?;

    if let Some(language) = item.get("language").and_then(Value::as_str) {
        settings.language = language.to_string();
    }
    if let Some(sort_order) = field("sortOrder", "sort_order").and_then(|v| serde_json::from_value(v).ok()) {
        settings.sort_order = sort_order;
    }
    if let Some(timeout) = field("autoLockTimeout", "auto_lock_timeout").and_then(|v| v.as_i64()) {
        settings.auto_lock_timeout = timeout as i32;
    }
    // The web app's light, dark and auto aren't colour schemes
    let theme = item.get("tuiTheme").or_else(|| {
        item.get("theme").filter(|theme| !matches!(theme.as_str(), Some("light" | "dark" | "auto")))
    });
    if let Some(theme) = theme.and_then(|v| serde_json::from_value(v.clone()).ok()) {
        settings.theme = theme;
    }
    if let Some(palette) = field("colorPalette", "color_palette").and_then(|v| serde_json::from_value(v).ok()) {
        settings.color_palette = Some(palette);
    }
    if let Some(tag_colors) = field("tagColors", "tag_colors").and_then(|v| serde_json::from_value(v).ok()) {
        settings.tag_colors = Some(tag_colors);
    }

    settings_repo.update(&settings)
}

/// Restore data from a backup.
///
/// What can be read is restored, and what can't is counted in the summary for the
/// caller to report. A damaged backup still gives back what it can, in one
/// transaction, so a failure part-way through changes nothing.
pub fn restore_backup(
    db: &Database,
    backup: &BackupData,
    keys: &RestoreKeys,
    on_progress: Option<ProgressCallback>,
) -> Result<RestoreSummary> {
    let crypto = CryptoService::new();
    let transaction = db.connection().unchecked_transaction()?;

    let note_repo = NoteRepository::new(&transaction);
    let version_repo = NoteVersionRepository::new(&transaction);
    let attachment_repo = AttachmentRepository::new(&transaction);
    let settings_repo = SettingsRepository::new(&transaction);

    if let Some(password) = &keys.new_vault_password {
        vault::store_local_envelope(&transaction, &crypto, password, &keys.vault_key)?;
    }

    let contents = read_contents(backup, &keys.backup_key);
    let mut summary = RestoreSummary { failed: contents.failed, ..Default::default() };
    let total = contents.notes.len() + contents.versions.len() + contents.attachments.len();
    let mut done = 0;
    let report = |done: usize, item: &str| {
        if let Some(callback) = &on_progress {
            callback(done, total, item);
        }
    };

    // Attachment records hold only data. Their names and types are on the references.
    let mut references: HashMap<String, Attachment> = HashMap::new();

    for item in contents.notes {
        done += 1;
        report(done, "notes");

        match read_note(&crypto, item, &keys.backup_key) {
            Ok((note, lost)) => {
                summary.failed += lost;
                references.extend(note.attachments.iter().map(|a| (a.id.clone(), a.clone())));

                let stored = note_repo.create(&note, &keys.vault_key)
                    .or_else(|_| note_repo.store(&note, &keys.vault_key));
                // A backup can be old, and a note in its bin shouldn't vanish at the next
                // unlock, with a tombstone behind it, because of a date from back then
                let stored = stored.and_then(|()| match note.deleted {
                    true => note_repo.mark_arrived_in_bin(&note.id),
                    false => Ok(()),
                });
                match stored {
                    Ok(()) => summary.notes += 1,
                    Err(e) => {
                        warn!("[Backup] Could not store note {}: {e}", note.id);
                        summary.failed += 1;
                    }
                }
            }
            Err(e) => {
                warn!("[Backup] Skipping unreadable note: {e}");
                summary.failed += 1;
            }
        }
    }

    for item in contents.versions {
        done += 1;
        report(done, "versions");

        match read_version(&crypto, item, &keys.backup_key) {
            Ok((version, lost)) => {
                summary.failed += lost;
                references.extend(version.attachments.iter().map(|a| (a.id.clone(), a.clone())));

                if version_repo.get_version_by_key(&version.version_key)?.is_some() {
                    continue;
                }
                match version_repo.insert_version_from_sync(&version, &keys.vault_key) {
                    Ok(()) => summary.versions += 1,
                    Err(e) => {
                        warn!("[Backup] Could not store version {}: {e}", version.version_key);
                        summary.failed += 1;
                    }
                }
            }
            Err(e) => {
                warn!("[Backup] Skipping unreadable version: {e}");
                summary.failed += 1;
            }
        }
    }

    for item in contents.attachments {
        done += 1;
        report(done, "attachments");

        let restored = (|| -> Result<bool> {
            let attachment: BackupAttachment = serde_json::from_value(item)?;

            // Nothing refers to it, so there's no name or type to store it under
            let Some(reference) = references.get(&attachment.id) else {
                return Ok(false);
            };

            let encrypted: EncryptedData = serde_json::from_slice(&STANDARD.decode(&attachment.blob)?)?;
            let data = crypto.decrypt_binary(&encrypted, &keys.backup_key)?;
            attachment_repo.store(
                &reference.id,
                &reference.filename,
                &reference.mime_type,
                reference.size,
                &data,
                &keys.vault_key,
            )?;
            Ok(true)
        })();

        match restored {
            Ok(stored) => summary.attachments += stored as usize,
            Err(e) => {
                warn!("[Backup] Skipping unreadable attachment: {e}");
                summary.failed += 1;
            }
        }
    }

    let saved_search_repo = crate::repository::SavedSearchRepository::new(&transaction);
    for item in contents.saved_searches {
        match read_saved_search(&crypto, item, keys) {
            // Pending, so that it reaches an account, and on a later time than a copy
            // already here it wins, as any restored change does
            Ok(search) => match saved_search_repo.apply(&search, true, None) {
                Ok(stored) => summary.saved_searches += stored as usize,
                Err(e) => {
                    warn!("[Backup] Could not store a saved search: {e}");
                    summary.failed += 1;
                }
            },
            Err(e) => {
                warn!("[Backup] Skipping unreadable saved search: {e}");
                summary.failed += 1;
            }
        }
    }

    if let Some(settings) = &contents.settings {
        if let Err(e) = restore_settings(&settings_repo, settings) {
            warn!("[Backup] Could not restore settings: {e}");
        }
    }

    // The server may not have what came from the backup
    note_repo.mark_changed_since_last_sync()?;
    version_repo.mark_all_unpushed()?;

    transaction.commit()?;

    if summary.failed > 0 {
        warn!("[Backup] {} records could not be restored", summary.failed);
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::VersionReason;
    use tempfile::NamedTempFile;

    /// Get test password from environment variable or generate one.
    /// Set JOTTERY_TEST_PASSWORD env var to override.
    fn test_password() -> String {
        std::env::var("JOTTERY_TEST_PASSWORD").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
    }

    fn random_password() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn empty_database() -> Database {
        Database::in_memory(&random_password()).unwrap()
    }

    /// A vault whose notes use a random master key, wrapped with the password.
    /// Returns it with that key and password, both generated at runtime.
    fn envelope_vault() -> (Database, [u8; 32], String) {
        let crypto = CryptoService::new();
        let master_key = crypto.generate_master_key();
        let password = random_password();
        let db = empty_database();
        vault::store_local_envelope(db.connection(), &crypto, &password, &master_key).unwrap();
        NoteRepository::new(db.connection())
            .create(&Note::new("Kept safe in a backup".to_string()), &master_key)
            .unwrap();
        (db, master_key, password)
    }

    /// Restore `backup` the way the CLI does, with one password for both.
    fn restore_with(db: &Database, backup: &BackupData, password: &str) -> RestoreSummary {
        let keys = restore_keys(db, backup, password, password).unwrap();
        restore_backup(db, backup, &keys, None).unwrap()
    }

    fn notes_in(db: &Database, key: &[u8; 32]) -> Vec<Note> {
        NoteRepository::new(db.connection()).list(false, key).unwrap()
    }

    /// A note with tags, an attachment and a version.
    fn add_full_note(db: &Database, key: &[u8; 32]) -> Note {
        let mut note = Note::new("Full note".to_string());
        note.tags = vec!["first".to_string(), "second".to_string()];
        note.attachments.push(Attachment {
            id: "attachment-1".to_string(),
            filename: "photo.png".to_string(),
            mime_type: "image/png".to_string(),
            size: 3,
            data: "attachment-1".to_string(),
            thumbnail_data: None,
        });
        NoteRepository::new(db.connection()).create(&note, key).unwrap();
        AttachmentRepository::new(db.connection())
            .store("attachment-1", "photo.png", "image/png", 3, b"png", key)
            .unwrap();
        NoteVersionRepository::new(db.connection())
            .create_version(&note, Utc::now(), VersionReason::Sync, key)
            .unwrap();
        note
    }

    #[test]
    fn an_envelope_backup_restores_onto_an_empty_database() {
        let (db, master_key, password) = envelope_vault();
        let backup = create_backup(&db, &master_key, None).unwrap();

        let key = verify_backup_password(&backup, &password).unwrap();
        assert_eq!(key, master_key);

        let restored = empty_database();
        assert_eq!(restore_with(&restored, &backup, &password).notes, 1);

        let notes = notes_in(&restored, &key);
        assert_eq!(notes[0].content, "Kept safe in a backup");

        // Unlocking from now on goes through the restored metadata.
        let crypto = CryptoService::new();
        assert_eq!(vault::unlock(restored.connection(), &crypto, &password).unwrap(), master_key);
    }

    #[test]
    fn an_envelope_backup_refuses_the_wrong_password() {
        let (db, master_key, _) = envelope_vault();
        let backup = create_backup(&db, &master_key, None).unwrap();
        let wrong_password = random_password();
        let error = verify_backup_password(&backup, &wrong_password).unwrap_err();
        assert_eq!(error.to_string(), "Incorrect password");
    }

    #[test]
    fn the_header_uses_the_fields_the_other_clients_read() {
        let (db, master_key, _) = envelope_vault();
        let backup = create_backup(&db, &master_key, None).unwrap();
        let header = serde_json::to_value(&backup.encryption).unwrap();
        assert_eq!(header["envelopeVersion"], 1);
        assert!(header["deviceSalt"].is_string());
        assert!(header["localWrappedMaster"].is_string());
        assert!(header.get("salt").is_none());
    }

    #[test]
    fn an_older_backup_opens_with_its_vaults_key_and_leaves_the_metadata_alone() {
        let (db, master_key, password) = envelope_vault();
        let mut backup = create_backup(&db, &master_key, None).unwrap();
        backup.encryption = serde_json::from_str(r#"{"salt":"","iterations":600000,"algorithm":"PBKDF2"}"#).unwrap();
        let before = EncryptionRepository::new(db.connection()).get().unwrap().unwrap();
        let other_key = CryptoService::new().generate_master_key();

        assert!(!backup.encryption.names_its_key());
        assert!(verify_backup_key(&backup, &master_key).is_ok());
        assert!(verify_backup_key(&backup, &other_key).is_err());

        restore_with(&db, &backup, &password);
        let after = EncryptionRepository::new(db.connection()).get().unwrap().unwrap();
        assert_eq!(after.local_wrapped_master, before.local_wrapped_master);
    }

    #[test]
    fn an_older_backup_will_not_start_a_vault() {
        let (db, master_key, password) = envelope_vault();
        let mut backup = create_backup(&db, &master_key, None).unwrap();
        backup.encryption = serde_json::from_str(r#"{"salt":"","iterations":600000,"algorithm":"PBKDF2"}"#).unwrap();

        assert!(restore_keys(&empty_database(), &backup, &password, &password).is_err());
    }

    #[test]
    fn test_backup_restore_roundtrip() {
        let password = test_password();
        let crypto = CryptoService::new();
        let salt = crypto.generate_salt();
        let key = crypto.derive_key(&password, &salt, 100_000).unwrap();

        // Create temporary database
        let db = empty_database();
        let note_repo = NoteRepository::new(db.connection());
        let encryption_repo = EncryptionRepository::new(db.connection());

        // Save encryption metadata
        encryption_repo.save(&salt, 100_000).unwrap();

        // Create test notes
        let note1 = Note::new("Test note 1".to_string());
        let note2 = Note::new("Test note 2".to_string());

        note_repo.create(&note1, &key).unwrap();
        note_repo.create(&note2, &key).unwrap();

        // Create backup
        let backup = create_backup(&db, &key, None).unwrap();
        assert_eq!(backup.version, "3.0");
        assert!(!backup.batches.is_empty());

        // Write to file
        let backup_file = NamedTempFile::new().unwrap();
        write_backup(&backup, backup_file.path()).unwrap();

        // Validate backup
        let validated = validate_backup(backup_file.path()).unwrap();
        assert_eq!(validated.version, "3.0");

        // Verify password
        let verified_key = verify_backup_password(&validated, &password).unwrap();
        assert_eq!(verified_key, key);

        // Create new database and restore
        let db2 = empty_database();
        assert_eq!(restore_with(&db2, &validated, &password).notes, 2);

        // Verify notes were restored
        assert_eq!(notes_in(&db2, &key).len(), 2);
    }

    #[test]
    fn attachments_and_versions_come_back() {
        let (db, key, password) = envelope_vault();
        let note = add_full_note(&db, &key);
        let backup = create_backup(&db, &key, None).unwrap();

        let summary = backup.summary.as_ref().unwrap();
        assert_eq!((summary.notes, summary.attachments, summary.versions), (2, 1, 1));

        let restored = empty_database();
        let result = restore_with(&restored, &backup, &password);
        assert_eq!(result, RestoreSummary { notes: 2, versions: 1, attachments: 1, saved_searches: 0, failed: 0 });

        let conn = restored.connection();
        let read = NoteRepository::new(conn).get(&note.id, &key).unwrap().unwrap();
        assert_eq!(read.tags, vec!["first", "second"]);
        assert_eq!(read.attachments[0].filename, "photo.png");

        let (filename, mime_type, _, data) = AttachmentRepository::new(conn).get("attachment-1", &key).unwrap().unwrap();
        assert_eq!((filename.as_str(), mime_type.as_str(), data.as_slice()), ("photo.png", "image/png", b"png".as_slice()));

        let versions = NoteVersionRepository::new(conn).get_versions_for_note(&note.id, &key).unwrap();
        assert_eq!(versions[0].content, "Full note");
    }

    #[test]
    fn saved_searches_come_back_under_the_vaults_key() {
        use crate::repository::SavedSearchRepository;

        let (db, key, password) = envelope_vault();
        let repo = SavedSearchRepository::new(db.connection());
        repo.create("Work", "#work -done", &key).unwrap();
        let gone = repo.create("Gone", "old", &key).unwrap();
        repo.delete(&gone.id).unwrap();

        let backup = create_backup(&db, &key, None).unwrap();
        assert_eq!(backup.summary.as_ref().unwrap().saved_searches, 2);

        let restored = empty_database();
        let result = restore_with(&restored, &backup, &password);
        assert_eq!(result.saved_searches, 2);

        let repo = SavedSearchRepository::new(restored.connection());
        let listed = repo.list(&key).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].name.as_str(), listed[0].query.as_str()), ("Work", "#work -done"));

        // Pending, so that an account gets it from a restored device
        assert_eq!(repo.pending().unwrap().len(), 2);
    }

    /// Rewrites the fixture in tests/fixtures with a version and a saved search on top of
    /// what it held, keeping its key and its notes.
    #[test]
    #[ignore = "rewrites a file in the repository, so it only runs when asked"]
    fn regenerate_the_backup_fixture() {
        let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let path = fixtures.join("tui-v3.jottery-backup");
        let password = std::fs::read_to_string(fixtures.join("tui-v3.jottery-backup.password")).unwrap();

        let backup = validate_backup(&path).unwrap();
        let db = empty_database();
        let keys = restore_keys(&db, &backup, &password, &password).unwrap();
        restore_backup(&db, &backup, &keys, None).unwrap();

        let note = notes_in(&db, &keys.vault_key).into_iter()
            .find(|note| note.content.contains("A note from the TUI"))
            .unwrap();
        NoteVersionRepository::new(db.connection())
            .create_version(&note, Utc::now(), VersionReason::Sync, &keys.vault_key)
            .unwrap();
        crate::repository::SavedSearchRepository::new(db.connection())
            .create("Cross-client work", "#work", &keys.vault_key)
            .unwrap();

        write_backup(&create_backup(&db, &keys.vault_key, None).unwrap(), &path).unwrap();
    }

    #[test]
    fn a_restored_note_in_the_bin_gets_its_30_days_again() {
        let (db, key, password) = envelope_vault();
        let mut old = Note::new("Binned long ago".to_string());
        old.deleted = true;
        old.deleted_at = Some(Utc::now() - chrono::Duration::days(365));
        let repo = NoteRepository::new(db.connection());
        repo.create(&old, &key).unwrap();
        repo.store(&old, &key).unwrap();
        let backup = create_backup(&db, &key, None).unwrap();

        let restored = empty_database();
        restore_with(&restored, &backup, &password);

        let repo = NoteRepository::new(restored.connection());
        repo.purge_expired_bin(Utc::now() - chrono::Duration::days(30)).unwrap();
        repo.purge_remote_deletions(Utc::now() - chrono::Duration::days(30)).unwrap();
        assert!(repo.get(&old.id, &key).unwrap().is_some());
        assert!(repo.get_pending_deletions().unwrap().is_empty());
    }

    /// Older web clients kept these in plaintext, and a backup holds what they kept.
    #[test]
    fn a_plaintext_saved_search_from_an_older_web_backup_is_encrypted() {
        let crypto = CryptoService::new();
        let key = crypto.generate_master_key();
        let keys = RestoreKeys { backup_key: key, vault_key: key, new_vault_password: None };

        let search = read_saved_search(&crypto, serde_json::json!({
            "id": "0b9e4d6c-7a21-4f3e-8c55-1d2e3f4a5b6c",
            "name": "Plain",
            "query": "#plain",
            "order": 1,
            "createdAt": "2026-01-01T00:00:00Z",
            "modifiedAt": "2026-01-01T00:00:00Z",
            "deleted": false,
            "version": 1,
        }), &keys).unwrap();

        let name: EncryptedData = serde_json::from_str(&search.name).unwrap();
        assert_eq!(crypto.decrypt_text(&name, &key).unwrap(), "Plain");
    }

    /// Ciphertext that won't open isn't a name. It's an error, like any other.
    #[test]
    fn a_saved_search_that_will_not_open_is_counted_as_lost() {
        let crypto = CryptoService::new();
        let (key, other_key) = (crypto.generate_master_key(), crypto.generate_master_key());
        let keys = RestoreKeys { backup_key: key, vault_key: key, new_vault_password: None };
        let foreign = serde_json::to_string(&crypto.encrypt_text("Elsewhere", &other_key).unwrap()).unwrap();

        let result = read_saved_search(&crypto, serde_json::json!({
            "id": "0b9e4d6c-7a21-4f3e-8c55-1d2e3f4a5b6c",
            "name": foreign,
            "query": foreign,
            "order": 1,
            "createdAt": "2026-01-01T00:00:00Z",
            "modifiedAt": "2026-01-01T00:00:00Z",
            "deleted": false,
            "version": 1,
        }), &keys);

        assert!(result.is_err());
    }

    #[test]
    fn a_note_in_a_backup_is_stored_the_way_the_web_app_stores_it() {
        let (db, key, _) = envelope_vault();
        let note = add_full_note(&db, &key);
        let backup = create_backup(&db, &key, None).unwrap();
        let crypto = CryptoService::new();

        let batch = backup.batches.iter().find(|b| b.batch_type == "notes").unwrap();
        let items = decrypt_batch(&batch.data, &key).unwrap();
        let item = items.iter().find(|i| i["id"] == note.id.as_str()).unwrap();

        // Tags: one blob, holding the JSON array
        assert_eq!(item["tags"].as_array().unwrap().len(), 1);
        let blob: EncryptedData = serde_json::from_str(item["tags"][0].as_str().unwrap()).unwrap();
        let tags: Vec<String> = crypto.decrypt_json(&blob, &key).unwrap();
        assert_eq!(tags, vec!["first", "second"]);

        // Filenames: encrypted, as a string of JSON
        let filename = item["attachments"][0]["filename"].as_str().unwrap();
        assert!(!filename.contains("photo"));
        let encrypted: EncryptedData = serde_json::from_str(filename).unwrap();
        assert_eq!(crypto.decrypt_text(&encrypted, &key).unwrap(), "photo.png");

        assert_eq!(item["createdAt"], note.created_at.to_rfc3339());
    }

    #[test]
    fn the_jwe_header_is_authenticated() {
        let key = CryptoService::new().generate_master_key();
        let jwe = encrypt_as_jwe(b"secret", &key).unwrap();
        let parts: Vec<&str> = jwe.split('.').collect();

        assert_eq!(parts.len(), 5);
        assert_eq!(parts[1], "");
        assert_eq!(decrypt_jwe(&jwe, &key).unwrap(), b"secret");

        // Without the header as additional data the tag doesn't match, which is
        // what a reader following RFC 7516 relies on
        let mut combined = URL_SAFE_NO_PAD.decode(parts[3]).unwrap();
        combined.extend(URL_SAFE_NO_PAD.decode(parts[4]).unwrap());
        let encrypted = EncryptedData {
            nonce: STANDARD.encode(URL_SAFE_NO_PAD.decode(parts[2]).unwrap()),
            ciphertext: STANDARD.encode(combined),
            tag: String::new(),
        };
        assert!(CryptoService::new().decrypt_binary(&encrypted, &key).is_err());
    }

    /// A token as older TUI builds wrote them, without the header authenticated.
    fn jwe_without_aad(plaintext: &[u8], key: &[u8; 32]) -> String {
        let encrypted = CryptoService::new().encrypt_binary(plaintext, key).unwrap();
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"dir","enc":"A256GCM"}"#);
        let ciphertext = STANDARD.decode(&encrypted.ciphertext).unwrap();
        let (ct, tag) = ciphertext.split_at(ciphertext.len() - 16);
        format!(
            "{}..{}.{}.{}",
            header,
            URL_SAFE_NO_PAD.encode(STANDARD.decode(&encrypted.nonce).unwrap()),
            URL_SAFE_NO_PAD.encode(ct),
            URL_SAFE_NO_PAD.encode(tag),
        )
    }

    #[test]
    fn a_backup_from_an_older_tui_still_restores() {
        let crypto = CryptoService::new();
        let password = random_password();
        let salt = crypto.generate_salt();
        let key = crypto.derive_key(&password, &salt, 100_000).unwrap();

        // v2.1: one token per record, each tag encrypted on its own as plain text
        let record = json!({
            "type": "note",
            "data": {
                "id": uuid::Uuid::new_v4().to_string(),
                "createdAt": "2025-01-01T10:00:00+00:00",
                "modifiedAt": "2025-01-02T10:00:00+00:00",
                "syncedAt": null,
                "content": encrypted_string(&crypto, "From an older TUI", &key).unwrap(),
                "tags": [encrypted_string(&crypto, "old-tag", &key).unwrap()],
                "attachments": [],
                "pinned": false,
                "deleted": false,
                "deletedAt": null,
                "syncHash": null,
                "version": 3,
                "wordWrap": true,
                "syntaxLanguage": "plain",
                "archived": false,
                "archivedAt": null,
                "locked": false,
                "lockedAt": null,
                "color": null,
                "contentHash": null,
                "parentHash": null,
                "hashChain": null,
            },
        });
        let backup = BackupData {
            version: "2.1".to_string(),
            backup_type: BACKUP_TYPE.to_string(),
            created_at: Utc::now().to_rfc3339(),
            encryption: serde_json::from_value(json!({
                "salt": STANDARD.encode(&salt),
                "iterations": 100_000,
                "algorithm": "PBKDF2",
            })).unwrap(),
            data: vec![jwe_without_aad(record.to_string().as_bytes(), &key)],
            batches: Vec::new(),
            summary: None,
            skipped: 0,
        };

        let db = empty_database();
        assert_eq!(restore_with(&db, &backup, &password).notes, 1);

        let notes = notes_in(&db, &key);
        assert_eq!(notes[0].content, "From an older TUI");
        assert_eq!(notes[0].tags, vec!["old-tag"]);
        assert_eq!(notes[0].version, 3);
    }

    #[test]
    fn a_web_note_with_fields_left_out_restores() {
        let crypto = CryptoService::new();
        let key = crypto.generate_master_key();
        let tags = serde_json::to_string(&crypto.encrypt_json(&vec!["a", "b"], &key).unwrap()).unwrap();
        let item = json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "createdAt": "2025-01-01T10:00:00Z",
            "modifiedAt": "2025-01-02T10:00:00Z",
            "content": encrypted_string(&crypto, "From the web", &key).unwrap(),
            "tags": [tags],
            "attachments": [],
        });

        let (note, lost) = read_note(&crypto, item, &key).unwrap();

        assert_eq!(note.tags, vec!["a", "b"]);
        assert_eq!(note.version, 1);
        assert!(note.word_wrap);
        assert_eq!(lost, 0);
    }

    #[test]
    fn tags_and_attachments_written_as_strings_by_ios_restore() {
        let crypto = CryptoService::new();
        let key = crypto.generate_master_key();
        let tags = serde_json::to_string(&crypto.encrypt_json(&vec!["ios"], &key).unwrap()).unwrap();
        let attachments = json!([{
            "id": "a",
            "filename": encrypted_string(&crypto, "photo.heic", &key).unwrap(),
            "mimeType": "image/heic",
            "size": 3,
            "data": "a",
        }]).to_string();
        let item = json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "createdAt": "2025-01-01T10:00:00Z",
            "modifiedAt": "2025-01-02T10:00:00Z",
            "content": encrypted_string(&crypto, "From iOS", &key).unwrap(),
            "tags": tags,
            "attachments": attachments,
        });

        let (note, _) = read_note(&crypto, item, &key).unwrap();

        assert_eq!(note.tags, vec!["ios"]);
        assert_eq!(note.attachments[0].filename, "photo.heic");
    }

    #[test]
    fn restoring_into_a_vault_keeps_its_key_and_its_notes() {
        let (source, source_key, source_password) = envelope_vault();
        let backup = create_backup(&source, &source_key, None).unwrap();

        let (target, target_key, target_password) = envelope_vault();
        let existing = Note::new("Already here".to_string());
        NoteRepository::new(target.connection()).create(&existing, &target_key).unwrap();
        let before = EncryptionRepository::new(target.connection()).get().unwrap().unwrap();

        let keys = restore_keys(&target, &backup, &source_password, &target_password).unwrap();
        let result = restore_backup(&target, &backup, &keys, None).unwrap();

        assert_eq!(result.notes, 1);
        assert_eq!(result.failed, 0);

        // The metadata is untouched, and everything opens with the vault's own key
        let after = EncryptionRepository::new(target.connection()).get().unwrap().unwrap();
        assert_eq!(after.local_wrapped_master, before.local_wrapped_master);
        let crypto = CryptoService::new();
        assert_eq!(vault::unlock(target.connection(), &crypto, &target_password).unwrap(), target_key);

        let list = NoteRepository::new(target.connection()).list_everything(&target_key).unwrap();
        assert_eq!(list.unreadable, 0);
        assert_eq!(list.notes.len(), 3);
    }

    #[test]
    fn the_wrong_vault_password_stops_a_restore_before_it_starts() {
        let (source, source_key, source_password) = envelope_vault();
        let backup = create_backup(&source, &source_key, None).unwrap();
        let (target, target_key, _) = envelope_vault();

        assert!(restore_keys(&target, &backup, &source_password, &random_password()).is_err());

        assert_eq!(notes_in(&target, &target_key).len(), 1);
    }

    #[test]
    fn a_separate_local_password_opens_the_restored_vault() {
        let (source, key, backup_password) = envelope_vault();
        let backup = create_backup(&source, &key, None).unwrap();
        let (db, local_password) = (empty_database(), random_password());

        let keys = restore_keys(&db, &backup, &backup_password, &local_password).unwrap();
        restore_backup(&db, &backup, &keys, None).unwrap();

        let crypto = CryptoService::new();
        assert_eq!(vault::unlock(db.connection(), &crypto, &local_password).unwrap(), key);
        assert_eq!(notes_in(&db, &key)[0].content, "Kept safe in a backup");
    }

    #[test]
    fn a_short_local_password_is_refused_for_a_new_vault() {
        let (source, key, backup_password) = envelope_vault();
        let backup = create_backup(&source, &key, None).unwrap();

        assert!(restore_keys(&empty_database(), &backup, &backup_password, &"x".repeat(5)).is_err());
    }

    #[test]
    fn a_damaged_batch_is_counted_and_the_rest_restores() {
        let (db, key, password) = envelope_vault();
        add_full_note(&db, &key);
        let mut backup = create_backup(&db, &key, None).unwrap();

        // Corrupt the versions, keeping the notes that verify_backup_password reads
        let versions = backup.batches.iter_mut().find(|b| b.batch_type == "versions").unwrap();
        versions.data = encrypt_as_jwe(b"not compressed json", &key).unwrap();

        let restored = empty_database();
        let result = restore_with(&restored, &backup, &password);

        assert_eq!(result.notes, 2);
        assert_eq!(result.versions, 0);
        assert_eq!(result.failed, 1);
    }

    #[test]
    fn a_note_that_cannot_be_read_makes_the_backup_say_so() {
        let (db, key, _) = envelope_vault();
        let other_key = CryptoService::new().generate_master_key();
        NoteRepository::new(db.connection()).create(&Note::new("Lost".to_string()), &other_key).unwrap();

        let backup = create_backup(&db, &key, None).unwrap();

        assert_eq!(backup.skipped, 1);
    }

    #[test]
    fn settings_leave_secrets_and_sync_behind() {
        let mut settings = UserSettings::default();
        settings.sync_endpoint = Some("https://example.com".to_string());

        let item = settings_for_backup(&settings).to_string();

        assert!(!item.contains("example.com"));
    }

    #[test]
    fn settings_from_the_web_do_not_take_over_the_theme() {
        let db = empty_database();
        let repo = SettingsRepository::new(db.connection());
        let before = repo.get().unwrap().theme;

        let web = json!({ "language": "fr", "theme": "auto", "sortOrder": "oldest", "autoLockTimeout": 30 });
        restore_settings(&repo, &web).unwrap();

        let settings = repo.get().unwrap();
        assert_eq!(settings.language, "fr");
        assert_eq!(settings.auto_lock_timeout, 30);
        assert_eq!(settings.sort_order, crate::models::SortOrder::Oldest);
        assert_eq!(settings.theme.to_string(), before.to_string());
    }

    #[test]
    fn a_v2_record_token_still_decrypts() {
        let key = CryptoService::new().generate_master_key();
        let jwe = encrypt_as_jwe(json!({"type": "settings", "data": {"test": "value"}}).to_string().as_bytes(), &key).unwrap();

        let parts: Vec<&str> = jwe.split('.').collect();
        assert_eq!(parts.len(), 5);

        let plaintext = decrypt_jwe(&jwe, &key).unwrap();
        let record: Value = serde_json::from_slice(&plaintext).unwrap();
        assert_eq!(record["data"]["test"], "value");
    }
}
