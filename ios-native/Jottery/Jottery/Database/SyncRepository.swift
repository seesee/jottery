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
    var userId: String?
    /// Where the next pull starts. Opaque, and belongs to one account on one
    /// server, like `lastSyncAt`. Nil until the server is known to support it.
    var pullCursor: String?
    /// Whether this device has ever opened or uploaded the account's wrapped
    /// key, or seen it change under a repair. Once true, the server's copy can't
    /// be one an older build wrote, so failing to open it means the password
    /// changed elsewhere (see `EnvelopeService.reconcileWrappedKey`).
    var serverKeyOpened: Bool
    /// A wrapped key still waiting to reach the server after a password change.
    /// Wrapped with the new notes password, as the local copy is.
    var pendingWrappedKey: String?

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
        case userId = "user_id"
        case pullCursor = "pull_cursor"
        case serverKeyOpened = "server_key_opened"
        case pendingWrappedKey = "pending_wrapped_key"
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

    func updatePullCursor(_ cursor: String?) throws {
        try db.dbPool.write { db in
            try db.execute(sql: "UPDATE sync_metadata SET pull_cursor = ? WHERE id = 1", arguments: [cursor])
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

    // MARK: - Account

    /// Start afresh with `userId`, after registering this device. Everything
    /// here belongs to one account on one server: keeping it would mean pulling
    /// from the wrong place, and treating another account's key as known.
    func beginAccount(userId: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE sync_metadata SET
                    user_id = ?,
                    last_sync_at = NULL,
                    last_push_at = NULL,
                    last_pull_at = NULL,
                    pull_cursor = NULL,
                    server_key_opened = 0,
                    pending_wrapped_key = NULL
                WHERE id = 1
            """, arguments: [userId])
        }
    }

    /// Forget the account after disconnecting this device or deleting it.
    func clearAccount() throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE sync_metadata SET
                    user_id = NULL,
                    last_sync_at = NULL,
                    last_push_at = NULL,
                    last_pull_at = NULL,
                    pull_cursor = NULL,
                    server_key_opened = 0,
                    pending_wrapped_key = NULL
                WHERE id = 1
            """)
            try db.execute(sql: "DELETE FROM note_deletions")
        }
    }

    // MARK: - Wrapped Key

    func markServerKeyOpened() throws {
        try db.dbPool.write { db in
            try db.execute(sql: "UPDATE sync_metadata SET server_key_opened = 1 WHERE id = 1")
        }
    }

    func savePendingWrappedKey(_ blob: String?) throws {
        try db.dbPool.write { db in
            try db.execute(
                sql: "UPDATE sync_metadata SET pending_wrapped_key = ? WHERE id = 1",
                arguments: [blob]
            )
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
            try Self.recordDeletion(id: id, deletedAt: deletedAt, in: db)
        }
    }

    /// Queue a permanent deletion to be pushed, as part of `db`'s transaction.
    static func recordDeletion(id: String, deletedAt: String, in db: Database) throws {
        try db.execute(sql: """
            INSERT OR REPLACE INTO note_deletions (id, deleted_at) VALUES (?, ?)
        """, arguments: [id, deletedAt])
    }

    /// Deletions to send. A permanent deletion removes the note, so one whose note
    /// exists again is stale: the note came back from a backup or an import, and
    /// sending it would delete the note again, on every device.
    func getPendingDeletions() throws -> [(id: String, deletedAt: String)] {
        try db.dbPool.read { db in
            let rows = try Row.fetchAll(db, sql: """
                SELECT id, deleted_at FROM note_deletions
                WHERE id NOT IN (SELECT id FROM notes)
            """)
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
