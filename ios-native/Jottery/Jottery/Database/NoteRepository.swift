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

    /// Count of notes in use that the server hasn't got.
    func countUnsynced() throws -> Int {
        try db.dbPool.read { db in
            try NoteRecord.filter(Column("deleted") == false && Column("needs_sync") == true).fetchCount(db)
        }
    }

    /// Count of active notes.
    func countActive() throws -> Int {
        try db.dbPool.read { db in
            try NoteRecord.filter(Column("deleted") == false).fetchCount(db)
        }
    }

    // MARK: - Create

    /// Create a new note with the given content. `id` is for a note that has to
    /// keep one it already has, and `fromInbox` marks an inbox item's note.
    func create(
        id: String? = nil,
        content: String,
        tags: [String] = [],
        fromInbox: Bool = false,
        copying original: DecryptedNote? = nil,
        attachments: [AttachmentRef] = [],
        syntaxLanguage: String? = nil,
        key: SymmetricKey
    ) throws -> DecryptedNote {
        let encContent = try CryptoService.encryptText(content, key: key)
        let encTags = try CryptoService.encryptStringArray(tags, key: key)

        let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
        let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)

        var record = NoteRecord.new(encryptedContent: contentJSON, encryptedTags: tagsJSON)
        if let id { record.id = id }
        record.fromInbox = fromInbox
        if let original {
            record.syntaxLanguage = original.syntaxLanguage
            record.wordWrap = original.wordWrap
            record.showPreview = original.showPreview
            record.color = original.color
        }
        if !attachments.isEmpty {
            record.attachments = String(decoding: try JSONEncoder().encode(attachments), as: UTF8.self)
        }
        if let syntaxLanguage { record.syntaxLanguage = syntaxLanguage }

        // Compute content hash using sync-format tags
        let syncTags = try encryptTagsToSyncFormat(tags, key: key)
        let contentHash = HashChainService.computeContentHash(
            encryptedContent: contentJSON,
            encryptedTags: syncTags,
            attachmentIds: attachments.map(\.id).sorted(),
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
    ///
    /// Attachments stay as they're stored. `note.attachments` holds decrypted
    /// filenames, so writing it back would put them in the clear, and sync would
    /// send them to the server. It can also be older than the stored list.
    /// `addAttachment` and `removeAttachment` are the only way to change them.
    func update(_ note: DecryptedNote, key: SymmetricKey) throws {
        let encContent = try CryptoService.encryptText(note.content, key: key)
        let encTags = try CryptoService.encryptStringArray(note.tags, key: key)

        let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
        let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)

        let now = Date().iso8601

        // Fetch the current DB record so we can read the previous hash and version
        let currentRecord = try getRaw(id: note.id)
        let attachmentsString = currentRecord?.attachments ?? "[]"
        let storedAttachments = (try? JSONDecoder().decode(
            [AttachmentRef].self, from: Data(attachmentsString.utf8)
        )) ?? []
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
            attachmentIds: storedAttachments.map(\.id).sorted(),
            pinned: note.pinned,
            archived: note.archived,
            locked: note.locked,
            syntaxLanguage: note.syntaxLanguage,
            wordWrap: note.wordWrap,
            showPreview: note.showPreview,
            color: note.color
        )
        let parentHash = previousHash
        // With nothing pending, the hash being replaced is the one the server has.
        let serverHash = currentRecord?.needsSync == false ? previousHash : currentRecord?.syncHash
        let hashChain = HashChainService.updateHashChain(
            currentHash: contentHash, parentChain: previousChain, keeping: serverHash
        )
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
                    sync_hash = ?,
                    from_inbox = from_inbox AND NOT ?,
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
                serverHash,
                note.showPreview,
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

    /// Soft-delete a note. Moves `modified_at` on, like any other change, so a
    /// push in flight can't mark it synced (see `markSynced`).
    func softDelete(id: String) throws {
        let now = Date().iso8601
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET
                    deleted = 1, deleted_at = ?, local_deleted_at = ?, modified_at = ?, needs_sync = 1
                WHERE id = ?
            """, arguments: [now, now, now, id])
        }
    }

    /// Soft-delete many notes in one transaction.
    func softDelete(ids: [String]) throws {
        let now = Date().iso8601
        try db.dbPool.write { db in
            try Self.softDelete(ids: ids, at: now, in: db)
        }
    }

    static func softDelete(ids: [String], at now: String, in db: Database) throws {
        for id in ids {
            try db.execute(sql: """
                UPDATE notes SET
                    deleted = 1, deleted_at = ?, local_deleted_at = ?, modified_at = ?, needs_sync = 1
                WHERE id = ?
            """, arguments: [now, now, now, id])
        }
    }

    /// Restore a soft-deleted note.
    func restore(id: String) throws {
        try db.dbPool.write { db in
            try db.execute(sql: """
                UPDATE notes SET
                    deleted = 0, deleted_at = NULL, local_deleted_at = NULL, modified_at = ?, needs_sync = 1
                WHERE id = ?
            """, arguments: [Date().iso8601, id])
        }
    }

    /// Delete a note, its version history and any attachment nothing else
    /// uses, and queue the deletion so other devices hear about it. One
    /// transaction, so a note can't go without the deletion being recorded.
    func permanentlyDelete(id: String, deletedAt: String = Date().iso8601) throws {
        try db.dbPool.write { db in
            try Self.permanentlyDelete(id: id, deletedAt: deletedAt, in: db)
        }
    }

    private static func permanentlyDelete(id: String, deletedAt: String, in db: Database) throws {
        let attachments = try String.fetchOne(db, sql: "SELECT attachments FROM notes WHERE id = ?", arguments: [id])
        guard let attachments else { return }
        let versions = try String.fetchAll(
            db, sql: "SELECT attachments FROM note_versions WHERE note_id = ?", arguments: [id]
        )

        try db.execute(sql: "DELETE FROM notes WHERE id = ?", arguments: [id])
        try db.execute(sql: "DELETE FROM note_versions WHERE note_id = ?", arguments: [id])
        try SyncRepository.recordDeletion(id: id, deletedAt: deletedAt, in: db)
        try removeUnreferencedBlobs(listedIn: [attachments] + versions, in: db)
    }

    /// Delete the blobs in these attachment lists that nothing refers to any
    /// more. Several notes can share an attachment, and a version of another
    /// note can still need it. Ids are UUIDs, so a substring match is exact.
    private static func removeUnreferencedBlobs(listedIn lists: [String], in db: Database) throws {
        let blobIds = Set(lists.flatMap { json -> [String] in
            let refs = try? JSONDecoder().decode([AttachmentRef].self, from: Data(json.utf8))
            return refs?.map(\.data) ?? []
        })
        for blobId in blobIds {
            try db.execute(sql: """
                DELETE FROM attachments WHERE id = ?1
                    AND NOT EXISTS (SELECT 1 FROM notes WHERE instr(attachments, ?1) > 0)
                    AND NOT EXISTS (SELECT 1 FROM note_versions WHERE instr(attachments, ?1) > 0)
            """, arguments: [blobId])
        }
    }

    /// Replace a note in one step, tidying up the attachments it had.
    func replace(with record: NoteRecord) throws {
        try db.dbPool.write { db in
            let previous = try String.fetchOne(
                db, sql: "SELECT attachments FROM notes WHERE id = ?", arguments: [record.id]
            )
            try record.save(db)
            try Self.removeUnreferencedBlobs(listedIn: [previous].compactMap { $0 }, in: db)
        }
    }

    /// What became of a note the server listed as permanently deleted.
    enum ServerDeletionResult: Equatable {
        /// It was in use, and has moved to the bin.
        case movedToBin
        /// It was in the bin already, or isn't here.
        case nothingToDo
        /// It has an edit the server hasn't seen, which outranks the deletion.
        case keptLocalEdit
    }

    /// Apply a permanent deletion from the server, which moves the note to the
    /// bin rather than deleting it. Tombstones aren't authenticated, so if a
    /// server deleted everything outright, every device would lose every note.
    /// The bin gives the user time to notice, and restoring a note pushes it
    /// back. The purge deletes it in the end, going by this device's clock.
    func applyServerDeletion(id: String) throws -> ServerDeletionResult {
        let now = Date().iso8601
        return try db.dbPool.write { db in
            guard let row = try Row.fetchOne(
                db, sql: "SELECT deleted, needs_sync FROM notes WHERE id = ?", arguments: [id]
            ) else {
                return .nothingToDo
            }
            let isDeleted: Bool = row["deleted"]
            let needsSync: Bool = row["needs_sync"]
            if needsSync { return .keptLocalEdit }
            if isDeleted { return .nothingToDo }
            try db.execute(sql: """
                UPDATE notes SET deleted = 1, deleted_at = ?, local_deleted_at = ? WHERE id = ?
            """, arguments: [now, now, id])
            return .movedToBin
        }
    }

    /// The server skipped this note because it was deleted for good after the
    /// edit `sentModifiedAt` was sent. That edit is what's being set aside, so
    /// unlike `applyServerDeletion` the waiting flag doesn't save it, but a newer
    /// edit does: that one is later than the deletion, and the next push brings
    /// the note back.
    func applySkippedDeletion(id: String, sentModifiedAt: String?) throws -> ServerDeletionResult {
        let now = Date().iso8601
        return try db.dbPool.write { db in
            guard let row = try Row.fetchOne(
                db, sql: "SELECT deleted, modified_at FROM notes WHERE id = ?", arguments: [id]
            ) else {
                return .nothingToDo
            }
            let modifiedAt: String = row["modified_at"]
            guard modifiedAt == sentModifiedAt else { return .keptLocalEdit }
            let isDeleted: Bool = row["deleted"]
            try db.execute(sql: """
                UPDATE notes SET deleted = 1, deleted_at = COALESCE(deleted_at, ?),
                    local_deleted_at = COALESCE(local_deleted_at, ?), needs_sync = 0
                WHERE id = ?
            """, arguments: [now, now, id])
            return isDeleted ? .nothingToDo : .movedToBin
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

    /// Archive many notes in one transaction.
    func archive(ids: [String]) throws {
        let now = Date().iso8601
        try db.dbPool.write { db in
            for id in ids {
                try db.execute(sql: """
                    UPDATE notes SET archived = 1, archived_at = ?, modified_at = ?, needs_sync = 1 WHERE id = ?
                """, arguments: [now, now, id])
            }
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
    ///
    /// `synced_at`/`version` always update. `needs_sync` is cleared only when
    /// `ifModifiedAt` is nil (unconditional, for legacy callers) or matches the
    /// row's current `modified_at` — i.e. the note hasn't been edited again
    /// since the snapshot that was actually pushed. This stops an edit made
    /// during the push round-trip from being clobbered back to "synced" when
    /// the server never saw it.
    func markSynced(
        id: String,
        syncedAt: String,
        serverVersion: Int? = nil,
        ifModifiedAt: String? = nil,
        pushedHash: String? = nil
    ) throws {
        try db.dbPool.write { db in
            if let pushedHash {
                try db.execute(sql: "UPDATE notes SET sync_hash = ? WHERE id = ?", arguments: [pushedHash, id])
            }
            if let serverVersion {
                try db.execute(sql: """
                    UPDATE notes SET synced_at = ?, version = ? WHERE id = ?
                """, arguments: [syncedAt, serverVersion, id])
            } else {
                try db.execute(sql: """
                    UPDATE notes SET synced_at = ? WHERE id = ?
                """, arguments: [syncedAt, id])
            }
            if let ifModifiedAt {
                try db.execute(sql: """
                    UPDATE notes SET needs_sync = 0 WHERE id = ? AND modified_at = ?
                """, arguments: [id, ifModifiedAt])
            } else {
                try db.execute(sql: "UPDATE notes SET needs_sync = 0 WHERE id = ?", arguments: [id])
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

    /// Encrypt any filename older builds stored in plaintext, in notes and in
    /// their version snapshots, and queue those notes for a push so the server's
    /// copy is replaced too (versions go up with their note). Returns how many
    /// notes that was. Does nothing once everything is encrypted, so it can run
    /// on every unlock. `modified_at` is left alone: the note hasn't been edited.
    @discardableResult
    func encryptPlaintextAttachmentFilenames(key: SymmetricKey) throws -> Int {
        try db.dbPool.write { db in
            var noteIds = Set<String>()

            let notes = try Row.fetchAll(db, sql: "SELECT id, attachments FROM notes WHERE attachments != '[]'")
            for row in notes {
                let id: String = row["id"]
                guard let json = try Self.repairedAttachments(row["attachments"], key: key) else { continue }
                try db.execute(sql: "UPDATE notes SET attachments = ? WHERE id = ?", arguments: [json, id])
                noteIds.insert(id)
            }

            let versions = try Row.fetchAll(db, sql: """
                SELECT version_key, note_id, attachments FROM note_versions WHERE attachments != '[]'
            """)
            for row in versions {
                let versionKey: String = row["version_key"]
                guard let json = try Self.repairedAttachments(row["attachments"], key: key) else { continue }
                try db.execute(
                    sql: "UPDATE note_versions SET attachments = ? WHERE version_key = ?",
                    arguments: [json, versionKey]
                )
                noteIds.insert(row["note_id"])
            }

            for id in noteIds {
                try db.execute(sql: "UPDATE notes SET needs_sync = 1 WHERE id = ?", arguments: [id])
            }
            return noteIds.count
        }
    }

    /// The JSON to store instead of `json`, or nil if it needs no change.
    private static func repairedAttachments(_ json: String, key: SymmetricKey) throws -> String? {
        guard let refs = try? JSONDecoder().decode([AttachmentRef].self, from: Data(json.utf8)) else { return nil }
        let repaired = try AttachmentFilename.encryptingPlaintext(in: refs, key: key)
        guard repaired.changed else { return nil }
        return String(data: try JSONEncoder().encode(repaired.refs), encoding: .utf8)
    }

    // MARK: - Insert Raw (for sync pull)

    /// Insert or replace a raw record (from server).
    func insertOrReplace(_ record: NoteRecord) throws {
        try db.dbPool.write { db in
            try record.save(db)
        }
    }

    /// Write several notes as one change: all of them, or none.
    func insertOrReplace(_ records: [NoteRecord]) throws {
        try db.dbPool.write { db in
            for record in records {
                try record.save(db)
            }
        }
    }

    /// Write a batch of restored notes in one transaction. A note here that's
    /// at least as new as the backup's copy stays as it is: it holds edits the
    /// backup doesn't, and they'd otherwise be overwritten and synced everywhere.
    func restore(_ records: [NoteRecord]) throws -> (restored: Int, keptNewer: Int) {
        try db.dbPool.write { db in
            var restored = 0
            var keptNewer = 0
            for record in records {
                if let existing = try NoteRecord.fetchOne(db, key: record.id),
                   !Date.isLater(record.modifiedAt, than: existing.modifiedAt) {
                    keptNewer += 1
                } else {
                    try record.save(db)
                    restored += 1
                }
            }
            return (restored, keptNewer)
        }
    }

    // MARK: - Purge

    /// Permanently delete notes that have been in the recycle bin longer than
    /// the given interval, by this device's clock: a date from the server can't
    /// make a note skip the bin. A note with a change waiting to go up stays.
    /// Returns the number of notes purged.
    ///
    /// The deletion is sent with the time the note was binned, so an edit made
    /// elsewhere since then still beats it.
    @discardableResult
    func purgeOldDeletedNotes(olderThan interval: TimeInterval = 30 * 24 * 60 * 60) throws -> Int {
        let cutoff = Date().addingTimeInterval(-interval).iso8601
        return try db.dbPool.write { db in
            let rows = try Row.fetchAll(db, sql: """
                SELECT id, deleted_at FROM notes
                WHERE deleted = 1 AND needs_sync = 0 AND local_deleted_at IS NOT NULL AND local_deleted_at < ?
            """, arguments: [cutoff])
            for row in rows {
                let id: String = row["id"]
                let deletedAt: String? = row["deleted_at"]
                try Self.permanentlyDelete(id: id, deletedAt: deletedAt ?? cutoff, in: db)
            }
            return rows.count
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
    func decrypt(_ record: NoteRecord, key: SymmetricKey) throws -> DecryptedNote {
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
            decrypted.filename = AttachmentFilename.display(stored: ref.filename, key: key)
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
