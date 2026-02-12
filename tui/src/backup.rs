//! Encrypted backup functionality (compatible with web app v2.1 format)
//!
//! Creates JWE-encrypted backups where each record is a JWE compact serialization token.
//! Format matches the web app's backup format for cross-platform compatibility.

use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use crate::{
    crypto::{CryptoService, EncryptedData},
    db::Database,
    models::Note,
    repository::{NoteRepository, EncryptionRepository, SettingsRepository},
};

const BACKUP_VERSION: &str = "2.1";
const BACKUP_TYPE: &str = "jottery-encrypted-backup";

/// JWE header for direct encryption with AES-256-GCM
#[derive(Debug, Serialize, Deserialize)]
struct JweHeader {
    alg: String,
    enc: String,
}

/// Backup file structure (matches web app v2.1)
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupData {
    pub version: String,
    #[serde(rename = "type")]
    pub backup_type: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub encryption: BackupEncryption,
    pub data: Vec<String>, // JWE compact serialization strings
}

/// Encryption metadata in backup
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupEncryption {
    pub salt: String,
    pub iterations: i64,
    pub algorithm: String,
}

/// Record types in backup
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[allow(clippy::large_enum_variant)]
enum BackupRecord {
    #[serde(rename = "note")]
    Note(BackupNote),
    #[serde(rename = "settings")]
    Settings(serde_json::Value),
    #[serde(rename = "version")]
    Version(serde_json::Value),
    #[serde(rename = "attachment")]
    Attachment(serde_json::Value),
    #[serde(rename = "saved_search")]
    SavedSearch(serde_json::Value),
    #[serde(rename = "sync_metadata")]
    SyncMetadata(serde_json::Value),
}

/// Note structure in backup (encrypted fields stored as-is from database)
#[derive(Debug, Serialize, Deserialize)]
struct BackupNote {
    id: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "modifiedAt")]
    modified_at: String,
    #[serde(rename = "syncedAt")]
    synced_at: Option<String>,
    content: String,
    tags: Vec<String>,
    attachments: Vec<serde_json::Value>,
    pinned: bool,
    deleted: bool,
    #[serde(rename = "deletedAt")]
    deleted_at: Option<String>,
    #[serde(rename = "syncHash")]
    sync_hash: Option<String>,
    version: i32,
    #[serde(rename = "wordWrap")]
    word_wrap: Option<bool>,
    #[serde(rename = "syntaxLanguage")]
    syntax_language: Option<String>,
    archived: bool,
    #[serde(rename = "archivedAt")]
    archived_at: Option<String>,
    locked: bool,
    #[serde(rename = "lockedAt")]
    locked_at: Option<String>,
    color: Option<String>,
    #[serde(rename = "contentHash")]
    content_hash: Option<String>,
    #[serde(rename = "parentHash")]
    parent_hash: Option<String>,
    #[serde(rename = "hashChain")]
    hash_chain: Option<Vec<String>>,
}

/// Progress callback for backup operations
pub type ProgressCallback = Box<dyn Fn(usize, usize, &str)>;

/// Encrypt a record as JWE compact serialization
fn encrypt_as_jwe(record: &BackupRecord, key: &[u8; 32]) -> Result<String> {
    let crypto = CryptoService::new();

    // Serialize the record to JSON
    let plaintext = serde_json::to_string(record)?;

    // Encrypt using AES-256-GCM
    let encrypted = crypto.encrypt_text(&plaintext, key)?;

    // Build JWE compact serialization:
    // BASE64URL(header).BASE64URL(encrypted_key).BASE64URL(IV).BASE64URL(ciphertext).BASE64URL(tag)
    // For "dir" algorithm, encrypted_key is empty

    let header = JweHeader {
        alg: "dir".to_string(),
        enc: "A256GCM".to_string(),
    };
    let header_json = serde_json::to_string(&header)?;
    let header_b64 = URL_SAFE_NO_PAD.encode(header_json.as_bytes());

    // encrypted_key is empty for "dir" algorithm
    let encrypted_key_b64 = "";

    // The EncryptedData fields are base64 STANDARD encoded, we need URL_SAFE_NO_PAD for JWE
    // Decode from STANDARD and re-encode as URL_SAFE_NO_PAD
    let nonce_bytes = base64::engine::general_purpose::STANDARD.decode(&encrypted.nonce)
        .context("Failed to decode nonce")?;
    let nonce_b64 = URL_SAFE_NO_PAD.encode(&nonce_bytes);

    let ciphertext_bytes = base64::engine::general_purpose::STANDARD.decode(&encrypted.ciphertext)
        .context("Failed to decode ciphertext")?;

    // GCM includes tag in ciphertext, so tag field might be empty
    // For JWE compatibility, we need to split the ciphertext and tag
    // The last 16 bytes of ciphertext are the authentication tag
    let (ct, tag) = if ciphertext_bytes.len() >= 16 {
        ciphertext_bytes.split_at(ciphertext_bytes.len() - 16)
    } else {
        (ciphertext_bytes.as_slice(), &[] as &[u8])
    };
    let ciphertext_b64 = URL_SAFE_NO_PAD.encode(ct);
    let tag_b64 = URL_SAFE_NO_PAD.encode(tag);

    Ok(format!("{}.{}.{}.{}.{}", header_b64, encrypted_key_b64, nonce_b64, ciphertext_b64, tag_b64))
}

