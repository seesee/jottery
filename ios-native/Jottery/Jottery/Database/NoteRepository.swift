import CryptoKit
import Foundation
import GRDB

/// CRUD operations on notes with automatic encrypt/decrypt.
struct NoteRepository: Sendable {

    let db: DatabaseManager
    let versionRepo: VersionRepository

    // MARK: - Read

    /// Fetch all active (non-deleted, non-archived) notes, decrypted.
    func listActive(key: SymmetricKey) throws -> [DecryptedNote] {
        let records = try db.dbPool.read { db in
            try NoteRecord.filter(Column("deleted") == false && Column("archived") == false)
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
            }
        }
        if failCount > 0 {
            Log.debug("[NoteRepo] \(failCount)/\(records.count) notes failed to decrypt")
        }
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

    /// Return note IDs and their referenced attachment blob IDs for all non-deleted
    /// notes. Uses raw DB access — no decryption required.
    func listIdsAndAttachmentBlobIds() throws -> [(id: String, blobIds: [String])] {
        try db.dbPool.read { db in
            let rows = try Row.fetchAll(db, sql: """
                SELECT id, attachments FROM notes WHERE deleted = 0
            """)
            return rows.map { row in
                let id: String = row["id"]
                let json: String = row["attachments"]
                if json != "[]",
                   let data = json.data(using: .utf8),
                   let refs = try? JSONDecoder().decode([AttachmentRef].self, from: data),
                   !refs.isEmpty {
                    return (id, refs.map(\.data))
                }
                return (id, [])
            }
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

        // Compute content hash using sync-format tags
        let syncTags = try encryptTagsToSyncFormat(tags, key: key)
        let contentHash = HashChainService.computeContentHash(
            encryptedContent: contentJSON,
            encryptedTags: syncTags,
            attachmentIds: [],
            pinned: record.pinned,
            archived: record.archived,
            locked: record.locked,
            syntaxLanguage: record.syntaxLanguage,
            wordWrap: record.wordWrap,
            showPreview: record.showPreview,
            color: record.color
        )
        record.contentHash = contentHash
        record.parentHash = nil
        let hashChain = HashChainService.updateHashChain(currentHash: contentHash, parentChain: [])
        record.hashChain = String(data: try JSONEncoder().encode(hashChain), encoding: .utf8)

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

        // Fetch the current DB record so we can read the previous hash and version
        let currentRecord = try getRaw(id: note.id)
        let previousHash = currentRecord?.contentHash
        let previousChain: [String]
        if let chainStr = currentRecord?.hashChain, let data = chainStr.data(using: .utf8) {
            previousChain = (try? JSONDecoder().decode([String].self, from: data)) ?? []
        } else {
            previousChain = []
        }

        // Compute content hash using sync-format tags
        let syncTags = try encryptTagsToSyncFormat(note.tags, key: key)
        let contentHash = HashChainService.computeContentHash(
            encryptedContent: contentJSON,
            encryptedTags: syncTags,
            attachmentIds: note.attachments.map(\.id).sorted(),
            pinned: note.pinned,
            archived: note.archived,
            locked: note.locked,
            syntaxLanguage: note.syntaxLanguage,
            wordWrap: note.wordWrap,
            showPreview: note.showPreview,
            color: note.color
        )
        let parentHash = previousHash
        let hashChain = HashChainService.updateHashChain(currentHash: contentHash, parentChain: previousChain)
        let hashChainStr = String(data: try JSONEncoder().encode(hashChain), encoding: .utf8)

        // First-edit snapshot: when version is 0, create a snapshot of the original state
        // then advance version to 1
        let currentVersion = currentRecord?.version ?? note.version
        let newVersion: Int
        if currentVersion == 0, let currentRecord {
            try versionRepo.createVersion(from: currentRecord, reason: "manual-sync")
            newVersion = 1
        } else {
            newVersion = currentVersion
        }

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
                    version = ?,
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
                newVersion,
                note.wordWrap,
                note.syntaxLanguage,
                note.showPreview,
                note.color,
                contentHash,
                parentHash,
                hashChainStr,
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

    /// Hard-delete only when the note has no unsynced local changes.
    /// A remote deletion must never destroy an edit the server has not seen —
    /// the surviving note re-pushes and is resurrected server-side.
    /// Returns true if the row was deleted.
    func hardDeleteIfSynced(id: String) throws -> Bool {
        try db.dbPool.write { db in
            try db.execute(
                sql: "DELETE FROM notes WHERE id = ? AND needs_sync = 0",
                arguments: [id]
            )
            return db.changesCount > 0
        }
    }

    /// Archive a note.
    func archive(id: String) throws {
        let now = Date().iso8601
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET archived = 1, archived_at = ?, modified_at = ?, needs_sync = 1 WHERE id = ?
            """, arguments: [now, now, id])
        }
    }

    /// Unarchive a note.
    func unarchive(id: String) throws {
        let now = Date().iso8601
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET archived = 0, archived_at = NULL, modified_at = ?, needs_sync = 1 WHERE id = ?
            """, arguments: [now, id])
        }
    }

