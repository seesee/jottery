//! Getting from a password to the key a vault's notes are encrypted with.
//!
//! Plain SQLite has no password of its own, so nothing else stands between a
//! wrong password and a vault that opens, looks empty and saves new notes under
//! the wrong key. Every path that unlocks a vault goes through here.

use anyhow::{Context, Result};
use base64::Engine as _;
use rusqlite::Connection;
use rust_i18n::t;

use crate::crypto::{CryptoService, EncryptedData, DEFAULT_ITERATIONS, WRAPPING_KDF_VERSION};
use crate::repository::encryption::EncryptionMetadata;
use crate::repository::{sync::SyncRepository, EncryptionRepository};

/// The password is wrong, as opposed to the vault being out of reach for now.
#[derive(Debug, thiserror::Error)]
#[error("{}", t!("password.incorrect"))]
pub struct WrongPassword;

/// Whether an error from unlocking means the password is wrong. Plain SQLite takes any
/// password and the vault's own check says so. SQLCipher fails to read the file.
pub fn is_wrong_password(error: &anyhow::Error) -> bool {
    if error.downcast_ref::<WrongPassword>().is_some() {
        return true;
    }

    matches!(
        error.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(failure, _)) if failure.code == rusqlite::ErrorCode::NotADatabase,
    )
}

/// Shortest notes password accepted when one is being set. Unlocking has no
/// minimum, because existing vaults can have shorter ones.
pub const MIN_NOTES_PASSWORD_CHARS: usize = 8;

/// Refuse a new notes password that's too short.
pub fn check_new_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_NOTES_PASSWORD_CHARS {
        anyhow::bail!("{}", t!("password.too_short", min = MIN_NOTES_PASSWORD_CHARS));
    }
    Ok(())
}

/// Refuse a notes password equal to the account password.
///
/// The server receives the account password in plaintext, and it knows the userId
/// that salts the wrapped master key. If the two match it can unwrap the key.
pub fn check_passwords_differ(notes_password: &str, account_password: &str) -> Result<()> {
    if notes_password == account_password {
        anyhow::bail!("{}", t!("password.same_as_account"));
    }
    Ok(())
}

/// Wrap `master_key` for this device, replacing whatever was stored.
pub fn store_local_envelope(
    conn: &Connection,
    crypto: &CryptoService,
    password: &str,
    master_key: &[u8; 32],
) -> Result<()> {
    let device_salt = crypto.generate_salt();
    let device_key = crypto.derive_key(password, &device_salt, DEFAULT_ITERATIONS)?;
    let wrapped = crypto.wrap_master_key(&device_key, master_key)?;

    EncryptionRepository::new(conn).save_envelope(
        1,
        &base64::engine::general_purpose::STANDARD.encode(&device_salt),
        &serde_json::to_string(&wrapped)?,
        WRAPPING_KDF_VERSION,
    )
}

/// The key `password` leads to under this metadata, if the metadata can tell.
fn key_for_password(
    crypto: &CryptoService,
    metadata: &EncryptionMetadata,
    password: &str,
) -> Result<[u8; 32]> {
    if metadata.envelope_version.is_some() {
        let device_salt = metadata.device_salt.as_deref()
            .ok_or_else(|| anyhow::anyhow!("Envelope metadata missing device_salt"))?;
        let device_salt = base64::engine::general_purpose::STANDARD.decode(device_salt)
            .context("Invalid device_salt base64")?;
        let wrapped = metadata.local_wrapped_master.as_deref()
            .ok_or_else(|| anyhow::anyhow!("Envelope metadata missing local_wrapped_master"))?;
        let wrapped: EncryptedData = serde_json::from_str(wrapped)
            .context("Invalid local_wrapped_master JSON")?;

        let device_key = crypto.derive_key(password, &device_salt, DEFAULT_ITERATIONS)?;
        crypto.unwrap_master_key(&device_key, &wrapped)
            .map_err(|_| WrongPassword.into())
    } else if let (Some(salt), Some(iterations)) = (&metadata.salt, metadata.iterations) {
        crypto.derive_key(password, salt, iterations)
    } else {
        anyhow::bail!("Invalid encryption metadata: neither envelope nor legacy fields found");
    }
}

/// What a vault's contents say about a key.
#[derive(Debug, PartialEq, Eq)]
enum ContentsCheck {
    Opens,
    Refuses,
    /// Nothing here is encrypted, so any key would do
    NothingToTry,
}