/// Decrypt a JWE compact serialization to a record
fn decrypt_jwe(jwe: &str, key: &[u8; 32]) -> Result<BackupRecord> {
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

    // Decode IV (nonce)
    let nonce_bytes = URL_SAFE_NO_PAD.decode(parts[2])
        .context("Failed to decode IV")?;

    // Decode ciphertext
    let ciphertext_bytes = URL_SAFE_NO_PAD.decode(parts[3])
        .context("Failed to decode ciphertext")?;

    // Decode tag
    let tag_bytes = URL_SAFE_NO_PAD.decode(parts[4])
        .context("Failed to decode tag")?;

    // Combine ciphertext and tag (GCM expects them together)
    let mut combined = ciphertext_bytes;
    combined.extend_from_slice(&tag_bytes);

    // Reconstruct EncryptedData with base64 STANDARD encoding (as expected by CryptoService)
    let encrypted = EncryptedData {
        nonce: base64::engine::general_purpose::STANDARD.encode(&nonce_bytes),
        ciphertext: base64::engine::general_purpose::STANDARD.encode(&combined),
        tag: String::new(), // Tag is included in ciphertext for GCM
    };

    // Decrypt
    let plaintext = crypto.decrypt_text(&encrypted, key)?;

    // Parse record
    let record: BackupRecord = serde_json::from_str(&plaintext)
        .context("Failed to parse decrypted record")?;

    Ok(record)
}

/// Create an encrypted backup of all data
pub fn create_backup(
    db: &Database,
    key: &[u8; 32],
    on_progress: Option<ProgressCallback>,
) -> Result<BackupData> {
    let note_repo = NoteRepository::new(db.connection());
    let encryption_repo = EncryptionRepository::new(db.connection());
    let settings_repo = SettingsRepository::new(db.connection());
    let crypto = CryptoService::new();

    // Get encryption metadata
    let encryption_meta = encryption_repo.get()?
        .ok_or_else(|| anyhow!("No encryption metadata found"))?;

    // Count total items
    let notes = note_repo.list(true, key)?; // Include deleted notes
    let total = notes.len() + 1; // +1 for settings
    let mut current = 0;

    let mut jwe_records = Vec::new();

    // Encrypt all notes
    for note in &notes {
        // Re-encrypt content and tags for backup format
        // The note content is already decrypted, so we need to encrypt it
        let encrypted_content = crypto.encrypt_text(&note.content, key)?;
        let content_json = serde_json::to_string(&encrypted_content)?;

        // Encrypt tags
        let encrypted_tags: Vec<String> = note.tags.iter()
            .map(|tag| {
                let encrypted = crypto.encrypt_text(tag, key)?;
                Ok(serde_json::to_string(&encrypted)?)
            })
            .collect::<Result<Vec<_>>>()?;

        let backup_note = BackupNote {
            id: note.id.clone(),
            created_at: note.created_at.to_rfc3339(),
            modified_at: note.modified_at.to_rfc3339(),
            synced_at: note.synced_at.map(|dt| dt.to_rfc3339()),
            content: content_json,
            tags: encrypted_tags,
            attachments: vec![], // Attachments not supported in TUI yet
            pinned: note.pinned,
            deleted: note.deleted,
            deleted_at: note.deleted_at.map(|dt| dt.to_rfc3339()),
            sync_hash: note.sync_hash.clone(),
            version: note.version,
            word_wrap: Some(note.word_wrap),
            syntax_language: Some(note.syntax_language.to_string()),
            archived: note.archived,
            archived_at: note.archived_at.map(|dt| dt.to_rfc3339()),
            locked: note.locked,
            locked_at: note.locked_at.map(|dt| dt.to_rfc3339()),
            color: note.color.clone(),
            content_hash: note.content_hash.clone(),
            parent_hash: note.parent_hash.clone(),
            hash_chain: note.hash_chain.clone(),
        };

        let record = BackupRecord::Note(backup_note);
        let jwe = encrypt_as_jwe(&record, key)?;
        jwe_records.push(jwe);

        current += 1;
        if let Some(ref callback) = on_progress {
            callback(current, total, "notes");
        }
    }

    // Encrypt settings
    let settings = settings_repo.get()?;
    let settings_json = serde_json::to_value(&settings)?;
    let settings_record = BackupRecord::Settings(settings_json);
    let settings_jwe = encrypt_as_jwe(&settings_record, key)?;
    jwe_records.push(settings_jwe);

    current += 1;
    if let Some(ref callback) = on_progress {
        callback(current, total, "settings");
    }

    // Build backup data
    let backup = BackupData {
        version: BACKUP_VERSION.to_string(),
        backup_type: BACKUP_TYPE.to_string(),
        created_at: Utc::now().to_rfc3339(),
        encryption: BackupEncryption {
            salt: base64::engine::general_purpose::STANDARD.encode(&encryption_meta.salt),
            iterations: encryption_meta.iterations as i64,
            algorithm: "PBKDF2".to_string(),
        },
        data: jwe_records,
    };

    Ok(backup)
}

