import Foundation
import GRDB

/// CRUD operations on version snapshots in the `note_versions` table.
struct VersionRepository: Sendable {

    let db: DatabaseManager

    // MARK: - Create

    /// Create a version snapshot from a NoteRecord.
    /// Deduplicates: skips if the latest version for this note already has the same contentHash.
    func createVersion(from record: NoteRecord, reason: String) throws {
        // Deduplicate: skip if latest version has same contentHash
        if let hash = record.contentHash {
            let existing = try db.dbPool.read { db in
                try NoteVersionRecord
                    .filter(Column("note_id") == record.id)
                    .order(Column("version").desc)
                    .limit(1)
                    .fetchOne(db)
            }
            if existing?.contentHash == hash {
                return
            }
        }

        let version = NoteVersionRecord.from(record, reason: reason)
        try db.dbPool.write { db in
            try version.save(db)
        }
    }

    // MARK: - Read

    /// Fetch all version snapshots for a note, ordered by version ascending.
    func getVersions(noteId: String) throws -> [NoteVersionRecord] {
        try db.dbPool.read { db in
            try NoteVersionRecord
                .filter(Column("note_id") == noteId)
                .order(Column("version").asc)
                .fetchAll(db)
        }
    }

    /// Fetch a specific version snapshot.
    func getVersion(noteId: String, version: Int) throws -> NoteVersionRecord? {
        let key = "\(noteId):\(version)"
        return try db.dbPool.read { db in
            try NoteVersionRecord.fetchOne(db, key: key)
        }
    }

    // MARK: - Upsert

    /// Insert or replace a version record (used when storing pulled versions from server).
    func insertOrReplace(_ record: NoteVersionRecord) throws {
        try db.dbPool.write { db in
            try record.save(db)
        }
    }

    // MARK: - Delete

    /// Delete all version snapshots for a note.
    func deleteVersions(noteId: String) throws {
        _ = try db.dbPool.write { db in
            try NoteVersionRecord
                .filter(Column("note_id") == noteId)
                .deleteAll(db)
        }
    }
}