/// Try `key` on what's in the vault.
///
/// A legacy vault stores no verifier, as its key comes straight from the password,
/// so the only test is its contents. One readable row is enough, since a vault can
/// hold a row that's damaged.
fn check_contents(conn: &Connection, crypto: &CryptoService, key: &[u8; 32]) -> Result<ContentsCheck> {
    let opens = |stored: &str| {
        serde_json::from_str::<EncryptedData>(stored)
            .is_ok_and(|encrypted| crypto.decrypt_text(&encrypted, key).is_ok())
    };

    let mut stmt = conn.prepare("SELECT content FROM notes")?;
    let mut rows = stmt.query([])?;
    let mut has_notes = false;
    while let Some(row) = rows.next()? {
        has_notes = true;
        if opens(&row.get::<_, String>(0)?) {
            return Ok(ContentsCheck::Opens);
        }
    }
    if has_notes {
        return Ok(ContentsCheck::Refuses);
    }

    // No notes, but a stored API key is encrypted with the same key. The prefixed
    // forms are credentials waiting to be processed, which aren't under this key.
    let api_key = SyncRepository::new(conn).get_metadata()?.and_then(|metadata| metadata.api_key);
    Ok(match api_key {
        Some(api_key) if serde_json::from_str::<EncryptedData>(&api_key).is_ok() => {
            if opens(&api_key) { ContentsCheck::Opens } else { ContentsCheck::Refuses }
        }
        _ => ContentsCheck::NothingToTry,
    })
}

/// Create a vault: a random master key, wrapped with the password.
///
/// Wrapping is what makes a wrong password detectable from now on.
fn create(conn: &Connection, crypto: &CryptoService, password: &str) -> Result<[u8; 32]> {
    check_new_password(password)?;

    let master_key = crypto.generate_master_key();
    store_local_envelope(conn, crypto, password, &master_key)?;
    Ok(master_key)
}

/// Get the vault's key from its password, creating the vault if there isn't one.
pub fn unlock(conn: &Connection, crypto: &CryptoService, password: &str) -> Result<[u8; 32]> {
    let Some(metadata) = EncryptionRepository::new(conn).get()? else {
        return create(conn, crypto, password);
    };

    let key = key_for_password(crypto, &metadata, password)?;

    if metadata.envelope_version.is_some() {
        return Ok(key);
    }

    match check_contents(conn, crypto, &key)? {
        ContentsCheck::Refuses => Err(WrongPassword.into()),
        ContentsCheck::Opens => {
            upgrade_legacy_vault(conn, crypto, password, &key);
            Ok(key)
        }
        // Any password opens an empty legacy vault, so this one proves nothing, and
        // making it the vault's password could lock its owner out
        ContentsCheck::NothingToTry => Ok(key),
    }
}

/// Wrap a legacy vault's key now the password is known to be right.
///
/// Its key stays, so nothing needs re-encrypting and every backup still opens. The salt
/// goes, so the file no longer offers a way to test passwords at the old iteration count,
/// and a wrong password can be told from a right one even if the vault is ever empty.
/// A vault that can't be upgraded still opens, and tries again next time.
fn upgrade_legacy_vault(conn: &Connection, crypto: &CryptoService, password: &str, key: &[u8; 32]) {
    if let Err(e) = store_local_envelope(conn, crypto, password, key) {
        tracing::warn!("Could not upgrade this vault's key wrapping: {e}");
    }
}