/// Write backup to file using chunked JSON generation
pub fn write_backup<P: AsRef<Path>>(backup: &BackupData, path: P) -> Result<()> {
    let file = File::create(path.as_ref())
        .context("Failed to create backup file")?;
    let mut writer = BufWriter::new(file);

    // Write JSON in chunks to handle large backups
    write!(writer, r#"{{"version":"#)?;
    serde_json::to_writer(&mut writer, &backup.version)?;
    write!(writer, r#","type":"#)?;
    serde_json::to_writer(&mut writer, &backup.backup_type)?;
    write!(writer, r#","createdAt":"#)?;
    serde_json::to_writer(&mut writer, &backup.created_at)?;
    write!(writer, r#","encryption":"#)?;
    serde_json::to_writer(&mut writer, &backup.encryption)?;
    write!(writer, r#","data":["#)?;

    for (i, jwe) in backup.data.iter().enumerate() {
        if i > 0 {
            write!(writer, ",")?;
        }
        serde_json::to_writer(&mut writer, jwe)?;
    }

    write!(writer, "]}}")?;
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

    if major > 2 || (major == 2 && minor > 1) {
        return Err(anyhow!("Backup version {} is not supported. Please update Jottery.", backup.version));
    }

    // Validate JWE format (5 parts separated by dots)
    for (i, jwe) in backup.data.iter().enumerate() {
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

    // Decode salt from backup
    let salt = base64::engine::general_purpose::STANDARD.decode(&backup.encryption.salt)
        .context("Failed to decode salt from backup")?;

    // Derive key using backup's encryption parameters
    let key = crypto.derive_key(password, &salt, backup.encryption.iterations as u32)?;

    // Try to decrypt first record to verify password
    if !backup.data.is_empty() {
        decrypt_jwe(&backup.data[0], &key)
            .context("Incorrect password")?;
    }

    Ok(key)
}

/// Restore data from a backup
pub fn restore_backup(
    db: &Database,
    backup: &BackupData,
    key: &[u8; 32],
    on_progress: Option<ProgressCallback>,
) -> Result<usize> {
    let note_repo = NoteRepository::new(db.connection());
    let encryption_repo = EncryptionRepository::new(db.connection());
    let settings_repo = SettingsRepository::new(db.connection());
    let crypto = CryptoService::new();

    // First, restore encryption metadata
    let salt = base64::engine::general_purpose::STANDARD.decode(&backup.encryption.salt)?;
    encryption_repo.save(&salt, backup.encryption.iterations as u32)?;

    let total = backup.data.len();
    let mut restored = 0;

    for (i, jwe) in backup.data.iter().enumerate() {
        let record = decrypt_jwe(jwe, key)?;

        match record {
            BackupRecord::Note(backup_note) => {
                // Decrypt content and tags
                let content_encrypted: EncryptedData = serde_json::from_str(&backup_note.content)?;
                let content = crypto.decrypt_text(&content_encrypted, key)?;

                let tags: Vec<String> = backup_note.tags.iter()
                    .map(|tag_json| {
                        let tag_encrypted: EncryptedData = serde_json::from_str(tag_json)?;
                        crypto.decrypt_text(&tag_encrypted, key)
                    })
                    .collect::<Result<Vec<_>>>()?;

                // Parse dates
                let created_at = backup_note.created_at.parse()?;
                let modified_at = backup_note.modified_at.parse()?;
                let synced_at = backup_note.synced_at.map(|s| s.parse()).transpose()?;
                let deleted_at = backup_note.deleted_at.map(|s| s.parse()).transpose()?;
                let archived_at = backup_note.archived_at.map(|s| s.parse()).transpose()?;
                let locked_at = backup_note.locked_at.map(|s| s.parse()).transpose()?;

                let note = Note {
                    id: backup_note.id,
                    created_at,
                    modified_at,
                    synced_at,
                    content,
                    tags,
                    attachments: vec![], // Attachments not supported yet
                    pinned: backup_note.pinned,
                    deleted: backup_note.deleted,
                    deleted_at,
                    sync_hash: backup_note.sync_hash,
                    version: backup_note.version,
                    word_wrap: backup_note.word_wrap.unwrap_or(true),
                    syntax_language: backup_note.syntax_language
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_default(),
                    archived: backup_note.archived,
                    archived_at,
                    locked: backup_note.locked,
                    locked_at,
                    color: backup_note.color,
                    content_hash: backup_note.content_hash,
                    parent_hash: backup_note.parent_hash,
                    hash_chain: backup_note.hash_chain,
                };

                // Try to create, if exists try to update
                match note_repo.create(&note, key) {
                    Ok(_) => restored += 1,
                    Err(_) => {
                        if note_repo.update(&note, key).is_ok() {
                            restored += 1;
                        }
                    }
                }
            }
            BackupRecord::Settings(settings_value) => {
                // Restore settings
                if let Ok(settings) = serde_json::from_value(settings_value) {
                    let _ = settings_repo.update(&settings);
                }
            }
            _ => {
                // Skip other record types for now (versions, attachments, etc.)
            }
        }

        if let Some(ref callback) = on_progress {
            callback(i + 1, total, "records");
        }
    }

    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// Get test password from environment variable or use default.
    /// Set JOTTERY_TEST_PASSWORD env var to override.
    fn test_password() -> String {
        std::env::var("JOTTERY_TEST_PASSWORD")
            .unwrap_or_else(|_| "test_password_placeholder".to_string())
    }

    #[test]
    fn test_backup_restore_roundtrip() {
        let password = test_password();
        let crypto = CryptoService::new();
        let salt = crypto.generate_salt();
        let key = crypto.derive_key(&password, &salt, 100_000).unwrap();

        // Create temporary database
        let db = Database::in_memory(&password).unwrap();
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
        assert_eq!(backup.version, "2.1");
        assert!(!backup.data.is_empty());

        // Write to file
        let backup_file = NamedTempFile::new().unwrap();
        write_backup(&backup, backup_file.path()).unwrap();

        // Validate backup
        let validated = validate_backup(backup_file.path()).unwrap();
        assert_eq!(validated.version, "2.1");

        // Verify password
        let verified_key = verify_backup_password(&validated, &password).unwrap();
        assert_eq!(verified_key, key);

        // Create new database and restore
        let db2 = Database::in_memory(&password).unwrap();
        let restored = restore_backup(&db2, &validated, &key, None).unwrap();
        assert_eq!(restored, 2);

        // Verify notes were restored
        let note_repo2 = NoteRepository::new(db2.connection());
        let notes = note_repo2.list(false, &key).unwrap();
        assert_eq!(notes.len(), 2);
    }

    #[test]
    fn test_jwe_roundtrip() {
        let crypto = CryptoService::new();
        let salt = crypto.generate_salt();
        let key = crypto.derive_key("test", &salt, 1000).unwrap();

        let record = BackupRecord::Settings(serde_json::json!({"test": "value"}));

        let jwe = encrypt_as_jwe(&record, &key).unwrap();

        // Verify JWE format
        let parts: Vec<&str> = jwe.split('.').collect();
        assert_eq!(parts.len(), 5);

        // Decrypt and verify
        let decrypted = decrypt_jwe(&jwe, &key).unwrap();
        match decrypted {
            BackupRecord::Settings(v) => {
                assert_eq!(v["test"], "value");
            }
            _ => panic!("Expected Settings record"),
        }
    }
}
