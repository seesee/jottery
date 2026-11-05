import Foundation
import GRDB

/// CRUD operations on attachment blobs in the `attachments` table.
struct AttachmentRepository: Sendable {

    let db: DatabaseManager

    /// Store an attachment blob with metadata from the note's AttachmentRef.
    /// Uses INSERT OR REPLACE so it works for both new and updated blobs.
    ///
    /// The filename stays in the note's ref, encrypted, and never comes here:
    /// the column is plaintext and nothing reads it. `uploaded` says the server
    /// already holds this blob, as it does one that was pulled.
    func storeBlob(id: String, mimeType: String, size: Int, data: Data, uploaded: Bool = false) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                INSERT OR REPLACE INTO attachments (id, filename, mime_type, size, data, uploaded)
                VALUES (?, '', ?, ?, ?, ?)
            """, arguments: [id, mimeType, size, data, uploaded])
        }
    }

    /// Blank up to `limit` of the plaintext filenames older builds left in this
    /// table, and say how many that was. Changing a row's size rewrites the
    /// attachment data in it, so this is a few rows at a time, never one statement
    /// over the table. Nothing reads the column.
    @discardableResult
    func clearOldFilenames(limit: Int) throws -> Int {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE attachments SET filename = ''
                WHERE id IN (SELECT id FROM attachments WHERE filename != '' LIMIT ?)
            """, arguments: [limit])
            return db.changesCount
        }
    }

    /// The blob, unless the server is known to hold it already.
    func getBlobToUpload(id: String) throws -> Data? {
        try db.dbPool.read { db in
            try Data.fetchOne(db, sql: "SELECT data FROM attachments WHERE id = ? AND uploaded = 0", arguments: [id])
        }
    }

    /// Bytes of each listed blob the server doesn't hold yet.
    func uploadSizes(ids: [String]) throws -> [String: Int] {
        guard !ids.isEmpty else { return [:] }
        return try db.dbPool.read { db in
            let placeholders = ids.map { _ in "?" }.joined(separator: ",")
            let rows = try Row.fetchAll(db, sql: """
                SELECT id, LENGTH(data) AS bytes FROM attachments WHERE uploaded = 0 AND id IN (\(placeholders))
            """, arguments: StatementArguments(ids))
            return Dictionary(uniqueKeysWithValues: rows.map { ($0["id"], $0["bytes"]) })
        }
    }

    func markUploaded(ids: [String]) throws {
        guard !ids.isEmpty else { return }
        try db.dbPool.write { db in
            let placeholders = ids.map { _ in "?" }.joined(separator: ",")
            try db.execute(
                sql: "UPDATE attachments SET uploaded = 1 WHERE id IN (\(placeholders))",
                arguments: StatementArguments(ids)
            )
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

    /// Update the raw blob data for an existing attachment (e.g. during password change re-encryption).
    func updateBlobData(id: String, data: Data) throws {
        try db.dbPool.write { db in
            try db.execute(sql: "UPDATE attachments SET data = ? WHERE id = ?", arguments: [data, id])
        }
    }

    /// Delete a blob by ID.
    func deleteBlob(id: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: "DELETE FROM attachments WHERE id = ?", arguments: [id])
        }
    }
}
