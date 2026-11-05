//! Envelope encryption operations
//!
//! Handles a device joining an account, keeping the server's wrapped copy of the
//! master key usable, and changing the notes password.
//!
//! The server holds the master key wrapped with the notes password and the userId.
//! It also receives the account password in plaintext, so a blob opened by that
//! password proves nothing: the server could have wrapped a key of its own choosing.
//! Only the notes password may unwrap a key this device adopts.
//!
//! Each device remembers whether it has ever opened or uploaded that blob, which is how
//! it tells an old-format blob it may repair from one another device has re-wrapped
//! after a password change, which it must leave alone.

use anyhow::{Context, Result};
use rusqlite::Connection;
use rust_i18n::t;

use crate::{
    api::{AuthClient, KeyWrite, KeyWriteResult, PutWrappedKeyRequest},
    crypto::{CryptoService, WRAPPING_ITERATIONS, WRAPPING_KDF_VERSION},
    db::Database,
    models::{encryption::EncryptedData, sync::SyncMetadata},
    repository::{rekey::rekey_vault, sync::SyncRepository},
    vault,
};

use super::super::app::App;
use super::sync::SyncLink;

/// How many times to read the key again after losing a race for it. Each loss means
/// another device has just written, so a few rounds is plenty.
const KEY_WRITE_ATTEMPTS: usize = 3;

/// An account this device can talk to.
pub struct ServerLink<'a> {
    pub client: &'a AuthClient,
    pub api_key: &'a str,
    pub user_id: &'a str,
}

/// What the server's wrapped key turned out to be, for a device that already has a key.
#[derive(Debug, PartialEq, Eq)]
enum ServerKey {
    /// The server has no wrapped key
    Missing,
    /// It unwraps with the notes password, to this device's key
    Matches,
    /// It unwraps with the notes password, to a different key
    Differs,
    /// It doesn't unwrap with the notes password
    Unreadable,
    /// It's wrapped in a way this version doesn't know
    Unknown,
}

/// Unwrap a blob from the server. This is the only way a server blob becomes a key.
fn unwrap_server_key(
    crypto: &CryptoService,
    notes_password: &str,
    user_id: &str,
    blob: &str,
) -> Result<[u8; 32]> {
    let wrapped: EncryptedData = serde_json::from_str(blob).context("Invalid wrapped key blob JSON")?;
    let wrapping_key = crypto.derive_wrapping_key(notes_password, user_id)?;
    crypto.unwrap_master_key(&wrapping_key, &wrapped)
}

/// How a blob the server sent relates to this device's key, under this password.
fn compare_with_master_key(
    crypto: &CryptoService,
    notes_password: &str,
    user_id: &str,
    blob: &str,
    master_key: &[u8; 32],
) -> ServerKey {
    match unwrap_server_key(crypto, notes_password, user_id, blob) {
        Ok(key) if key == *master_key => ServerKey::Matches,
        Ok(_) => ServerKey::Differs,
        Err(_) => ServerKey::Unreadable,
    }
}

/// The server's wrapped key as this device found it.
struct FoundKey {
    state: ServerKey,
    /// Identifies the blob, if there is one
    fingerprint: Option<String>,
    /// What a conditional write has to quote, if the server offers those
    updated_at: Option<String>,
}

/// Look at the server's wrapped key without changing anything.
fn inspect_server_key(
    crypto: &CryptoService,
    server: &ServerLink<'_>,
    notes_password: &str,
    master_key: &[u8; 32],
) -> Result<FoundKey> {
    let Some(wrapped) = server.client.get_wrapped_key(server.api_key)? else {
        return Ok(FoundKey { state: ServerKey::Missing, fingerprint: None, updated_at: None });
    };

    let state = if wrapped.kdf_version == WRAPPING_KDF_VERSION {
        compare_with_master_key(crypto, notes_password, server.user_id, &wrapped.blob, master_key)
    } else {
        ServerKey::Unknown
    };

    Ok(FoundKey {
        state,
        fingerprint: Some(crypto.hash(&wrapped.blob)),
        updated_at: wrapped.updated_at,
    })
}

/// How giving the server a wrapped key went.
#[derive(Debug, PartialEq, Eq)]
enum Upload {
    /// It's there, and this is its fingerprint
    Stored(String),
    /// Another device wrote first and nothing changed. Read the key again and decide
    /// again: never send the same thing back.
    Lost,
}

/// Wrap `master_key` with the notes password and give it to the server.
fn upload_wrapped_key(
    crypto: &CryptoService,
    server: &ServerLink<'_>,
    notes_password: &str,
    master_key: &[u8; 32],
    purpose: KeyWrite<'_>,
) -> Result<Upload> {
    let wrapping_key = crypto.derive_wrapping_key(notes_password, server.user_id)?;
    let wrapped = crypto.wrap_master_key(&wrapping_key, master_key)?;
    let blob = serde_json::to_string(&wrapped)?;

    let request = PutWrappedKeyRequest {
        blob: blob.clone(),
        kdf_version: WRAPPING_KDF_VERSION,
        kdf_iterations: WRAPPING_ITERATIONS,
    };

    Ok(match server.client.put_wrapped_key(server.api_key, &request, purpose)? {
        KeyWriteResult::Stored => Upload::Stored(crypto.hash(&blob)),
        KeyWriteResult::Lost => Upload::Lost,
    })
}

