import CryptoKit
import Foundation
import GRDB

/// Moves a whole vault from one master key to another in a single database
/// transaction. Every value the master key protects is decrypted with the old
/// key and re-encrypted with the new one, and the vault's encryption metadata
/// is swapped in the same transaction. If any protected value cannot be
/// decrypted, the transaction rolls back and nothing changes: the vault is
/// entirely on the old key or entirely on the new one, never a mix.
///
/// It replaces two loops that skipped (and only logged) anything they could not
/// re-encrypt and swapped the key separately afterwards, so a partial failure
/// left notes under the old key and unreadable after the switch.
///
/// What the master key protects:
/// - `notes` and `note_versions`: content, tags, and each attachment filename
///   in the attachments JSON
/// - `attachments`: the blob data, which the server then needs again
/// - `saved_searches`: name and query
/// - `settings`: category names and tag colours
/// - `encryption_metadata`: the verification token (replaced with `newMetadata`)
///
/// Values that already open with the new key are left as they are. A vault
/// that joined an account and synced before moving onto the account's key
/// holds rows pulled under that key; treating them as unreadable would block
/// the move for good. AES-GCM authentication makes "opens with this key"
/// unambiguous. Only a value that opens with neither key aborts the re-key.
///
/// Two kinds of value are also left alone because they are under no key at
/// all: plaintext attachment filenames and blobs that are not encrypted JSON
/// (a backup restore stores those as-is). Content, tags and saved searches are
/// always encrypted, so anything there that does not parse aborts the re-key.
/// Settings are the exception: they're only preferences, so one that can't be
/// read is left as it is rather than stopping a password change.
///
/// Runs synchronously on the caller's thread. Called from the main actor, that
/// also keeps any other main-actor write (an editor save with the old key, say)
/// from landing between the re-key and the caller switching keys.
enum VaultReKeyService {

    struct Summary: Equatable, Sendable {
        var notes = 0
        var versions = 0
        var attachments = 0
        var savedSearches = 0
        /// Rows left untouched because every value already opened with the
        /// new key.
        var alreadyOnNewKey = 0
    }

    enum ReKeyError: LocalizedError, Equatable {
        /// A protected value could not be decrypted with the old key. The
        /// transaction was rolled back, so nothing changed.
        case unreadable(table: String, id: String)

        var errorDescription: String? {
            switch self {
            case .unreadable:
                return L.vaultReKeyErrorUnreadable
            }
        }
    }

    /// Re-encrypt everything from `oldKey` to `newKey` and store `newMetadata`,
    /// all in one transaction. Notes and saved searches are flagged for sync,
    /// since their server copies (if any) are under the old key.
    @discardableResult
    static func reKey(
        database: DatabaseManager,
        from oldKey: SymmetricKey,
        to newKey: SymmetricKey,
        newMetadata: EncryptionMetadata
    ) throws -> Summary {
        let keys = ReKeyer(oldKey: oldKey, newKey: newKey)
        return try database.dbPool.write { db in
            var summary = Summary()

            for row in try Row.fetchAll(db, sql: "SELECT id, content, tags, attachments FROM notes") {
                let id: String = row["id"]
                let old: [String?] = [row["content"], row["tags"], row["attachments"]]
                let new: [String?] = [
                    try keys.required(old[0], "notes", id),
                    try keys.required(old[1], "notes", id),
                    try keys.attachmentList(old[2], "notes", id),
                ]
                guard new != old else { summary.alreadyOnNewKey += 1; continue }
                try db.execute(
                    sql: "UPDATE notes SET content = ?, tags = ?, attachments = ?, needs_sync = 1 WHERE id = ?",
                    arguments: [new[0], new[1], new[2], id]
                )
                summary.notes += 1
            }

            for row in try Row.fetchAll(db, sql: "SELECT version_key, content, tags, attachments FROM note_versions") {
                let key: String = row["version_key"]
                let old: [String?] = [row["content"], row["tags"], row["attachments"]]
                let new: [String?] = [
                    try keys.required(old[0], "note_versions", key),
                    try keys.required(old[1], "note_versions", key),
                    try keys.attachmentList(old[2], "note_versions", key),
                ]
                guard new != old else { summary.alreadyOnNewKey += 1; continue }
                try db.execute(
                    sql: "UPDATE note_versions SET content = ?, tags = ?, attachments = ? WHERE version_key = ?",
                    arguments: [new[0], new[1], new[2], key]
                )
                summary.versions += 1
            }

            // One blob at a time: attachments can be large, so loading them
            // all at once could exhaust memory mid-transaction.
            for id in try String.fetchAll(db, sql: "SELECT id FROM attachments") {
                let data = try Data.fetchOne(db, sql: "SELECT data FROM attachments WHERE id = ?", arguments: [id])
                guard let reEncrypted = try keys.blob(data, "attachments", id) else { continue }
                try db.execute(sql: "UPDATE attachments SET data = ?, uploaded = 0 WHERE id = ?", arguments: [reEncrypted, id])
                summary.attachments += 1
            }

            for row in try Row.fetchAll(db, sql: "SELECT id, name, query FROM saved_searches") {
                let id: String = row["id"]
                let old: [String?] = [row["name"], row["query"]]
                let new: [String?] = [
                    try keys.required(old[0], "saved_searches", id),
                    try keys.required(old[1], "saved_searches", id),
                ]
                guard new != old else { summary.alreadyOnNewKey += 1; continue }
                try db.execute(
                    sql: "UPDATE saved_searches SET name = ?, query = ?, needs_sync = 1 WHERE id = ?",
                    arguments: [new[0], new[1], id]
                )
                summary.savedSearches += 1
            }

            if let row = try Row.fetchOne(db, sql: "SELECT color_palette, tag_colors FROM settings WHERE id = 1") {
                try db.execute(
                    sql: "UPDATE settings SET color_palette = ?, tag_colors = ? WHERE id = 1",
                    arguments: [keys.preference(row["color_palette"]), keys.preference(row["tag_colors"])]
                )
            }

            try newMetadata.save(db)
            return summary
        }
    }
}

