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

        enum CodingKeys: String, CodingKey {
            case id
            case name
            case query
            case displayOrder = "display_order"
            case createdAt = "created_at"
            case modifiedAt = "modified_at"
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
    }

    // MARK: - Read

    func listAll(key: SymmetricKey) throws -> [SavedSearch] {
        let records = try db.dbPool.read { db in
            try SavedSearchRecord
                .order(Column("display_order").asc)
                .fetchAll(db)
        }
        return records.compactMap { decrypt($0, key: key) }
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
            modifiedAt: nowStr
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
            modifiedAt: now
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
                UPDATE saved_searches SET name = ?, query = ?, modified_at = ? WHERE id = ?
            """, arguments: [nameJSON, queryJSON, now, id])
        }
    }

    // MARK: - Delete

    func delete(id: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: "DELETE FROM saved_searches WHERE id = ?", arguments: [id])
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
                modifiedAt: Date(iso8601: record.modifiedAt) ?? Date()
            )
        } catch {
            return nil
        }
    }
}