/// Record what this device knows about the server's wrapped key. `seen` being set is
/// the proof that stops this device overwriting the blob later.
fn remember_server_key(conn: &Connection, seen: Option<String>, stale: bool) -> Result<()> {
    let repo = SyncRepository::new(conn);
    let mut metadata = repo.get_metadata()?.unwrap_or_default();
    metadata.server_key_seen = seen;
    metadata.server_key_stale = stale;
    repo.update_metadata(&metadata)
}

/// Settle which master key this device uses now it's connected to `account`,
/// returning that key.
///
/// If the account has a key, the notes password must open it. Local notes move to
/// that key in the same transaction that stores it, so they stay readable, and a
/// failure leaves the vault untouched. If the account has no key yet, this device's
/// key becomes the account's.
pub fn adopt_account_key(
    db: &Database,
    crypto: &CryptoService,
    server: &ServerLink<'_>,
    notes_password: &str,
    local_key: &[u8; 32],
) -> Result<[u8; 32]> {
    for _ in 0..KEY_WRITE_ATTEMPTS {
        let Some(wrapped) = server.client.get_wrapped_key(server.api_key)? else {
            // Older servers can't store a key. Sync still works there, so this isn't fatal.
            let seen = match upload_wrapped_key(crypto, server, notes_password, local_key, KeyWrite::First) {
                Ok(Upload::Stored(fingerprint)) => Some(fingerprint),
                // Another device gave the account its key first, so that's the one to use
                Ok(Upload::Lost) => continue,
                Err(e) => {
                    tracing::warn!("Could not upload the wrapped key: {e}");
                    None
                }
            };
            vault::store_local_envelope(db.connection(), crypto, notes_password, local_key)?;
            remember_server_key(db.connection(), seen, false)?;
            return Ok(*local_key);
        };

        let account_key = unwrap_server_key(crypto, notes_password, server.user_id, &wrapped.blob)
            .map_err(|_| anyhow::anyhow!("{}", t!("envelope.unwrap_failed")))?;

        let transaction = db.connection().unchecked_transaction()?;
        if account_key != *local_key {
            rekey_vault(&transaction, crypto, local_key, &account_key)
                .map_err(|e| anyhow::anyhow!("{}", t!("envelope.rekey_failed", error = format!("{e:#}"))))?;
        }
        vault::store_local_envelope(&transaction, crypto, notes_password, &account_key)?;
        remember_server_key(&transaction, Some(crypto.hash(&wrapped.blob)), false)?;
        transaction.commit()?;

        return Ok(account_key);
    }

    anyhow::bail!("{}", t!("envelope.key_keeps_changing"))
}

/// This device's connection to its account, if it has one: endpoint, API key, userId.
fn connection_details(
    conn: &Connection,
    crypto: &CryptoService,
    master_key: &[u8; 32],
) -> Result<Option<(String, String, String)>> {
    let Some(metadata) = SyncRepository::new(conn).get_metadata()? else {
        return Ok(None);
    };

    let (true, Some(user_id), Some(api_key), false) = (
        metadata.sync_enabled,
        metadata.user_id,
        metadata.api_key,
        metadata.sync_endpoint.is_empty(),
    ) else {
        return Ok(None);
    };

    let api_key: EncryptedData = serde_json::from_str(&api_key)
        .context("Failed to parse encrypted API key JSON")?;
    let api_key = crypto.decrypt_text(&api_key, master_key)?;

    Ok(Some((metadata.sync_endpoint, api_key, user_id)))
}

/// Whether this device may replace a blob its password can't open.
///
/// Older clients wrote the blob with the account password, so a device that has never
/// opened or uploaded it can't tell that from a password change, and may heal it, once.
/// A device that has done either knows the blob isn't in the old format, so failing to
/// open it now means another device changed the password, and overwriting it would take
/// that change back. The one exception is a re-wrap this device still owes after its own
/// password change, as long as the blob is the very one it last knew.
fn may_replace_unreadable(metadata: &SyncMetadata, fingerprint: Option<&str>) -> bool {
    match metadata.server_key_seen.as_deref() {
        None => true,
        Some(seen) => metadata.server_key_stale && Some(seen) == fingerprint,
    }
}

/// Repair the server's wrapped key if the notes password can't open it, and upload any
/// re-wrap a failed password change left owing.
///
/// A device that has synced holds the account's real key, so it can replace a blob
/// that's missing or in the old format safely. A blob that does open is never replaced,
/// even when it holds another key: that means this device has diverged, and overwriting
/// would damage the account.
pub fn repair_server_key(app: &mut App, notes_password: &str) -> Result<()> {
    let master_key = app.key.ok_or_else(|| anyhow::anyhow!("Vault is locked"))?;
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    let debug_log = app.debug_log.clone();
    let log = move |message: &str| crate::ui::app::write_debug_log(&debug_log, message);

    let notice = check_server_key(db.connection(), &app.crypto, &master_key, notes_password, &SyncLink::default(), &log)?;
    if let Some(notice) = notice {
        app.error = Some(notice);
    }
    Ok(())
}

