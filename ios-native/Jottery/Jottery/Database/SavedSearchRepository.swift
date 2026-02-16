import CryptoKit
import Foundation
import GRDB

/// CRUD for saved searches with encrypted name/query fields.
struct SavedSearchRepository: Sendable {

    let db: DatabaseManager

    // MARK: - Model

    struct SavedSearchRecord: Codable, FetchableRecord, PersistableRecord, Identifiable {
        static let databaseTableName = "saved_searches"

        var id: String
        var name: String            // Encrypted JSON
        var query: String           // Encrypted JSON
        var displayOrder: Int
        var createdAt: String
        var modifiedAt: String
        var syncedAt: String?
        var deleted: Bool
        var deletedAt: String?
        var version: Int
        var needsSync: Bool

        enum CodingKeys: String, CodingKey {
            case id
            case name
            case query
            case displayOrder = "display_order"
            case createdAt = "created_at"
            case modifiedAt = "modified_at"
            case syncedAt = "synced_at"
            case deleted
            case deletedAt = "deleted_at"
            case version
            case needsSync = "needs_sync"
        }
    }

    /// Decrypted saved search for in-memory use.
    struct SavedSearch: Identifiable {
        let id: String
        var name: String
        var query: String
        var displayOrder: Int
        var createdAt: Date
        var modifiedAt: Date
        var syncedAt: Date?
        var deleted: Bool
        var deletedAt: Date?
        var version: Int
    }

    // MARK: - Read

    func listAll(key: SymmetricKey) throws -> [SavedSearch] {
        let records = try db.dbPool.read { db in
            try SavedSearchRecord
                .filter(Column("deleted") == false)
                .order(Column("display_order").asc)
                .fetchAll(db)
        }
        return records.compactMap { decrypt($0, key: key) }
    }

    /// Return records that need syncing (for push).
    func listNeedingSync() throws -> [SavedSearchRecord] {
        try db.dbPool.read { db in
            try SavedSearchRecord
                .filter(Column("needs_sync") == true)
                .fetchAll(db)
        }
    }

    /// Fetch a single raw record by ID.
    func getRaw(id: String) throws -> SavedSearchRecord? {
        try db.dbPool.read { db in
            try SavedSearchRecord.fetchOne(db, key: id)
        }
    }

    // MARK: - Create

    func create(name: String, query: String, key: SymmetricKey) throws -> SavedSearch {
        let id = UUID().uuidString
        let now = Date()
        let nowStr = now.iso8601

        let encName = try CryptoService.encryptText(name, key: key)
        let encQuery = try CryptoService.encryptText(query, key: key)

        // Get next display order
        let maxOrder = try db.dbPool.read { db -> Int in
            try Int.fetchOne(db, sql: "SELECT MAX(display_order) FROM saved_searches") ?? 0
        }

        let record = SavedSearchRecord(
            id: id,
            name: try CryptoService.serializeEncryptedJSON(encName),
            query: try CryptoService.serializeEncryptedJSON(encQuery),
            displayOrder: maxOrder + 1,
            createdAt: nowStr,
            modifiedAt: nowStr,
            syncedAt: nil,
            deleted: false,
            deletedAt: nil,
            version: 1,
            needsSync: true
        )
        try db.dbPool.write { db in
            try record.insert(db)
        }

        return SavedSearch(
            id: id,
            name: name,
            query: query,
            displayOrder: maxOrder + 1,
            createdAt: now,
            modifiedAt: now,
            syncedAt: nil,
            deleted: false,
            deletedAt: nil,
            version: 1
        )
    }

    // MARK: - Update

    func update(id: String, name: String, query: String, key: SymmetricKey) throws {
        let encName = try CryptoService.encryptText(name, key: key)
        let encQuery = try CryptoService.encryptText(query, key: key)
        let nameJSON = try CryptoService.serializeEncryptedJSON(encName)
        let queryJSON = try CryptoService.serializeEncryptedJSON(encQuery)
        let now = Date().iso8601

        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE saved_searches
                SET name = ?, query = ?, modified_at = ?, needs_sync = 1, version = version + 1
                WHERE id = ?
            """, arguments: [nameJSON, queryJSON, now, id])
        }
    }

    // MARK: - Delete (soft)

    func delete(id: String) throws {
        let now = Date().iso8601
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE saved_searches
                SET deleted = 1, deleted_at = ?, modified_at = ?, needs_sync = 1, version = version + 1
                WHERE id = ?
            """, arguments: [now, now, id])
        }
    }

    /// Update the encrypted name and query fields directly (used during password change re-encryption).
    func updateEncrypted(id: String, name: String, query: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE saved_searches SET name = ?, query = ?, needs_sync = 1 WHERE id = ?
            """, arguments: [name, query, id])
        }
    }

    // MARK: - Sync Helpers

    /// Mark a saved search as synced after successful push.
    func markSynced(id: String, syncedAt: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE saved_searches SET needs_sync = 0, synced_at = ? WHERE id = ?
            """, arguments: [syncedAt, id])
        }
    }

    /// Upsert a record from a pull response using last-write-wins on modifiedAt.
    func insertOrReplace(record: SavedSearchRecord) throws {
        try db.dbPool.write { db in
            if let existing = try SavedSearchRecord.fetchOne(db, key: record.id) {
                // LWW: only overwrite if remote is newer
                if record.modifiedAt > existing.modifiedAt {
                    try record.update(db)
                }
            } else {
                try record.insert(db)
            }
        }
    }

    // MARK: - Private

    private func decrypt(_ record: SavedSearchRecord, key: SymmetricKey) -> SavedSearch? {
        do {
            let encName = try CryptoService.parseEncryptedJSON(record.name)
            let name = try CryptoService.decryptText(encName, key: key)
            let encQuery = try CryptoService.parseEncryptedJSON(record.query)
            let query = try CryptoService.decryptText(encQuery, key: key)
            return SavedSearch(
                id: record.id,
                name: name,
                query: query,
                displayOrder: record.displayOrder,
                createdAt: Date(iso8601: record.createdAt) ?? Date(),
                modifiedAt: Date(iso8601: record.modifiedAt) ?? Date(),
                syncedAt: record.syncedAt.flatMap { Date(iso8601: $0) },
                deleted: record.deleted,
                deletedAt: record.deletedAt.flatMap { Date(iso8601: $0) },
                version: record.version
            )
        } catch {
            return nil
        }
    }
}