/// The per-value re-encryption rules. Every failure becomes
/// `ReKeyError.unreadable`, which aborts the surrounding transaction.
private struct ReKeyer {
    let oldKey: SymmetricKey
    let newKey: SymmetricKey

    /// A value that must be encrypted, under the old key or already the new.
    func required(_ json: String?, _ table: String, _ id: String) throws -> String {
        do {
            guard let json else { throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id) }
            return try reEncrypt(CryptoService.parseEncryptedJSON(json)) ?? json
        } catch {
            throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
        }
    }

    /// A preference: re-encrypted if it can be, otherwise as it was.
    func preference(_ json: String?) -> String? {
        guard let json, let encrypted = try? CryptoService.parseEncryptedJSON(json),
              let reEncrypted = try? reEncrypt(encrypted) else {
            return json
        }
        return reEncrypted
    }

    /// An attachments JSON array: each filename is re-encrypted, except
    /// plaintext ones, which are under no key, and ones already on the new
    /// key. Returned unchanged when no filename changed.
    func attachmentList(_ json: String?, _ table: String, _ id: String) throws -> String? {
        guard let json else { return nil }
        let trimmed = json.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != "[]" else { return json }
        guard var refs = try? JSONDecoder().decode([AttachmentRef].self, from: Data(json.utf8)) else {
            throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
        }
        var changed = false
        for index in refs.indices {
            guard let encrypted = try? CryptoService.parseEncryptedJSON(refs[index].filename) else { continue }
            do {
                if let reEncrypted = try reEncrypt(encrypted) {
                    refs[index].filename = reEncrypted
                    changed = true
                }
            } catch {
                throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
            }
        }
        guard changed else { return json }
        guard let encoded = try? JSONEncoder().encode(refs) else {
            throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
        }
        return String(decoding: encoded, as: UTF8.self)
    }

    /// An attachment blob, or nil to leave it as it is: empty, not encrypted
    /// JSON at all, or already on the new key.
    func blob(_ data: Data?, _ table: String, _ id: String) throws -> Data? {
        guard let data, !data.isEmpty,
              let json = String(data: data, encoding: .utf8),
              let encrypted = try? CryptoService.parseEncryptedJSON(json) else {
            return nil
        }
        do {
            return try reEncrypt(encrypted).map { Data($0.utf8) }
        } catch {
            throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
        }
    }

    /// Re-encrypt from the old key to the new one. Nil when the value already
    /// opens with the new key, so it is left alone. Throws when it opens with
    /// neither.
    private func reEncrypt(_ encrypted: EncryptedData) throws -> String? {
        if let plain = try? CryptoService.decrypt(encrypted, key: oldKey) {
            return try CryptoService.serializeEncryptedJSON(CryptoService.encrypt(plain, key: newKey))
        }
        _ = try CryptoService.decrypt(encrypted, key: newKey)
        return nil
    }
}