/// The work of `repair_server_key`, which needs nothing but the database, so a background
/// sync can do it. Returns anything the user has to be told.
pub(crate) fn check_server_key(
    conn: &Connection,
    crypto: &CryptoService,
    master_key: &[u8; 32],
    notes_password: &str,
    link: &SyncLink,
    log: &dyn Fn(&str),
) -> Result<Option<String>> {
    let metadata = SyncRepository::new(conn).get_metadata()?.unwrap_or_default();
    if metadata.last_sync_at.is_none() {
        return Ok(None);
    }

    let Some((endpoint, api_key, user_id)) = connection_details(conn, crypto, master_key)? else {
        return Ok(None);
    };

    let client = AuthClient::new(endpoint);
    let server = ServerLink { client: &client, api_key: &api_key, user_id: &user_id };

    // Losing a race means another device has just written the key. What it wrote is
    // judged afresh, except that a blob that still won't open is then known not to be
    // in the old format, so it's left alone.
    let mut lost_a_race = false;

    for _ in 0..KEY_WRITE_ATTEMPTS {
        let found = inspect_server_key(crypto, &server, notes_password, master_key)?;

        let purpose = match found.state {
            ServerKey::Matches => {
                link.write(|| remember_server_key(conn, found.fingerprint, false))?;
                return Ok(None);
            }
            ServerKey::Differs => {
                link.write(|| remember_server_key(conn, found.fingerprint, false))?;
                return Ok(Some(t!("envelope.diverged").to_string()));
            }
            ServerKey::Unknown => return Ok(None),
            ServerKey::Missing => KeyWrite::First,
            ServerKey::Unreadable if !lost_a_race && may_replace_unreadable(&metadata, found.fingerprint.as_deref()) => {
                KeyWrite::Repair { updated_at: found.updated_at.as_deref() }
            }
            ServerKey::Unreadable => {
                let seen = if lost_a_race { found.fingerprint } else { metadata.server_key_seen };
                link.write(|| remember_server_key(conn, seen, false))?;
                return Ok(Some(t!("envelope.password_changed_elsewhere").to_string()));
            }
        };

        match upload_wrapped_key(crypto, &server, notes_password, master_key, purpose)? {
            Upload::Stored(fingerprint) => {
                link.write(|| remember_server_key(conn, Some(fingerprint), false))?;
                log("[Envelope] Gave the server a wrapped key the notes password can open");
                return Ok(None);
            }
            Upload::Lost => lost_a_race = true,
        }
    }

    Ok(None)
}

/// Why a device left the server's wrapped key alone after a password change.
#[derive(Debug, PartialEq, Eq)]
enum Rewrap {
    /// The server's copy now opens with the new password, or there's no account
    Done,
    /// The server's copy must not be replaced, for the reason given
    Refused(String),
}

/// Give the server the key wrapped with the new notes password, if that's safe.
fn rewrap_on_server(
    app: &App,
    current_password: &str,
    new_password: &str,
    master_key: &[u8; 32],
) -> Result<Rewrap> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
    let Some((endpoint, api_key, user_id)) = connection_details(db.connection(), &app.crypto, master_key)? else {
        return Ok(Rewrap::Done);
    };
    let metadata = SyncRepository::new(db.connection()).get_metadata()?.unwrap_or_default();

    let client = AuthClient::new(endpoint);
    let server = ServerLink { client: &client, api_key: &api_key, user_id: &user_id };

    let mut lost_a_race = false;

    for _ in 0..KEY_WRITE_ATTEMPTS {
        let found = inspect_server_key(&app.crypto, &server, current_password, master_key)?;

        let purpose = match found.state {
            ServerKey::Differs | ServerKey::Unknown => {
                return Ok(Rewrap::Refused(t!("envelope.diverged").to_string()));
            }
            ServerKey::Matches => KeyWrite::PasswordChange,
            ServerKey::Missing => KeyWrite::First,
            ServerKey::Unreadable => {
                // Another device may already have set this very password
                let with_new = inspect_server_key(&app.crypto, &server, new_password, master_key)?;
                if with_new.state == ServerKey::Matches {
                    remember_server_key(db.connection(), with_new.fingerprint, false)?;
                    return Ok(Rewrap::Done);
                }
                if lost_a_race || !may_replace_unreadable(&metadata, found.fingerprint.as_deref()) {
                    return Ok(Rewrap::Refused(t!("envelope.password_changed_elsewhere").to_string()));
                }
                KeyWrite::Repair { updated_at: found.updated_at.as_deref() }
            }
        };

        match upload_wrapped_key(&app.crypto, &server, new_password, master_key, purpose)? {
            Upload::Stored(fingerprint) => {
                remember_server_key(db.connection(), Some(fingerprint), false)?;
                return Ok(Rewrap::Done);
            }
            Upload::Lost => lost_a_race = true,
        }
    }

    Ok(Rewrap::Refused(t!("envelope.key_keeps_changing").to_string()))
}

