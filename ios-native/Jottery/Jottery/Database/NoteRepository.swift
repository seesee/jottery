import CryptoKit
import Foundation
import GRDB

/// CRUD operations on notes with automatic encrypt/decrypt.
struct NoteRepository: Sendable {

    let db: DatabaseManager

    // MARK: - Read

    /// Fetch all active (non-deleted) notes, decrypted.
    func listActive(key: SymmetricKey) throws -> [DecryptedNote] {
        let records = try db.dbPool.read { db in
            try NoteRecord.filter(Column("deleted") == false)
                .order(Column("modified_at").desc)
                .fetchAll(db)
        }
        var decrypted: [DecryptedNote] = []
        var failCount = 0
        for record in records {
            do {
                decrypted.append(try decrypt(record, key: key))
            } catch {
                failCount += 1
                if failCount <= 3 {
                    print("[NoteRepo] Failed to decrypt note \(record.id): \(error)")
                    print("[NoteRepo]   content prefix: \(String(record.content.prefix(80)))")
                    print("[NoteRepo]   tags prefix: \(String(record.tags.prefix(80)))")
                }
            }
        }
        print("[NoteRepo] Loaded \(decrypted.count)/\(records.count) notes (\(failCount) failed)")
        return decrypted
    }

    /// Fetch all deleted notes, decrypted (recycle bin).
    func listDeleted(key: SymmetricKey) throws -> [DecryptedNote] {
        let records = try db.dbPool.read { db in
            try NoteRecord.filter(Column("deleted") == true)
                .order(Column("deleted_at").desc)
                .fetchAll(db)
        }
        return records.compactMap { try? decrypt($0, key: key) }
    }

    /// Fetch a single note by ID, decrypted.
    func get(id: String, key: SymmetricKey) throws -> DecryptedNote? {
        guard let record = try db.dbPool.read({ db in
            try NoteRecord.fetchOne(db, key: id)
        }) else { return nil }
        return try decrypt(record, key: key)
    }

    /// Fetch a raw (encrypted) record by ID.
    func getRaw(id: String) throws -> NoteRecord? {
        try db.dbPool.read { db in
            try NoteRecord.fetchOne(db, key: id)
        }
    }

    /// Fetch all records that need syncing.
    func listNeedingSync() throws -> [NoteRecord] {
        try db.dbPool.read { db in
            try NoteRecord.filter(Column("needs_sync") == true).fetchAll(db)
        }
    }

    /// Count of active notes.
    func countActive() throws -> Int {
        try db.dbPool.read { db in
            try NoteRecord.filter(Column("deleted") == false).fetchCount(db)
        }
    }

    // MARK: - Create

    /// Create a new note with the given content.
    func create(content: String, tags: [String] = [], key: SymmetricKey) throws -> DecryptedNote {
        let encContent = try CryptoService.encryptText(content, key: key)
        let encTags = try CryptoService.encryptStringArray(tags, key: key)

        let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
        let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)

        var record = NoteRecord.new(encryptedContent: contentJSON, encryptedTags: tagsJSON)

        try db.dbPool.write { db in
            try record.insert(db)
        }

