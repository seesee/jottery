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
/// - `attachments`: the blob data (the filename column there is plaintext)
/// - `saved_searches`: name and query
/// - `encryption_metadata`: the verification token (replaced with `newMetadata`)
///
/// Two kinds of value are left alone because they are under no key at all:
/// attachment filenames that are plaintext (very old attachments) and blobs
/// that are not encrypted JSON (a backup restore stores those as-is). Content,
/// tags and saved searches are always encrypted, so anything there that does
/// not decrypt aborts the re-key.
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
    }

    enum ReKeyError: LocalizedError, Equatable {
        /// A protected value could not be decrypted with the old key. The
        /// transaction was rolled back, so nothing changed.
        case unreadable(table: String, id: String)

        var errorDescription: String? {
            switch self {
            case let .unreadable(table, id):
                return "Could not re-encrypt \(table) \(id); nothing was changed"
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
                try db.execute(
                    sql: "UPDATE notes SET content = ?, tags = ?, attachments = ?, needs_sync = 1 WHERE id = ?",
                    arguments: [
                        keys.required(row["content"], "notes", id),
                        keys.required(row["tags"], "notes", id),
                        keys.attachmentList(row["attachments"], "notes", id),
                        id,
                    ]
                )
                summary.notes += 1
            }

            for row in try Row.fetchAll(db, sql: "SELECT version_key, content, tags, attachments FROM note_versions") {
                let key: String = row["version_key"]
                try db.execute(
                    sql: "UPDATE note_versions SET content = ?, tags = ?, attachments = ? WHERE version_key = ?",
                    arguments: [
                        keys.required(row["content"], "note_versions", key),
                        keys.required(row["tags"], "note_versions", key),
                        keys.attachmentList(row["attachments"], "note_versions", key),
                        key,
                    ]
                )
                summary.versions += 1
            }

            for row in try Row.fetchAll(db, sql: "SELECT id, data FROM attachments") {
                let id: String = row["id"]
                guard let data = try keys.blob(row["data"], "attachments", id) else { continue }
                try db.execute(sql: "UPDATE attachments SET data = ? WHERE id = ?", arguments: [data, id])
                summary.attachments += 1
            }

            for row in try Row.fetchAll(db, sql: "SELECT id, name, query FROM saved_searches") {
                let id: String = row["id"]
                try db.execute(
                    sql: "UPDATE saved_searches SET name = ?, query = ?, needs_sync = 1 WHERE id = ?",
                    arguments: [
                        keys.required(row["name"], "saved_searches", id),
                        keys.required(row["query"], "saved_searches", id),
                        id,
                    ]
                )
                summary.savedSearches += 1
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

    /// A value that must be encrypted under the old key.
    func required(_ json: String?, _ table: String, _ id: String) throws -> String {
        do {
            guard let json else { throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id) }
            return try reEncrypt(CryptoService.parseEncryptedJSON(json))
        } catch {
            throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
        }
    }

    /// An attachments JSON array: each filename is re-encrypted, except
    /// plaintext ones, which are under no key.
    func attachmentList(_ json: String?, _ table: String, _ id: String) throws -> String? {
        guard let json else { return nil }
        let trimmed = json.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != "[]" else { return json }
        guard var refs = try? JSONDecoder().decode([AttachmentRef].self, from: Data(json.utf8)) else {
            throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
        }
        for index in refs.indices {
            guard let encrypted = try? CryptoService.parseEncryptedJSON(refs[index].filename) else { continue }
            do {
                refs[index].filename = try reEncrypt(encrypted)
            } catch {
                throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
            }
        }
        guard let encoded = try? JSONEncoder().encode(refs) else {
            throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
        }
        return String(decoding: encoded, as: UTF8.self)
    }

    /// An attachment blob, or nil to leave it as it is: empty, or not
    /// encrypted JSON at all.
    func blob(_ data: Data?, _ table: String, _ id: String) throws -> Data? {
        guard let data, !data.isEmpty,
              let json = String(data: data, encoding: .utf8),
              let encrypted = try? CryptoService.parseEncryptedJSON(json) else {
            return nil
        }
        do {
            return Data(try reEncrypt(encrypted).utf8)
        } catch {
            throw VaultReKeyService.ReKeyError.unreadable(table: table, id: id)
        }
    }

    private func reEncrypt(_ encrypted: EncryptedData) throws -> String {
        let plain = try CryptoService.decrypt(encrypted, key: oldKey)
        return try CryptoService.serializeEncryptedJSON(CryptoService.encrypt(plain, key: newKey))
    }
}