/// Change password with envelope encryption support.
///
/// Only the wrapping changes, so no note needs re-encrypting. The local copy has to
/// succeed. If the server's copy can't be updated, the device remembers that it owes
/// the server a new wrapping, retries at the next unlock and sync, and says so, because
/// until then a new device can't join with the new password.
pub fn change_password(
    app: &mut App,
    current_password: &str,
    new_password: &str,
    master_key: &[u8; 32],
) -> Result<()> {
    // A running sync holds the old password, and it may be about to use it on the server
    super::sync::cancel_background_sync(app);

    {
        let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
        vault::store_local_envelope(db.connection(), &app.crypto, new_password, master_key)?;
    }
    app.debug_log("[ChangePassword] Local envelope updated with new password");

    match rewrap_on_server(app, current_password, new_password, master_key) {
        Ok(Rewrap::Done) => {}
        Ok(Rewrap::Refused(reason)) => app.error = Some(reason),
        Err(e) => {
            app.debug_log(&format!("[ChangePassword] Server wrapped key not updated: {}", e));
            let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
            let seen = SyncRepository::new(db.connection()).get_metadata()?
                .and_then(|metadata| metadata.server_key_seen);
            remember_server_key(db.connection(), seen, true)?;
            app.error = Some(t!("envelope.server_update_pending").to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{sync::SyncMetadata, Note};
    use crate::repository::{EncryptionRepository, NoteRepository};
    use crate::test_support::read_request;
    use serde_json::{json, Value};
    use std::io::Write;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    const USER_ID: &str = "b3c1f5a2-8f0e-4c55-9d52-2f6a1c0e7a11";

    /// A fresh random password, so no credential-like literals live in the tests
    fn random_password() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn blob_for(crypto: &CryptoService, password: &str, key: &[u8; 32]) -> String {
        let wrapping_key = crypto.derive_wrapping_key(password, USER_ID).unwrap();
        serde_json::to_string(&crypto.wrap_master_key(&wrapping_key, key).unwrap()).unwrap()
    }

    /// What a client said it wanted from a PUT.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct PutSeen {
        if_match: Option<String>,
        replace: Option<bool>,
    }

    /// Serves one wrapped key as the real server does, conditional writes included, and
    /// records what's put there.
    struct StubServer {
        endpoint: String,
        stored: Arc<Mutex<Option<String>>>,
        puts: Arc<Mutex<usize>>,
        seen: Arc<Mutex<Vec<PutSeen>>>,
        refusing_puts: Arc<Mutex<bool>>,
        /// Written by "another device" just before the next PUT is looked at
        written_first: Arc<Mutex<Option<String>>>,
        /// Answer the next GET as though there were no key
        hide_next_get: Arc<Mutex<bool>>,
        /// An older server: it offers no updatedAt and ignores conditions
        older: Arc<Mutex<bool>>,
    }

    impl StubServer {
        fn start(blob: Option<String>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let stored = Arc::new(Mutex::new(blob));
            let puts = Arc::new(Mutex::new(0));
            let seen = Arc::new(Mutex::new(Vec::new()));
            let refusing_puts = Arc::new(Mutex::new(false));
            let written_first: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let hide_next_get = Arc::new(Mutex::new(false));
            let older = Arc::new(Mutex::new(false));
            let (server_stored, server_puts, server_seen) = (stored.clone(), puts.clone(), seen.clone());
            let (server_refusing, server_written, server_hiding, server_older) =
                (refusing_puts.clone(), written_first.clone(), hide_next_get.clone(), older.clone());

            std::thread::spawn(move || {
                let mut revision = 0;
                for stream in listener.incoming() {
                    let mut stream = stream.unwrap();
                    let Some(request) = read_request(&mut stream) else {
                        continue;
                    };
                    let if_match = request.header("if-match").map(String::from);
                    let (method, body) = (request.method, request.body);
                    let is_older = *server_older.lock().unwrap();

                    let (status, reply) = if method == "PUT" && *server_refusing.lock().unwrap() {
                        ("500 Internal Server Error", "{}".to_string())
                    } else if method == "PUT" {
                        let request: Value = serde_json::from_slice(&body).unwrap();
                        let replace = request["replace"].as_bool();
                        server_seen.lock().unwrap().push(PutSeen { if_match: if_match.clone(), replace });

                        if let Some(blob) = server_written.lock().unwrap().take() {
                            *server_stored.lock().unwrap() = Some(blob);
                            revision += 1;
                        }

                        let mut stored = server_stored.lock().unwrap();
                        let allowed = is_older || match (&if_match, replace) {
                            (Some(tag), _) => stored.is_some() && *tag == format!("revision-{revision}"),
                            (None, Some(false)) => stored.is_none(),
                            (None, _) => true,
                        };

                        if allowed {
                            *stored = request["blob"].as_str().map(String::from);
                            *server_puts.lock().unwrap() += 1;
                            revision += 1;
                            ("200 OK", "{}".to_string())
                        } else {
                            ("409 Conflict", "{}".to_string())
                        }
                    } else {
                        let hidden = std::mem::take(&mut *server_hiding.lock().unwrap());
                        match server_stored.lock().unwrap().as_ref().filter(|_| !hidden) {
                            Some(blob) if is_older => ("200 OK", json!({
                                "blob": blob,
                                "kdfVersion": WRAPPING_KDF_VERSION,
                                "kdfIterations": WRAPPING_ITERATIONS,
                            }).to_string()),
                            Some(blob) => ("200 OK", json!({
                                "blob": blob,
                                "kdfVersion": WRAPPING_KDF_VERSION,
                                "kdfIterations": WRAPPING_ITERATIONS,
                                "updatedAt": format!("revision-{revision}"),
                            }).to_string()),
                            None => ("404 Not Found", "{}".to_string()),
                        }
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                        reply.len(),
                    );
                    stream.write_all(response.as_bytes()).unwrap();
                }
            });

            Self { endpoint, stored, puts, seen, refusing_puts, written_first, hide_next_get, older }
        }

        /// Fail every upload from now on, or stop failing them.
        fn refuse_puts(&self, refuse: bool) {
            *self.refusing_puts.lock().unwrap() = refuse;
        }

        /// Have another device write this blob after this client's next read.
        fn another_device_writes_first(&self, blob: String) {
            *self.written_first.lock().unwrap() = Some(blob);
        }

        fn last_put(&self) -> PutSeen {
            self.seen.lock().unwrap().last().unwrap().clone()
        }

        fn blob(&self) -> Option<String> {
            self.stored.lock().unwrap().clone()
        }

        fn put_count(&self) -> usize {
            *self.puts.lock().unwrap()
        }
    }

    /// A vault holding one note, under a key of its own.
    fn local_vault(notes_password: &str) -> (Database, [u8; 32], Note) {
        let crypto = CryptoService::new();
        let db = Database::in_memory(&random_password()).unwrap();
        let key = vault::unlock(db.connection(), &crypto, notes_password).unwrap();
        let note = Note::new("Written before connecting".to_string());
        NoteRepository::new(db.connection()).create(&note, &key).unwrap();
        (db, key, note)
    }

    fn adopt(db: &Database, endpoint: &str, notes_password: &str, key: &[u8; 32]) -> Result<[u8; 32]> {
        let client = AuthClient::new(endpoint.to_string());
        let server = ServerLink { client: &client, api_key: "device-api-key", user_id: USER_ID };
        adopt_account_key(db, &CryptoService::new(), &server, notes_password, key)
    }

    #[test]
    fn a_blob_wrapped_with_the_notes_password_is_adopted() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let account_key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &notes_password, &account_key)));
        let (db, local_key, _) = local_vault(&notes_password);

        let adopted = adopt(&db, &server.endpoint, &notes_password, &local_key).unwrap();

        assert_eq!(adopted, account_key);
    }

    /// The attack: the server saw the account password at login, so it can wrap a
    /// key of its own with it. A device must not accept that key.
    #[test]
    fn a_blob_wrapped_with_the_account_password_is_refused() {
        let crypto = CryptoService::new();
        let (notes_password, account_password) = (random_password(), random_password());
        let planted_key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &account_password, &planted_key)));
        let (db, local_key, note) = local_vault(&notes_password);
        let before = EncryptionRepository::new(db.connection()).get().unwrap().unwrap();

        let error = adopt(&db, &server.endpoint, &notes_password, &local_key).unwrap_err();

        assert_eq!(error.to_string(), t!("envelope.unwrap_failed"));

        // Nothing moved: the vault still opens with what it had
        let after = EncryptionRepository::new(db.connection()).get().unwrap().unwrap();
        assert_eq!(after.local_wrapped_master, before.local_wrapped_master);
        let kept = NoteRepository::new(db.connection()).get(&note.id, &local_key).unwrap().unwrap();
        assert_eq!(kept.content, note.content);
    }

    #[test]
    fn joining_an_account_keeps_local_notes_readable() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let account_key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &notes_password, &account_key)));
        let (db, local_key, note) = local_vault(&notes_password);

        adopt(&db, &server.endpoint, &notes_password, &local_key).unwrap();

        let moved = NoteRepository::new(db.connection()).get(&note.id, &account_key).unwrap().unwrap();
        assert_eq!(moved.content, note.content);

        // And the next unlock leads to the account's key
        assert_eq!(vault::unlock(db.connection(), &crypto, &notes_password).unwrap(), account_key);
    }

    #[test]
    fn a_note_that_will_not_move_leaves_the_vault_as_it_was() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let account_key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &notes_password, &account_key)));
        let (db, local_key, note) = local_vault(&notes_password);
        let stranger = crypto.generate_master_key();
        NoteRepository::new(db.connection()).create(&Note::new("Damaged".to_string()), &stranger).unwrap();

        assert!(adopt(&db, &server.endpoint, &notes_password, &local_key).is_err());

        let kept = NoteRepository::new(db.connection()).get(&note.id, &local_key).unwrap().unwrap();
        assert_eq!(kept.content, note.content);
        assert_eq!(vault::unlock(db.connection(), &crypto, &notes_password).unwrap(), local_key);
    }

    #[test]
    fn the_first_device_gives_the_account_its_key() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let server = StubServer::start(None);
        let (db, local_key, _) = local_vault(&notes_password);

        let adopted = adopt(&db, &server.endpoint, &notes_password, &local_key).unwrap();

        assert_eq!(adopted, local_key);
        let blob = server.blob().unwrap();
        assert_eq!(unwrap_server_key(&crypto, &notes_password, USER_ID, &blob).unwrap(), local_key);
    }

    /// An unlocked, connected app. `synced` says whether a sync has ever finished.
    fn connected_app(endpoint: &str, key: [u8; 32], synced: bool) -> App {
        let dir = tempfile::tempdir().unwrap();
        let path: PathBuf = dir.path().join("test.db");
        std::mem::forget(dir);
        let mut app = App::new(path, None).unwrap();
        let db = Database::in_memory(&random_password()).unwrap();

        let api_key = app.crypto.encrypt_text("device-api-key", &key).unwrap();
        let metadata = SyncMetadata {
            api_key: Some(serde_json::to_string(&api_key).unwrap()),
            user_id: Some(USER_ID.to_string()),
            sync_enabled: true,
            sync_endpoint: endpoint.to_string(),
            last_sync_at: synced.then(chrono::Utc::now),
            ..Default::default()
        };
        SyncRepository::new(db.connection()).update_metadata(&metadata).unwrap();

        app.db = Some(db);
        app.key = Some(key);
        app
    }

    #[test]
    fn a_synced_device_repairs_a_blob_its_password_cannot_open() {
        let crypto = CryptoService::new();
        let (notes_password, account_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &account_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);

        repair_server_key(&mut app, &notes_password).unwrap();

        assert_eq!(server.put_count(), 1);
        let blob = server.blob().unwrap();
        assert_eq!(unwrap_server_key(&crypto, &notes_password, USER_ID, &blob).unwrap(), key);
    }

    #[test]
    fn a_device_that_has_never_synced_leaves_the_blob_alone() {
        let crypto = CryptoService::new();
        let (notes_password, account_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &account_password, &key)));
        let mut app = connected_app(&server.endpoint, key, false);

        repair_server_key(&mut app, &notes_password).unwrap();

        assert_eq!(server.put_count(), 0);
    }

    #[test]
    fn a_blob_that_opens_is_left_alone() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &notes_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);

        repair_server_key(&mut app, &notes_password).unwrap();

        assert_eq!(server.put_count(), 0);
        assert!(app.error.is_none());
    }

    #[test]
    fn a_blob_holding_another_key_is_never_overwritten() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let (this_device, the_account) = (crypto.generate_master_key(), crypto.generate_master_key());
        let server = StubServer::start(Some(blob_for(&crypto, &notes_password, &the_account)));
        let mut app = connected_app(&server.endpoint, this_device, true);

        repair_server_key(&mut app, &notes_password).unwrap();

        assert_eq!(server.put_count(), 0);
        assert!(app.error.is_some());
    }

    #[test]
    fn changing_the_password_rewraps_the_servers_copy() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        vault::store_local_envelope(app.db.as_ref().unwrap().connection(), &crypto, &old_password, &key).unwrap();

        change_password(&mut app, &old_password, &new_password, &key).unwrap();

        let blob = server.blob().unwrap();
        assert_eq!(unwrap_server_key(&crypto, &new_password, USER_ID, &blob).unwrap(), key);
        let conn = app.db.as_ref().unwrap().connection();
        assert_eq!(vault::unlock(conn, &crypto, &new_password).unwrap(), key);
    }

    #[test]
    fn changing_the_password_never_overwrites_a_different_account_key() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let (this_device, the_account) = (crypto.generate_master_key(), crypto.generate_master_key());
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &the_account)));
        let mut app = connected_app(&server.endpoint, this_device, true);
        vault::store_local_envelope(app.db.as_ref().unwrap().connection(), &crypto, &old_password, &this_device).unwrap();

        change_password(&mut app, &old_password, &new_password, &this_device).unwrap();

        assert_eq!(server.put_count(), 0);
    }

    fn seen_and_stale(app: &App) -> (Option<String>, bool) {
        let db = app.db.as_ref().unwrap();
        let metadata = SyncRepository::new(db.connection()).get_metadata().unwrap().unwrap();
        (metadata.server_key_seen, metadata.server_key_stale)
    }

    #[test]
    fn opening_the_blob_proves_it_is_not_in_the_old_format() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &notes_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        assert!(seen_and_stale(&app).0.is_none());

        repair_server_key(&mut app, &notes_password).unwrap();

        assert!(seen_and_stale(&app).0.is_some());
    }

    #[test]
    fn joining_an_account_counts_as_having_opened_its_blob() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let account_key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &notes_password, &account_key)));
        let (db, local_key, _) = local_vault(&notes_password);

        adopt(&db, &server.endpoint, &notes_password, &local_key).unwrap();

        let metadata = SyncRepository::new(db.connection()).get_metadata().unwrap().unwrap();
        assert!(metadata.server_key_seen.is_some());
    }

    #[test]
    fn giving_an_account_its_key_counts_as_having_seen_the_blob() {
        let notes_password = random_password();
        let server = StubServer::start(None);
        let (db, local_key, _) = local_vault(&notes_password);

        adopt(&db, &server.endpoint, &notes_password, &local_key).unwrap();

        let metadata = SyncRepository::new(db.connection()).get_metadata().unwrap().unwrap();
        assert!(metadata.server_key_seen.is_some());
    }

    /// Device A changes the password, which re-wraps the blob. Device B, still on the
    /// old password, can't open it, but it has opened the blob before, so it knows that
    /// isn't the old format. Overwriting would undo A's change, and A would then do the
    /// same to B: this is the ping-pong the flag exists to stop.
    #[test]
    fn a_password_change_on_one_device_is_not_undone_by_another() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &key)));

        let mut device_a = connected_app(&server.endpoint, key, true);
        let mut device_b = connected_app(&server.endpoint, key, true);
        vault::store_local_envelope(device_a.db.as_ref().unwrap().connection(), &crypto, &old_password, &key).unwrap();
        repair_server_key(&mut device_a, &old_password).unwrap();
        repair_server_key(&mut device_b, &old_password).unwrap();

        change_password(&mut device_a, &old_password, &new_password, &key).unwrap();
        let after_the_change = server.blob().unwrap();
        let uploads = server.put_count();

        repair_server_key(&mut device_b, &old_password).unwrap();

        assert_eq!(server.put_count(), uploads);
        assert_eq!(server.blob().unwrap(), after_the_change);
        assert_eq!(device_b.error, Some(t!("envelope.password_changed_elsewhere").to_string()));
        assert_eq!(unwrap_server_key(&crypto, &new_password, USER_ID, &after_the_change).unwrap(), key);

        // Device A is unaffected, and neither device starts a fight
        repair_server_key(&mut device_a, &new_password).unwrap();
        assert_eq!(server.put_count(), uploads);
        assert!(device_a.error.is_none());
    }

    #[test]
    fn a_device_only_heals_once() {
        let crypto = CryptoService::new();
        let (notes_password, account_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &account_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);

        repair_server_key(&mut app, &notes_password).unwrap();
        assert_eq!(server.put_count(), 1);

        // Another device has since changed the password
        *server.stored.lock().unwrap() = Some(blob_for(&crypto, &random_password(), &key));
        repair_server_key(&mut app, &notes_password).unwrap();

        assert_eq!(server.put_count(), 1);
        assert_eq!(app.error, Some(t!("envelope.password_changed_elsewhere").to_string()));
    }

    #[test]
    fn changing_the_password_leaves_a_blob_changed_elsewhere_alone() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        repair_server_key(&mut app, &old_password).unwrap();
        vault::store_local_envelope(app.db.as_ref().unwrap().connection(), &crypto, &old_password, &key).unwrap();

        *server.stored.lock().unwrap() = Some(blob_for(&crypto, &random_password(), &key));
        change_password(&mut app, &old_password, &new_password, &key).unwrap();

        assert_eq!(server.put_count(), 0);
        assert_eq!(app.error, Some(t!("envelope.password_changed_elsewhere").to_string()));
    }

    /// The way out of the warning above: set this device to the password the other
    /// device chose. The blob already opens with it, so there's nothing to upload.
    #[test]
    fn catching_up_with_a_password_changed_elsewhere_uploads_nothing() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        repair_server_key(&mut app, &old_password).unwrap();
        vault::store_local_envelope(app.db.as_ref().unwrap().connection(), &crypto, &old_password, &key).unwrap();

        *server.stored.lock().unwrap() = Some(blob_for(&crypto, &new_password, &key));
        change_password(&mut app, &old_password, &new_password, &key).unwrap();

        assert_eq!(server.put_count(), 0);
        assert!(app.error.is_none());

        repair_server_key(&mut app, &new_password).unwrap();
        assert!(app.error.is_none());
    }

    #[test]
    fn a_missing_blob_is_put_back_by_a_device_that_has_synced() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let key = crypto.generate_master_key();
        let server = StubServer::start(None);
        let mut app = connected_app(&server.endpoint, key, true);

        repair_server_key(&mut app, &notes_password).unwrap();

        let blob = server.blob().unwrap();
        assert_eq!(unwrap_server_key(&crypto, &notes_password, USER_ID, &blob).unwrap(), key);
    }

    /// A password change while the server is down: the local change stands, the user
    /// hears about it, and the device owes the server an upload until it gets through.
    #[test]
    fn a_failed_rewrap_is_remembered_and_retried() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        repair_server_key(&mut app, &old_password).unwrap();
        vault::store_local_envelope(app.db.as_ref().unwrap().connection(), &crypto, &old_password, &key).unwrap();

        server.refuse_puts(true);
        change_password(&mut app, &old_password, &new_password, &key).unwrap();

        assert_eq!(app.error, Some(t!("envelope.server_update_pending").to_string()));
        assert!(seen_and_stale(&app).1);
        assert_eq!(vault::unlock(app.db.as_ref().unwrap().connection(), &crypto, &new_password).unwrap(), key);

        server.refuse_puts(false);
        app.error = None;
        repair_server_key(&mut app, &new_password).unwrap();

        assert!(!seen_and_stale(&app).1);
        assert!(app.error.is_none());
        let blob = server.blob().unwrap();
        assert_eq!(unwrap_server_key(&crypto, &new_password, USER_ID, &blob).unwrap(), key);
    }

    /// An owed upload is only owed to the blob this device last knew. If another device
    /// has re-wrapped it since, this one's change has lost.
    #[test]
    fn an_owed_upload_does_not_overwrite_a_newer_change() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        repair_server_key(&mut app, &old_password).unwrap();
        vault::store_local_envelope(app.db.as_ref().unwrap().connection(), &crypto, &old_password, &key).unwrap();

        server.refuse_puts(true);
        change_password(&mut app, &old_password, &new_password, &key).unwrap();
        server.refuse_puts(false);

        let elsewhere = blob_for(&crypto, &random_password(), &key);
        *server.stored.lock().unwrap() = Some(elsewhere.clone());
        repair_server_key(&mut app, &new_password).unwrap();

        assert_eq!(server.blob().unwrap(), elsewhere);
        assert_eq!(app.error, Some(t!("envelope.password_changed_elsewhere").to_string()));
        assert!(!seen_and_stale(&app).1);
    }

    #[test]
    fn a_first_upload_says_it_is_one() {
        let notes_password = random_password();
        let server = StubServer::start(None);
        let (db, local_key, _) = local_vault(&notes_password);

        adopt(&db, &server.endpoint, &notes_password, &local_key).unwrap();

        assert_eq!(server.last_put(), PutSeen { if_match: None, replace: Some(false) });
    }

    /// Two devices join an empty account at once. The loser must take the winner's key,
    /// or the account ends up with notes under two keys.
    #[test]
    fn a_device_that_loses_the_first_upload_adopts_the_winners_key() {
        let crypto = CryptoService::new();
        let notes_password = random_password();
        let winners_key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &notes_password, &winners_key)));
        *server.hide_next_get.lock().unwrap() = true;
        let (db, local_key, note) = local_vault(&notes_password);

        let adopted = adopt(&db, &server.endpoint, &notes_password, &local_key).unwrap();

        assert_eq!(adopted, winners_key);
        assert_eq!(server.put_count(), 0);
        let moved = NoteRepository::new(db.connection()).get(&note.id, &winners_key).unwrap().unwrap();
        assert_eq!(moved.content, note.content);
    }

    #[test]
    fn a_repair_names_the_blob_it_read() {
        let crypto = CryptoService::new();
        let (notes_password, account_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &account_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);

        repair_server_key(&mut app, &notes_password).unwrap();

        assert_eq!(server.last_put(), PutSeen { if_match: Some("revision-0".to_string()), replace: None });
        assert_eq!(server.put_count(), 1);
    }

    /// The race the flag alone can't close: a device that has never seen the blob reads
    /// it, can't open it, and another device changes the password before the write lands.
    /// Without a condition, that write would undo the change.
    #[test]
    fn a_repair_that_loses_a_race_leaves_the_winner_alone() {
        let crypto = CryptoService::new();
        let (notes_password, account_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &account_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);

        let the_new_blob = blob_for(&crypto, &random_password(), &key);
        server.another_device_writes_first(the_new_blob.clone());

        repair_server_key(&mut app, &notes_password).unwrap();

        assert_eq!(server.blob().unwrap(), the_new_blob);
        assert_eq!(server.put_count(), 0);
        assert_eq!(app.error, Some(t!("envelope.password_changed_elsewhere").to_string()));

        // Losing proved the blob isn't in the old format, so this device can't heal later
        let (seen, _) = seen_and_stale(&app);
        assert_eq!(seen, Some(crypto.hash(&the_new_blob)));
    }

    /// If the race was lost to the very password this device has, it's simply settled.
    #[test]
    fn a_repair_that_loses_to_its_own_password_is_finished() {
        let crypto = CryptoService::new();
        let (notes_password, account_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &account_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        server.another_device_writes_first(blob_for(&crypto, &notes_password, &key));

        repair_server_key(&mut app, &notes_password).unwrap();

        assert!(app.error.is_none());
        assert_eq!(server.put_count(), 0);
        assert!(seen_and_stale(&app).0.is_some());
    }

    #[test]
    fn a_password_change_says_it_means_to_replace() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        vault::store_local_envelope(app.db.as_ref().unwrap().connection(), &crypto, &old_password, &key).unwrap();

        change_password(&mut app, &old_password, &new_password, &key).unwrap();

        assert_eq!(server.last_put(), PutSeen { if_match: None, replace: Some(true) });
    }

    /// `If-Match` can fail on an older server, so it only goes to one that offers an
    /// updatedAt, and the repair still works there.
    #[test]
    fn an_older_server_gets_no_if_match_and_a_repair_still_lands() {
        let crypto = CryptoService::new();
        let (notes_password, account_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &account_password, &key)));
        *server.older.lock().unwrap() = true;
        let mut app = connected_app(&server.endpoint, key, true);

        repair_server_key(&mut app, &notes_password).unwrap();

        assert_eq!(server.last_put(), PutSeen { if_match: None, replace: Some(false) });
        let blob = server.blob().unwrap();
        assert_eq!(unwrap_server_key(&crypto, &notes_password, USER_ID, &blob).unwrap(), key);
    }

    #[test]
    fn an_owed_rewrap_names_the_blob_it_replaces() {
        let crypto = CryptoService::new();
        let (old_password, new_password) = (random_password(), random_password());
        let key = crypto.generate_master_key();
        let server = StubServer::start(Some(blob_for(&crypto, &old_password, &key)));
        let mut app = connected_app(&server.endpoint, key, true);
        repair_server_key(&mut app, &old_password).unwrap();
        vault::store_local_envelope(app.db.as_ref().unwrap().connection(), &crypto, &old_password, &key).unwrap();

        server.refuse_puts(true);
        change_password(&mut app, &old_password, &new_password, &key).unwrap();
        server.refuse_puts(false);
        app.error = None;

        repair_server_key(&mut app, &new_password).unwrap();

        assert_eq!(server.last_put().if_match.as_deref(), Some("revision-0"));
        assert!(app.error.is_none());
    }
}