        return try decrypt(record, key: key)
    }

    // MARK: - Update

    /// Update a decrypted note's content and tags back to the database.
    func update(_ note: DecryptedNote, key: SymmetricKey) throws {
        let encContent = try CryptoService.encryptText(note.content, key: key)
        let encTags = try CryptoService.encryptStringArray(note.tags, key: key)

        let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
        let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)

        let now = Date().iso8601
        let attachmentsJSON = try JSONEncoder().encode(note.attachments)
        let attachmentsString = String(data: attachmentsJSON, encoding: .utf8) ?? "[]"

        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET
                    content = ?,
                    tags = ?,
                    modified_at = ?,
                    attachments = ?,
                    pinned = ?,
                    archived = ?,
                    archived_at = ?,
                    locked = ?,
                    locked_at = ?,
                    deleted = ?,
                    deleted_at = ?,
                    version = version + 1,
                    word_wrap = ?,
                    syntax_language = ?,
                    show_preview = ?,
                    color = ?,
                    content_hash = ?,
                    parent_hash = ?,
                    hash_chain = ?,
                    needs_sync = 1
                WHERE id = ?
            """, arguments: [
                contentJSON,
                tagsJSON,
                now,
                attachmentsString,
                note.pinned,
                note.archived,
                note.archivedAt?.iso8601,
                note.locked,
                note.lockedAt?.iso8601,
                note.deleted,
                note.deletedAt?.iso8601,
                note.wordWrap,
                note.syntaxLanguage,
                note.showPreview,
                note.color,
                note.contentHash,
                note.parentHash,
                note.hashChain.isEmpty ? nil : try? String(data: JSONEncoder().encode(note.hashChain), encoding: .utf8),
                note.id,
            ])
        }
    }

    /// Update a raw record (used during sync).
    func updateRaw(_ record: NoteRecord) throws {
        try db.dbPool.write { db in
            try record.update(db)
        }
    }

    /// Soft-delete a note.
    func softDelete(id: String) throws {
        let now = Date().iso8601
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET deleted = 1, deleted_at = ?, needs_sync = 1 WHERE id = ?
            """, arguments: [now, id])
        }
    }

    /// Restore a soft-deleted note.
    func restore(id: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET deleted = 0, deleted_at = NULL, needs_sync = 1 WHERE id = ?
            """, arguments: [id])
        }
    }

    /// Hard-delete a note (permanent).
    func hardDelete(id: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: "DELETE FROM notes WHERE id = ?", arguments: [id])
        }
    }

    /// Toggle pin status.
    func togglePin(id: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET pinned = NOT pinned, modified_at = ?, needs_sync = 1 WHERE id = ?
            """, arguments: [Date().iso8601, id])
        }
    }

    /// Mark note as synced.
    func markSynced(id: String, syncedAt: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET needs_sync = 0, synced_at = ? WHERE id = ?
            """, arguments: [syncedAt, id])
        }
    }

    // MARK: - Insert Raw (for sync pull)

    /// Insert or replace a raw record (from server).
    func insertOrReplace(_ record: NoteRecord) throws {
        try db.dbPool.write { db in
            try record.save(db)
        }
    }

    // MARK: - Private

    /// Decrypt a `NoteRecord` to a `DecryptedNote`.
    private func decrypt(_ record: NoteRecord, key: SymmetricKey) throws -> DecryptedNote {
        let encContent = try CryptoService.parseEncryptedJSON(record.content)
        let content = try CryptoService.decryptText(encContent, key: key)

        let encTags = try CryptoService.parseEncryptedJSON(record.tags)
        let tags: [String]
        let decryptedTagsText = try CryptoService.decryptText(encTags, key: key)
        if decryptedTagsText.isEmpty {
            tags = []
        } else if decryptedTagsText.hasPrefix("[") {
            tags = (try? JSONDecoder().decode([String].self, from: Data(decryptedTagsText.utf8))) ?? []
        } else {
            // Plain string — single tag or comma-separated
            tags = decryptedTagsText.split(separator: ",")
                .map { String($0).trimmingCharacters(in: .whitespaces) }
                .filter { !$0.isEmpty }
        }

        let attachments: [AttachmentRef]
        if let data = record.attachments.data(using: .utf8) {
            attachments = (try? JSONDecoder().decode([AttachmentRef].self, from: data)) ?? []
        } else {
            attachments = []
        }

        let hashChain: [String]
        if let chainStr = record.hashChain, let data = chainStr.data(using: .utf8) {
            hashChain = (try? JSONDecoder().decode([String].self, from: data)) ?? []
        } else {
            hashChain = []
        }

        return DecryptedNote(
            id: record.id,
            createdAt: Date(iso8601: record.createdAt) ?? Date(),
            modifiedAt: Date(iso8601: record.modifiedAt) ?? Date(),
            syncedAt: record.syncedAt.flatMap { Date(iso8601: $0) },
            content: content,
            tags: tags,
            attachments: attachments,
            pinned: record.pinned,
            archived: record.archived,
            archivedAt: record.archivedAt.flatMap { Date(iso8601: $0) },
            locked: record.locked,
            lockedAt: record.lockedAt.flatMap { Date(iso8601: $0) },
            deleted: record.deleted,
            deletedAt: record.deletedAt.flatMap { Date(iso8601: $0) },
            version: record.version,
            wordWrap: record.wordWrap,
            syntaxLanguage: record.syntaxLanguage,
            showPreview: record.showPreview,
            color: record.color,
            contentHash: record.contentHash,
            parentHash: record.parentHash,
            hashChain: hashChain,
            needsSync: record.needsSync,
            decryptedAt: Date()
        )
    }
}
