import Foundation
import GRDB

/// Manages the GRDB `DatabasePool` and runs migrations.
/// Database location: `~/Library/Application Support/Jottery/jottery.db`
final class DatabaseManager: Sendable {

    let dbPool: DatabasePool

    /// Initialise with the default application support path.
    init() throws {
        let path = try DatabaseManager.databasePath()
        var config = Configuration()
        config.foreignKeysEnabled = true
        dbPool = try DatabasePool(path: path, configuration: config)
        try migrate()
    }

    /// Initialise with a specific path (for testing).
    init(path: String) throws {
        var config = Configuration()
        config.foreignKeysEnabled = true
        dbPool = try DatabasePool(path: path, configuration: config)
        try migrate()
    }

    /// In-memory database (for unit tests).
    init(inMemory: Bool) throws {
        var config = Configuration()
        config.foreignKeysEnabled = true
        if inMemory {
            dbPool = try DatabasePool(path: ":memory:", configuration: config)
        } else {
            let path = try DatabaseManager.databasePath()
            dbPool = try DatabasePool(path: path, configuration: config)
        }
        try migrate()
    }

    // MARK: - Path

    private static func databasePath() throws -> String {
        let fm = FileManager.default
        let appSupport = try fm.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let dir = appSupport.appendingPathComponent("Jottery", isDirectory: true)
        try fm.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("jottery.db").path
    }

    // MARK: - Migrations