    /// Fetch all archived notes, decrypted.
    func listArchived(key: SymmetricKey) throws -> [DecryptedNote] {
        let records = try db.dbPool.read { db in
            try NoteRecord.filter(Column("archived") == true && Column("deleted") == false)
                .order(Column("archived_at").desc)
                .fetchAll(db)
        }
        return records.compactMap { try? decrypt($0, key: key) }
    }

    /// Toggle lock status.
    func toggleLock(id: String) throws {
        let now = Date().iso8601
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET
                    locked = NOT locked,
                    locked_at = CASE WHEN locked THEN NULL ELSE ? END,
                    modified_at = ?,
                    needs_sync = 1
                WHERE id = ?
            """, arguments: [now, now, id])
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

    /// Mark note as synced, optionally updating version from server.
    func markSynced(id: String, syncedAt: String, serverVersion: Int? = nil) throws {
        if let serverVersion {
            try db.dbPool.write { db in
                try db.execute(sql: """
                    UPDATE notes SET needs_sync = 0, synced_at = ?, version = ? WHERE id = ?
                """, arguments: [syncedAt, serverVersion, id])
            }
        } else {
            try db.dbPool.write { db in
                try db.execute(sql: """
                    UPDATE notes SET needs_sync = 0, synced_at = ? WHERE id = ?
                """, arguments: [syncedAt, id])
            }
        }
    }

    // MARK: - Attachment Management

    /// Add an attachment reference to a note's attachments JSON array.
    func addAttachment(noteId: String, ref: AttachmentRef) throws {
        guard var record = try getRaw(id: noteId) else { return }
        var attachments: [AttachmentRef] = []
        if let data = record.attachments.data(using: .utf8) {
            attachments = (try? JSONDecoder().decode([AttachmentRef].self, from: data)) ?? []
        }
        attachments.append(ref)
        let attachmentsJSON = try JSONEncoder().encode(attachments)
        record.attachments = String(data: attachmentsJSON, encoding: .utf8) ?? "[]"
        record.modifiedAt = Date().iso8601
        record.needsSync = true
        try updateRaw(record)
    }

    /// Remove an attachment reference from a note's attachments JSON array.
    func removeAttachment(noteId: String, attachmentId: String) throws {
        guard var record = try getRaw(id: noteId) else { return }
        var attachments: [AttachmentRef] = []
        if let data = record.attachments.data(using: .utf8) {
            attachments = (try? JSONDecoder().decode([AttachmentRef].self, from: data)) ?? []
        }
        attachments.removeAll { $0.id == attachmentId }
        let attachmentsJSON = try JSONEncoder().encode(attachments)
        record.attachments = String(data: attachmentsJSON, encoding: .utf8) ?? "[]"
        record.modifiedAt = Date().iso8601
        record.needsSync = true
        try updateRaw(record)
    }

    // MARK: - Insert Raw (for sync pull)

    /// Insert or replace a raw record (from server).
    func insertOrReplace(_ record: NoteRecord) throws {
        try db.dbPool.write { db in
            try record.save(db)
        }
    }

    // MARK: - Purge

    /// Permanently delete notes that have been in the recycle bin longer than the given interval.
    /// Returns the number of notes purged.
    @discardableResult
    func purgeOldDeletedNotes(olderThan interval: TimeInterval = 30 * 24 * 60 * 60) throws -> Int {
        let cutoff = Date().addingTimeInterval(-interval).iso8601
        return try db.dbPool.write { db in
            let count = try NoteRecord
                .filter(Column("deleted") == true)
                .filter(Column("deleted_at") != nil)
                .filter(Column("deleted_at") < cutoff)
                .fetchCount(db)
            try db.execute(sql: """
                DELETE FROM notes WHERE deleted = 1 AND deleted_at IS NOT NULL AND deleted_at < ?
            """, arguments: [cutoff])
            return count
        }
    }

    // MARK: - Private

    /// Encrypt plaintext tags to sync format (each tag JSON-encoded then individually encrypted).
    /// This is needed for content hash computation to match the web client.
    private func encryptTagsToSyncFormat(_ tags: [String], key: SymmetricKey) throws -> [String] {
        try tags.map { tag in
            try CryptoService.encryptTagForSync(tag, key: key)
        }
    }

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

        let rawAttachments: [AttachmentRef]
        if let data = record.attachments.data(using: .utf8) {
            rawAttachments = (try? JSONDecoder().decode([AttachmentRef].self, from: data)) ?? []
        } else {
            rawAttachments = []
        }

        // Decrypt attachment filenames (stored as encrypted JSON strings)
        let attachments = rawAttachments.map { ref -> AttachmentRef in
            var decrypted = ref
            if let enc = try? CryptoService.parseEncryptedJSON(ref.filename) {
                decrypted.filename = (try? CryptoService.decryptText(enc, key: key)) ?? ref.filename
            }
            return decrypted
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
