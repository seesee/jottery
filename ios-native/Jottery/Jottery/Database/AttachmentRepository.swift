import Foundation
import GRDB

/// CRUD operations on attachment blobs in the `attachments` table.
struct AttachmentRepository: Sendable {

    let db: DatabaseManager

    /// Store an attachment blob with metadata from the note's AttachmentRef.
    /// Uses INSERT OR REPLACE so it works for both new and updated blobs.
    func storeBlob(id: String, filename: String, mimeType: String, size: Int, data: Data) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                INSERT OR REPLACE INTO attachments (id, filename, mime_type, size, data)
                VALUES (?, ?, ?, ?, ?)
            """, arguments: [id, filename, mimeType, size, data])
        }
    }

    /// Fetch the raw blob data for an attachment.
    func getBlob(id: String) throws -> Data? {
        try db.dbPool.read { db in
            try Data.fetchOne(db, sql: "SELECT data FROM attachments WHERE id = ?", arguments: [id])
        }
    }

    /// Check whether a blob exists for the given ID.
    func hasBlob(id: String) throws -> Bool {
        try db.dbPool.read { db in
            let count = try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM attachments WHERE id = ?", arguments: [id])
            return (count ?? 0) > 0
        }
    }

    /// List all stored attachment blob IDs.
    func listBlobIds() throws -> [String] {
        try db.dbPool.read { db in
            try String.fetchAll(db, sql: "SELECT id FROM attachments")
        }
    }

    /// Delete a blob by ID.
    func deleteBlob(id: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: "DELETE FROM attachments WHERE id = ?", arguments: [id])
        }
    }
}