/// Whether `password` is the password of the open vault whose key is `key`.
pub fn password_matches(
    conn: &Connection,
    crypto: &CryptoService,
    password: &str,
    key: &[u8; 32],
) -> Result<bool> {
    let metadata = EncryptionRepository::new(conn).get()?
        .ok_or_else(|| anyhow::anyhow!("No encryption metadata found"))?;

    Ok(key_for_password(crypto, &metadata, password).is_ok_and(|derived| derived == *key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::{sync::SyncMetadata, Note};
    use crate::repository::NoteRepository;

    fn password() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn empty_database() -> Database {
        Database::in_memory(&password()).unwrap()
    }

    /// A vault as older builds made it: the key comes straight from the password.
    fn legacy_vault(password: &str) -> (Database, [u8; 32]) {
        let db = empty_database();
        let crypto = CryptoService::new();
        let salt = crypto.generate_salt();
        EncryptionRepository::new(db.connection()).save(&salt, 100_000).unwrap();
        let key = crypto.derive_key(password, &salt, 100_000).unwrap();
        (db, key)
    }

    #[test]
    fn a_new_vault_refuses_the_wrong_password() {
        let db = empty_database();
        let crypto = CryptoService::new();
        let right = password();

        let key = unlock(db.connection(), &crypto, &right).unwrap();

        assert_eq!(unlock(db.connection(), &crypto, &right).unwrap(), key);
        assert!(unlock(db.connection(), &crypto, &password()).is_err());
    }

    #[test]
    fn a_new_vault_wraps_a_random_key() {
        let db = empty_database();
        let crypto = CryptoService::new();
        let password = password();

        let key = unlock(db.connection(), &crypto, &password).unwrap();
        let metadata = EncryptionRepository::new(db.connection()).get().unwrap().unwrap();

        assert_eq!(metadata.envelope_version, Some(1));

        // Another vault with the same password has a different key
        let other = unlock(empty_database().connection(), &crypto, &password).unwrap();
        assert_ne!(key, other);
    }

    #[test]
    fn a_legacy_vault_refuses_the_wrong_password() {
        let right = password();
        let (db, key) = legacy_vault(&right);
        NoteRepository::new(db.connection()).create(&Note::new("Secret".to_string()), &key).unwrap();

        let crypto = CryptoService::new();
        assert_eq!(unlock(db.connection(), &crypto, &right).unwrap(), key);
        assert!(unlock(db.connection(), &crypto, &password()).is_err());
    }

    #[test]
    fn a_legacy_vault_is_wrapped_at_the_next_unlock() {
        let right = password();
        let (db, key) = legacy_vault(&right);
        let notes = NoteRepository::new(db.connection());
        notes.create(&Note::new("Secret".to_string()), &key).unwrap();
        let crypto = CryptoService::new();

        assert_eq!(unlock(db.connection(), &crypto, &right).unwrap(), key);

        let metadata = EncryptionRepository::new(db.connection()).get().unwrap().unwrap();
        assert_eq!(metadata.envelope_version, Some(1));
        assert_eq!(metadata.salt, None);
        assert_eq!(metadata.iterations, None);

        // The same key, so the notes still open, and the password is still the password
        assert_eq!(unlock(db.connection(), &crypto, &right).unwrap(), key);
        assert_eq!(notes.list(false, &key).unwrap().len(), 1);
        assert!(unlock(db.connection(), &crypto, &password()).is_err());
    }

    #[test]
    fn the_wrong_password_never_upgrades_a_legacy_vault() {
        let right = password();
        let (db, key) = legacy_vault(&right);
        NoteRepository::new(db.connection()).create(&Note::new("Secret".to_string()), &key).unwrap();

        assert!(unlock(db.connection(), &CryptoService::new(), &password()).is_err());

        let metadata = EncryptionRepository::new(db.connection()).get().unwrap().unwrap();
        assert_eq!(metadata.envelope_version, None);
    }

    /// Any password opens an empty legacy vault. Wrapping would make a typo the password.
    #[test]
    fn an_empty_legacy_vault_waits_for_something_to_prove_the_password() {
        let right = password();
        let (db, _) = legacy_vault(&right);

        unlock(db.connection(), &CryptoService::new(), &password()).unwrap();

        let metadata = EncryptionRepository::new(db.connection()).get().unwrap().unwrap();
        assert_eq!(metadata.envelope_version, None);
    }

    #[test]
    fn one_damaged_note_does_not_lock_a_legacy_vault() {
        let right = password();
        let (db, key) = legacy_vault(&right);
        let notes = NoteRepository::new(db.connection());
        let other_key = CryptoService::new().generate_master_key();
        notes.create(&Note::new("Damaged".to_string()), &other_key).unwrap();
        notes.create(&Note::new("Fine".to_string()), &key).unwrap();

        assert_eq!(unlock(db.connection(), &CryptoService::new(), &right).unwrap(), key);
    }

    #[test]
    fn an_empty_legacy_vault_with_an_api_key_checks_that_instead() {
        let right = password();
        let (db, key) = legacy_vault(&right);
        let crypto = CryptoService::new();
        let api_key = crypto.encrypt_text("device-api-key", &key).unwrap();
        let metadata = SyncMetadata {
            api_key: Some(serde_json::to_string(&api_key).unwrap()),
            ..Default::default()
        };
        SyncRepository::new(db.connection()).update_metadata(&metadata).unwrap();

        assert!(unlock(db.connection(), &crypto, &right).is_ok());
        assert!(unlock(db.connection(), &crypto, &password()).is_err());
    }

    #[test]
    fn a_short_password_opens_an_existing_vault_but_cannot_start_one() {
        let crypto = CryptoService::new();
        let short = "x".repeat(7);
        let short = short.as_str();

        assert!(unlock(empty_database().connection(), &crypto, short).is_err());

        let (db, _) = legacy_vault(short);
        assert!(unlock(db.connection(), &crypto, short).is_ok());

        let db = empty_database();
        let key = crypto.generate_master_key();
        store_local_envelope(db.connection(), &crypto, short, &key).unwrap();
        assert_eq!(unlock(db.connection(), &crypto, short).unwrap(), key);
    }

    #[test]
    fn the_password_rule_counts_characters_not_bytes() {
        assert!(check_new_password(&"パ".repeat(5)).is_err());
        assert!(check_new_password(&"パ".repeat(9)).is_ok());
    }

    #[test]
    fn a_notes_password_must_differ_from_the_account_password() {
        let (notes, account) = (password(), password());
        assert!(check_passwords_differ(&notes, &notes).is_err());
        assert!(check_passwords_differ(&notes, &account).is_ok());
    }

    #[test]
    fn password_matches_tells_the_right_password_from_the_wrong_one() {
        let crypto = CryptoService::new();

        let right = password();
        let db = empty_database();
        let key = unlock(db.connection(), &crypto, &right).unwrap();
        assert!(password_matches(db.connection(), &crypto, &right, &key).unwrap());
        assert!(!password_matches(db.connection(), &crypto, &password(), &key).unwrap());

        let (db, key) = legacy_vault(&right);
        assert!(password_matches(db.connection(), &crypto, &right, &key).unwrap());
        assert!(!password_matches(db.connection(), &crypto, &password(), &key).unwrap());
    }
}
