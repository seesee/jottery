import Foundation
import GRDB

/// Sync metadata stored in the `sync_metadata` table (single row).
struct SyncMetadataRecord: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "sync_metadata"

    var id: Int = 1
    var lastSyncAt: String?
    var lastPushAt: String?
    var lastPullAt: String?
    var apiKey: String?
    var clientId: String?
    var syncEnabled: Bool
    var syncEndpoint: String?
    var autoSyncInterval: Int?
    var userEmail: String?
    var pendingRegistrationEmail: String?
    var pendingDeviceName: String?

    enum CodingKeys: String, CodingKey {
        case id
        case lastSyncAt = "last_sync_at"
        case lastPushAt = "last_push_at"
        case lastPullAt = "last_pull_at"
        case apiKey = "api_key"
        case clientId = "client_id"
        case syncEnabled = "sync_enabled"
        case syncEndpoint = "sync_endpoint"
        case autoSyncInterval = "auto_sync_interval"
        case userEmail = "user_email"
        case pendingRegistrationEmail = "pending_registration_email"
        case pendingDeviceName = "pending_device_name"
    }
}

/// Per-note sync tracking.
struct NoteSyncRecord: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "note_sync_metadata"

    var noteId: String
    var syncedAt: String
    var syncHash: String
    var serverVersion: Int
    var lastSyncStatus: String
    var errorMessage: String?
    var conflictData: String?

    enum CodingKeys: String, CodingKey {
        case noteId = "note_id"
        case syncedAt = "synced_at"
        case syncHash = "sync_hash"
        case serverVersion = "server_version"
        case lastSyncStatus = "last_sync_status"
        case errorMessage = "error_message"
        case conflictData = "conflict_data"
    }
}

/// Repository for sync metadata operations.
struct SyncRepository: Sendable {

    let db: DatabaseManager

    // MARK: - Global Sync Metadata

    func getMetadata() throws -> SyncMetadataRecord? {
        try db.dbPool.read { db in
            try SyncMetadataRecord.fetchOne(db, key: 1)
        }
    }

    func saveMetadata(_ record: SyncMetadataRecord) throws {
        try db.dbPool.write { db in
            try record.save(db)
        }
    }

    func updateLastSync(at timestamp: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE sync_metadata SET last_sync_at = ? WHERE id = 1
            """, arguments: [timestamp])
        }
    }

    func updateLastPush(at timestamp: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE sync_metadata SET last_push_at = ? WHERE id = 1
            """, arguments: [timestamp])
        }
    }

    func updateLastPull(at timestamp: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE sync_metadata SET last_pull_at = ? WHERE id = 1
            """, arguments: [timestamp])
        }
    }

    // MARK: - Per-Note Sync Metadata

    func getNoteSyncMetadata(noteId: String) throws -> NoteSyncRecord? {
        try db.dbPool.read { db in
            try NoteSyncRecord.fetchOne(db, key: noteId)
        }
    }

    func saveNoteSyncMetadata(_ record: NoteSyncRecord) throws {
        try db.dbPool.write { db in
            try record.save(db)
        }
    }

    // MARK: - Deletions

    func recordDeletion(id: String, deletedAt: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                INSERT OR REPLACE INTO note_deletions (id, deleted_at) VALUES (?, ?)
            """, arguments: [id, deletedAt])
        }
    }

    func getPendingDeletions() throws -> [(id: String, deletedAt: String)] {
        try db.dbPool.read { db in
            let rows = try Row.fetchAll(db, sql: "SELECT id, deleted_at FROM note_deletions")
            return rows.map { (id: $0["id"], deletedAt: $0["deleted_at"]) }
        }
    }

    func clearDeletions(ids: [String]) throws {
        guard !ids.isEmpty else { return }
        try db.dbPool.write { db in
            let placeholders = ids.map { _ in "?" }.joined(separator: ",")
            try db.execute(
                sql: "DELETE FROM note_deletions WHERE id IN (\(placeholders))",
                arguments: StatementArguments(ids)
            )
        }
    }
}