    /// Run all migrations matching the TUI's 15 migration files.
    private func migrate() throws {
        var migrator = DatabaseMigrator()

        // 001_initial
        migrator.registerMigration("001_initial") { db in
            try db.create(table: "notes", ifNotExists: true) { t in
                t.column("id", .text).primaryKey()
                t.column("created_at", .text).notNull()
                t.column("modified_at", .text).notNull()
                t.column("synced_at", .text)
                t.column("content", .text).notNull()
                t.column("tags", .text).notNull()
                t.column("attachments", .text).notNull()
                t.column("pinned", .boolean).notNull().defaults(to: false)
                t.column("deleted", .boolean).notNull().defaults(to: false)
                t.column("deleted_at", .text)
                t.column("sync_hash", .text)
                t.column("version", .integer).notNull().defaults(to: 1)
                t.column("word_wrap", .boolean).notNull().defaults(to: true)
                t.column("syntax_language", .text).notNull().defaults(to: "plain")
            }

            try db.create(indexOn: "notes", columns: ["modified_at"])
            try db.create(indexOn: "notes", columns: ["created_at"])
            try db.create(indexOn: "notes", columns: ["deleted"])
            try db.create(indexOn: "notes", columns: ["pinned"])
            try db.create(indexOn: "notes", columns: ["deleted", "modified_at"])

            try db.create(table: "attachments", ifNotExists: true) { t in
                t.column("id", .text).primaryKey()
                t.column("filename", .text).notNull()
                t.column("mime_type", .text).notNull()
                t.column("size", .integer).notNull()
                t.column("data", .blob).notNull()
                t.column("thumbnail_data", .blob)
            }

            try db.create(table: "settings", ifNotExists: true) { t in
                t.column("id", .integer).primaryKey().check { $0 == 1 }
                t.column("language", .text).notNull().defaults(to: "en-GB")
                t.column("theme", .text).notNull().defaults(to: "auto")
                t.column("sort_order", .text).notNull().defaults(to: "recent")
                t.column("auto_lock_timeout", .integer).notNull().defaults(to: 15)
                t.column("sync_enabled", .boolean).notNull().defaults(to: false)
                t.column("sync_endpoint", .text)
            }

            try db.execute(sql: """
                INSERT OR IGNORE INTO settings (id, language, theme, sort_order, auto_lock_timeout, sync_enabled)
                VALUES (1, 'en-GB', 'auto', 'recent', 15, 0)
            """)

            try db.create(table: "encryption_metadata", ifNotExists: true) { t in
                t.column("id", .integer).primaryKey().check { $0 == 1 }
                t.column("salt", .text).notNull()
                t.column("iterations", .integer).notNull()
                t.column("created_at", .text).notNull()
                t.column("algorithm", .text).notNull().defaults(to: "AES-256-GCM")
            }

            try db.create(table: "sync_metadata", ifNotExists: true) { t in
                t.column("id", .integer).primaryKey().check { $0 == 1 }
                t.column("last_sync_at", .text)
                t.column("last_push_at", .text)
                t.column("last_pull_at", .text)
                t.column("api_key", .text)
                t.column("client_id", .text)
                t.column("sync_enabled", .boolean).notNull().defaults(to: false)
                t.column("sync_endpoint", .text)
                t.column("auto_sync_interval", .integer).defaults(to: 5)
            }

            try db.create(table: "note_sync_metadata", ifNotExists: true) { t in
                t.column("note_id", .text).primaryKey()
                    .references("notes", column: "id", onDelete: .cascade)
                t.column("synced_at", .text).notNull()
                t.column("sync_hash", .text).notNull()
                t.column("server_version", .integer).notNull()
                t.column("last_sync_status", .text).notNull().defaults(to: "pending")
                t.column("error_message", .text)
            }
        }

        // 004_update_syntax_default — update existing 'plain' to 'markdown'
        migrator.registerMigration("004_update_syntax_default") { db in
            try db.execute(sql: """
                UPDATE notes SET syntax_language = 'markdown' WHERE syntax_language = 'plain'
            """)
        }

        // 005_add_note_versions
        migrator.registerMigration("005_add_note_versions") { db in
            try db.create(table: "note_versions", ifNotExists: true) { t in
                t.column("version_key", .text).primaryKey()
                t.column("note_id", .text).notNull()
                t.column("version", .integer).notNull()
                t.column("created_at", .text).notNull()
                t.column("synced_at", .text).notNull()
                t.column("content", .text).notNull()
                t.column("tags", .text).notNull()
                t.column("attachments", .text).notNull()
                t.column("syntax_language", .text)
                t.column("word_wrap", .boolean)
                t.column("reason", .text).notNull()
            }

            try db.create(indexOn: "note_versions", columns: ["note_id"])
            try db.create(indexOn: "note_versions", columns: ["note_id", "version"])
        }

        // 006_add_user_email
        migrator.registerMigration("006_add_user_email") { db in
            try db.execute(sql: """
                ALTER TABLE sync_metadata ADD COLUMN user_email TEXT
            """)
        }

        // 007_add_conflict_data
        migrator.registerMigration("007_add_conflict_data") { db in
            try db.execute(sql: """
                ALTER TABLE note_sync_metadata ADD COLUMN conflict_data TEXT
            """)
        }

        // 008_add_pending_registration
        migrator.registerMigration("008_add_pending_registration") { db in
            try db.execute(sql: """
                ALTER TABLE sync_metadata ADD COLUMN pending_registration_email TEXT
            """)
        }

        // 009_add_note_colors
        migrator.registerMigration("009_add_note_colors") { db in
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN color TEXT")
            try db.execute(sql: "ALTER TABLE note_versions ADD COLUMN color TEXT")
            try db.execute(sql: "ALTER TABLE settings ADD COLUMN color_palette TEXT")
            try db.execute(sql: "ALTER TABLE settings ADD COLUMN tag_colors TEXT")
        }

        // 010_add_archive_support
        migrator.registerMigration("010_add_archive_support") { db in
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN archived INTEGER NOT NULL DEFAULT 0")
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN archived_at TEXT")
            try db.create(indexOn: "notes", columns: ["archived"])
            try db.create(indexOn: "notes", columns: ["archived", "modified_at"])
        }

        // 011_add_lock_support
        migrator.registerMigration("011_add_lock_support") { db in
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN locked INTEGER NOT NULL DEFAULT 0")
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN locked_at TEXT")
        }

        // 012_add_deletions_tracking
        migrator.registerMigration("012_add_deletions_tracking") { db in
            try db.create(table: "note_deletions", ifNotExists: true) { t in
                t.column("id", .text).primaryKey()
                t.column("deleted_at", .text).notNull()
            }
        }

        // 013_add_pending_device_name
        migrator.registerMigration("013_add_pending_device_name") { db in
            try db.execute(sql: """
                ALTER TABLE sync_metadata ADD COLUMN pending_device_name TEXT
            """)
        }

        // 014_add_version_show_preview
        migrator.registerMigration("014_add_version_show_preview") { db in
            try db.execute(sql: "ALTER TABLE note_versions ADD COLUMN show_preview INTEGER")
        }

        // 015_add_hash_chain
        migrator.registerMigration("015_add_hash_chain") { db in
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN content_hash TEXT")
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN parent_hash TEXT")
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN hash_chain TEXT")
            try db.execute(sql: "ALTER TABLE note_versions ADD COLUMN content_hash TEXT")
            try db.execute(sql: "ALTER TABLE note_versions ADD COLUMN parent_hash TEXT")
        }

        // iOS-specific: add show_preview and needs_sync to notes
        migrator.registerMigration("ios_001_add_show_preview") { db in
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN show_preview INTEGER NOT NULL DEFAULT 0")
        }

        migrator.registerMigration("ios_002_add_needs_sync") { db in
            try db.execute(sql: "ALTER TABLE notes ADD COLUMN needs_sync INTEGER NOT NULL DEFAULT 0")
        }

        try migrator.migrate(dbPool)
    }
}
