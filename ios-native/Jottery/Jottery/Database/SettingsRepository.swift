import Foundation
import GRDB

/// Reads and writes user settings.
struct SettingsRepository: Sendable {

    let db: DatabaseManager

    /// Fetch current settings.
    func get() throws -> UserSettings {
        try db.dbPool.read { db in
            try UserSettings.fetchOne(db, key: 1)
        } ?? .defaults
    }

    /// Save settings.
    func save(_ settings: UserSettings) throws {
        try db.dbPool.write { db in
            try settings.save(db)
        }
    }

    /// Update sync endpoint and enabled flag.
    func updateSync(enabled: Bool, endpoint: String?) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE settings SET sync_enabled = ?, sync_endpoint = ? WHERE id = 1
            """, arguments: [enabled, endpoint])
        }
    }
}
