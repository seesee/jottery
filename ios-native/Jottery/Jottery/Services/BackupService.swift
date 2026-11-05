import Compression
import CryptoKit
import Foundation
import GRDB

/// Encrypted backup service. Writes and reads the v3.0 format that the web
/// client's `backupService.ts` defines, so a backup from either client restores
/// on the other.
///
/// Format, with each batch on a line of its own and the keys in this order,
/// which the web client's streaming reader relies on:
/// ```json
/// {
/// "version": "3.0",
/// "type": "jottery-encrypted-backup",
/// "createdAt": "ISO 8601",
/// "encryption": { "envelopeVersion": 1, "deviceSalt": "base64", "localWrappedMaster": "json", ... },
/// "batches": [
/// { "type": "notes", "index": 0, "count": N, "data": "JWE compact" }
/// ],
/// "summary": { "notes": N, "attachments": N, "versions": N, "savedSearches": N }
/// }
/// ```
///
/// Each batch is: JSON array → zlib (RFC 1950) → AES-256-GCM → JWE compact
/// (RFC 7516), with the protected header as authenticated data. Items keep the
/// web's shape: `tags` and `attachments` are arrays, and `tags` holds one blob
/// with every tag in it.
///
/// Builds before this format was adopted sealed batches with no authenticated
/// data, compressed them with raw DEFLATE, and stored `tags`, `attachments` and
/// `hashChain` as strings. Those backups are still read.
///
/// A vault can be far larger than memory, so nothing here holds more than a
/// batch: backups are written to a file as they're made, and read from one.
enum BackupService {

    // MARK: - Constants

    static let noteBatchSize = 250
    static let attachmentBatchSize = 50
    static let versionBatchSize = 250

    /// A batch of attachments closes at this many bytes of blobs, however few
    /// there are: fifty full-size videos won't fit in memory.
    static let attachmentBatchBytes = 16 * 1024 * 1024

    private static let formatVersion = "3.0"
    private static let formatType = "jottery-encrypted-backup"

    /// How far a backup or restore has got, in items or batches.
    struct Progress: Sendable, Equatable {
        let completed: Int
        let total: Int
    }

    /// What a restore did.
    struct RestoreResult: Sendable, Equatable {
        var notes = 0
        /// Items in the file that couldn't be read, and were left out.
        var skipped = 0
        /// Notes left alone, because this device's copy is at least as new.
        var keptNewer = 0
    }

    // MARK: - Backup Data Structures

    /// How to get from the password to the key the contents are encrypted with.
    /// Web, iOS and the TUI all write and read this, so keep them in step.
    ///
    /// An envelope vault encrypts with a random master key, so the backup carries
    /// it wrapped with the password, as the vault stores it. A legacy vault's key
    /// comes straight from the password, salt and iterations.
    struct BackupEncryption: Codable {
        var envelopeVersion: Int?
        var deviceSalt: String?
        var localWrappedMaster: String?
        var wrappingKdfVersion: Int?
        var salt: String?
        var iterations: Int?
        var createdAt: String?
        let algorithm: String

        init(metadata: EncryptionMetadata) {
            if metadata.envelopeVersion != nil {
                envelopeVersion = metadata.envelopeVersion
                deviceSalt = metadata.deviceSalt
                localWrappedMaster = metadata.localWrappedMaster
                wrappingKdfVersion = metadata.wrappingKdfVersion
                algorithm = metadata.algorithm
            } else {
                salt = metadata.salt
                iterations = metadata.iterations
                algorithm = "PBKDF2"
            }
            createdAt = metadata.createdAt
        }
    }

    struct BatchRecord: Codable {
        let type: String
        let index: Int
        let count: Int
        let data: String  // JWE compact serialisation
    }

    struct BackupSummary: Codable {
        let notes: Int
        let attachments: Int
        let versions: Int
        let savedSearches: Int
    }

    // MARK: - Batch Item Types

    struct BackupNote: Codable {
        let id: String
        let createdAt: String
        let modifiedAt: String
        let syncedAt: String?
        let content: String
        let tags: [String]
        let attachments: [AttachmentRef]
        let pinned: Bool
        let archived: Bool
        let archivedAt: String?
        let locked: Bool
        let lockedAt: String?
        let deleted: Bool
        let deletedAt: String?
        let version: Int
        let wordWrap: Bool
        let syntaxLanguage: String
        let showPreview: Bool
        let color: String?
        let contentHash: String?
        let parentHash: String?
        let hashChain: [String]?
        /// Kept for the web client, which restores a backup as it stands.
        /// Restoring here always queues the note for a push.
        let needsSync: Bool?

        init(record: NoteRecord) {
            id = record.id
            createdAt = record.createdAt
            modifiedAt = record.modifiedAt
            syncedAt = record.syncedAt
            content = record.content
            tags = [record.tags]
            attachments = decodedRefs(record.attachments)
            pinned = record.pinned
            archived = record.archived
            archivedAt = record.archivedAt
            locked = record.locked
            lockedAt = record.lockedAt
            deleted = record.deleted
            deletedAt = record.deletedAt
            version = record.version
            wordWrap = record.wordWrap
            syntaxLanguage = record.syntaxLanguage
            showPreview = record.showPreview
            color = record.color
            contentHash = record.contentHash
            parentHash = record.parentHash
            hashChain = record.hashChain.flatMap { try? JSONDecoder().decode([String].self, from: Data($0.utf8)) }
            needsSync = record.needsSync
        }

        /// Web notes leave out what they don't use, and old iOS backups hold
        /// arrays as strings.
        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            id = try container.decode(String.self, forKey: .id)
            createdAt = try container.decode(String.self, forKey: .createdAt)
            modifiedAt = try container.decode(String.self, forKey: .modifiedAt)
            syncedAt = try container.decodeIfPresent(String.self, forKey: .syncedAt)
            content = try container.decode(String.self, forKey: .content)
            tags = try container.decodeTags(forKey: .tags)
            attachments = try container.decodeArrayOrJSONString(AttachmentRef.self, forKey: .attachments) ?? []
            pinned = try container.decodeIfPresent(Bool.self, forKey: .pinned) ?? false
            archived = try container.decodeIfPresent(Bool.self, forKey: .archived) ?? false
            archivedAt = try container.decodeIfPresent(String.self, forKey: .archivedAt)
            locked = try container.decodeIfPresent(Bool.self, forKey: .locked) ?? false
            lockedAt = try container.decodeIfPresent(String.self, forKey: .lockedAt)
            deleted = try container.decodeIfPresent(Bool.self, forKey: .deleted) ?? false
            deletedAt = try container.decodeIfPresent(String.self, forKey: .deletedAt)
            version = try container.decodeIfPresent(Int.self, forKey: .version) ?? 0
            wordWrap = try container.decodeIfPresent(Bool.self, forKey: .wordWrap) ?? true
            syntaxLanguage = try container.decodeIfPresent(String.self, forKey: .syntaxLanguage) ?? "markdown"
            showPreview = try container.decodeIfPresent(Bool.self, forKey: .showPreview) ?? false
            color = try container.decodeIfPresent(String.self, forKey: .color)
            contentHash = try container.decodeIfPresent(String.self, forKey: .contentHash)
            parentHash = try container.decodeIfPresent(String.self, forKey: .parentHash)
            hashChain = try container.decodeArrayOrJSONString(String.self, forKey: .hashChain)
            needsSync = try container.decodeIfPresent(Bool.self, forKey: .needsSync)
        }
    }

    struct BackupAttachment: Codable {
        let id: String
        let blob: String  // base64 of encrypted blob data
    }

    struct BackupVersion: Codable {
        let versionKey: String
        let noteId: String
        let version: Int
        let createdAt: String
        let syncedAt: String?
        let content: String
        let tags: [String]
        let attachments: [AttachmentRef]
        let syntaxLanguage: String?
        let wordWrap: Bool?
        let showPreview: Bool?
        let color: String?
        let reason: String?
        let contentHash: String?
        let parentHash: String?

        init(record: NoteVersionRecord) {
            versionKey = record.versionKey
            noteId = record.noteId
            version = record.version
            createdAt = record.createdAt
            syncedAt = record.syncedAt
            content = record.content
            tags = [record.tags]
            attachments = decodedRefs(record.attachments)
            syntaxLanguage = record.syntaxLanguage
            wordWrap = record.wordWrap
            showPreview = record.showPreview
            color = record.color
            reason = record.reason
            contentHash = record.contentHash
            parentHash = record.parentHash
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            versionKey = try container.decode(String.self, forKey: .versionKey)
            noteId = try container.decode(String.self, forKey: .noteId)
            version = try container.decode(Int.self, forKey: .version)
            createdAt = try container.decode(String.self, forKey: .createdAt)
            syncedAt = try container.decodeIfPresent(String.self, forKey: .syncedAt)
            content = try container.decode(String.self, forKey: .content)
            tags = try container.decodeTags(forKey: .tags)
            attachments = try container.decodeArrayOrJSONString(AttachmentRef.self, forKey: .attachments) ?? []
            syntaxLanguage = try container.decodeIfPresent(String.self, forKey: .syntaxLanguage)
            wordWrap = try container.decodeIfPresent(Bool.self, forKey: .wordWrap)
            showPreview = try container.decodeIfPresent(Bool.self, forKey: .showPreview)
            color = try container.decodeIfPresent(String.self, forKey: .color)
            reason = try container.decodeIfPresent(String.self, forKey: .reason)
            contentHash = try container.decodeIfPresent(String.self, forKey: .contentHash)
            parentHash = try container.decodeIfPresent(String.self, forKey: .parentHash)
        }
    }

    struct BackupSavedSearch: Codable {
        let id: String
        let name: String
        let query: String
        let order: Int
        let createdAt: String
        let modifiedAt: String
        let syncedAt: String?
        let deleted: Bool
        let deletedAt: String?
        let version: Int
        let needsSync: Bool?

        init(record: SavedSearchRepository.SavedSearchRecord) {
            id = record.id
            name = record.name
            query = record.query
            order = record.displayOrder
            createdAt = record.createdAt
            modifiedAt = record.modifiedAt
            syncedAt = record.syncedAt
            deleted = record.deleted
            deletedAt = record.deletedAt
            version = record.version
            needsSync = record.needsSync
        }
    }

    // MARK: - Create Backup

    /// Write an encrypted backup of all data to `url`, a batch at a time.
    /// Run this off the main actor: it does the work of the whole vault.
    /// Stops with `CancellationError` if the current task is cancelled.
    @discardableResult
    static func writeBackup(
        to url: URL,
        noteRepo: NoteRepository,
        attachmentRepo: AttachmentRepository,
        savedSearchRepo: SavedSearchRepository,
        encryptionRepo: EncryptionRepository,
        key: SymmetricKey,
        maxAttachmentBatchBytes: Int = attachmentBatchBytes,
        progress: (@Sendable (Progress) -> Void)? = nil
    ) throws -> BackupSummary {
        guard let metadata = try encryptionRepo.get() else {
            throw BackupError.noVault
        }

        // Everything goes in, including deleted notes, as raw encrypted records.
        let database = noteRepo.db.dbPool
        let noteIds = try database.read { try String.fetchAll($0, sql: "SELECT id FROM notes") }
        let versionKeys = try database.read { try String.fetchAll($0, sql: "SELECT version_key FROM note_versions") }
        let blobIds = try attachmentRepo.listBlobIds()
        let savedSearches = try savedSearchRepo.db.dbPool.read {
            try SavedSearchRepository.SavedSearchRecord.fetchAll($0)
        }

        let total = noteIds.count + versionKeys.count + blobIds.count + savedSearches.count
        var completed = 0
        func advance(by count: Int) {
            completed += count
            progress?(Progress(completed: completed, total: total))
        }

        var writer = try BackupWriter(url: url, encryption: BackupEncryption(metadata: metadata))
        defer { writer.close() }

        for (index, start) in stride(from: 0, to: noteIds.count, by: noteBatchSize).enumerated() {
            try Task.checkCancellation()
            try autoreleasepool {
                let ids = noteIds[start..<min(start + noteBatchSize, noteIds.count)]
                let records = try database.read { try NoteRecord.filter(keys: Array(ids)).fetchAll($0) }
                let items = records.map { BackupNote(record: $0) }
                try writer.append(type: "notes", index: index, count: items.count, data: encryptBatch(items, key: key))
                advance(by: ids.count)
            }
        }

        var attachmentCount = 0
        var pending: [BackupAttachment] = []
        var pendingBytes = 0
        var attachmentBatchIndex = 0
        func flushAttachments() throws {
            guard !pending.isEmpty else { return }
            try autoreleasepool {
                try writer.append(
                    type: "attachments", index: attachmentBatchIndex, count: pending.count,
                    data: encryptBatch(pending, key: key)
                )
            }
            attachmentCount += pending.count
            attachmentBatchIndex += 1
            pending = []
            pendingBytes = 0
        }
        for blobId in blobIds {
            try Task.checkCancellation()
            try autoreleasepool {
                if let blob = try attachmentRepo.getBlob(id: blobId) {
                    pending.append(BackupAttachment(id: blobId, blob: blob.base64EncodedString()))
                    pendingBytes += blob.count
                }
            }
            if pending.count >= attachmentBatchSize || pendingBytes >= maxAttachmentBatchBytes {
                try flushAttachments()
            }
            advance(by: 1)
        }
        try flushAttachments()

        for (index, start) in stride(from: 0, to: versionKeys.count, by: versionBatchSize).enumerated() {
            try Task.checkCancellation()
            try autoreleasepool {
                let keys = versionKeys[start..<min(start + versionBatchSize, versionKeys.count)]
                let records = try database.read { try NoteVersionRecord.filter(keys: Array(keys)).fetchAll($0) }
                let items = records.map { BackupVersion(record: $0) }
                try writer.append(type: "versions", index: index, count: items.count, data: encryptBatch(items, key: key))
                advance(by: keys.count)
            }
        }

        if !savedSearches.isEmpty {
            let items = savedSearches.map { BackupSavedSearch(record: $0) }
            try writer.append(type: "saved_searches", index: 0, count: items.count, data: encryptBatch(items, key: key))
            advance(by: items.count)
        }

        let summary = BackupSummary(
            notes: noteIds.count,
            attachments: attachmentCount,
            versions: versionKeys.count,
            savedSearches: savedSearches.count
        )
        try writer.finish(summary: summary)
        return summary
    }

    /// Writes the file in the layout `downloadBackup` in `backupService.ts` uses.
    private struct BackupWriter {
        private let handle: FileHandle
        private var wroteBatch = false

        init(url: URL, encryption: BackupEncryption) throws {
            guard FileManager.default.fileExists(atPath: url.path)
                    || FileManager.default.createFile(atPath: url.path, contents: nil) else {
                throw CocoaError(.fileWriteUnknown)
            }
            handle = try FileHandle(forWritingTo: url)
            try handle.truncate(atOffset: 0)

            let encoder = JSONEncoder()
            encoder.outputFormatting = [.sortedKeys]
            var header = Data("{\n\"version\":\"\(formatVersion)\",\n\"type\":\"\(formatType)\"".utf8)
            header.append(Data(",\n\"createdAt\":\"\(Date().iso8601)\",\n\"encryption\":".utf8))
            header.append(try encoder.encode(encryption))
            header.append(Data(",\n\"batches\":[\n".utf8))
            try handle.write(contentsOf: header)
        }

        mutating func append(type: String, index: Int, count: Int, data: String) throws {
            if wroteBatch {
                try handle.write(contentsOf: Data(",\n".utf8))
            }
            wroteBatch = true
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.sortedKeys]
            let record = BatchRecord(type: type, index: index, count: count, data: data)
            try handle.write(contentsOf: encoder.encode(record))
        }

        func finish(summary: BackupSummary) throws {
            var tail = Data("\n],\n\"summary\":".utf8)
            tail.append(try JSONEncoder().encode(summary))
            tail.append(Data("\n}".utf8))
            try handle.write(contentsOf: tail)
        }

        func close() {
            try? handle.close()
        }
    }

    // MARK: - Restore Backup

    /// Restore from an encrypted backup file, re-encrypting everything under
    /// `currentKey`, a batch at a time. `password` leads to the backup's key.
    /// Run this off the main actor. An item that can't be read is left out and
    /// counted, so one bad note doesn't cost the rest of the backup.
    /// Stops with `CancellationError` if the current task is cancelled.
    @discardableResult
    static func restoreBackup(
        from url: URL,
        password: String,
        noteRepo: NoteRepository,
        versionRepo: VersionRepository,
        attachmentRepo: AttachmentRepository,
        savedSearchRepo: SavedSearchRepository,
        currentKey: SymmetricKey,
        progress: (@Sendable (Progress) -> Void)? = nil
    ) throws -> RestoreResult {
        let (index, encryption) = try readHeader(of: url)
        let namedKey = try keyNamedByHeader(encryption, password: password)
        let backupKey = namedKey ?? currentKey

        guard !index.elements.isEmpty else {
            return RestoreResult()  // Empty backup
        }

        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        func batch(at range: Range<UInt64>) throws -> BatchRecord {
            try readBatch(at: range, from: handle)
        }

        // Verify key by trying to decrypt first batch
        do {
            _ = try decryptBatchRaw(batch(at: index.elements[0]).data, key: backupKey)
        } catch {
            throw namedKey == nil ? BackupError.keyNotInBackup : BackupError.wrongPassword
        }

        var result = RestoreResult()
        for (position, range) in index.elements.enumerated() {
            try Task.checkCancellation()
            try autoreleasepool {
                let record = try batch(at: range)
                switch record.type {
                case "notes":
                    var records: [NoteRecord] = []
                    for item in try decryptItems(record.data, as: BackupNote.self, key: backupKey, result: &result) {
                        do {
                            records.append(try reEncrypted(item, from: backupKey, to: currentKey))
                        } catch {
                            Log.debug("[Backup] Skipping note \(item.id): \(error)")
                            result.skipped += 1
                        }
                    }
                    // A batch goes in whole or not at all, and restoring again
                    // after a failure picks up what's missing.
                    let written = try noteRepo.restore(records)
                    result.notes += written.restored
                    result.keptNewer += written.keptNewer

                case "attachments":
                    for item in try decryptItems(record.data, as: BackupAttachment.self, key: backupKey, result: &result) {
                        do {
                            try restoreAttachment(item, from: backupKey, to: currentKey, repo: attachmentRepo)
                        } catch {
                            Log.debug("[Backup] Skipping attachment blob \(item.id): \(error)")
                            result.skipped += 1
                        }
                    }

                case "versions":
                    for item in try decryptItems(record.data, as: BackupVersion.self, key: backupKey, result: &result) {
                        do {
                            try versionRepo.insertOrReplace(reEncrypted(item, from: backupKey, to: currentKey))
                        } catch {
                            Log.debug("[Backup] Skipping version \(item.versionKey): \(error)")
                            result.skipped += 1
                        }
                    }

                case "saved_searches":
                    for item in try decryptItems(record.data, as: BackupSavedSearch.self, key: backupKey, result: &result) {
                        do {
                            try savedSearchRepo.insertOrReplace(record: reEncrypted(item, from: backupKey, to: currentKey))
                        } catch {
                            Log.debug("[Backup] Skipping saved search \(item.id): \(error)")
                            result.skipped += 1
                        }
                    }

                default:
                    // Settings and sync details belong to the device that made
                    // the backup. Restoring merges notes into this vault.
                    break
                }
            }
            progress?(Progress(completed: position + 1, total: index.elements.count))
        }

        return result
    }

    /// The key a backup's contents are encrypted with, for a device with no key
    /// of its own to try. Proved against the first batch, so this also tells a
    /// legacy backup's wrong password from a right one, and it changes nothing.
    static func masterKey(of url: URL, password: String) throws -> Data {
        let (index, encryption) = try readHeader(of: url)
        guard let key = try keyNamedByHeader(encryption, password: password) else {
            throw BackupError.keyNotInBackup
        }
        guard key.bitCount == CryptoService.keyLength * 8 else {
            throw BackupError.invalidFormat
        }
        guard let first = index.elements.first else {
            throw BackupError.nothingToRestore
        }

        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        let record = try readBatch(at: first, from: handle)
        guard (try? decryptBatchRaw(record.data, key: key)) != nil else {
            throw BackupError.wrongPassword
        }
        return key.withUnsafeBytes { Data($0) }
    }

    /// Where the file's batches are, and how to get its key. Nothing's decrypted.
    private static func readHeader(of url: URL) throws -> (index: JSONFileScanner.Index, encryption: BackupEncryption) {
        let index: JSONFileScanner.Index
        do {
            index = try JSONFileScanner.scan(url, array: "batches")
        } catch JSONFileScanner.ScanError.malformed {
            throw BackupError.invalidFormat
        }

        let decoder = JSONDecoder()
        guard let typeJSON = index.values["type"], let versionJSON = index.values["version"],
              let encryptionJSON = index.values["encryption"],
              let type = try? decoder.decode(String.self, from: typeJSON),
              let version = try? decoder.decode(String.self, from: versionJSON),
              let encryption = try? decoder.decode(BackupEncryption.self, from: encryptionJSON),
              type == formatType else {
            throw BackupError.invalidFormat
        }
        guard version == formatVersion else {
            throw BackupError.unsupportedVersion(version)
        }
        return (index, encryption)
    }

    private static func readBatch(at range: Range<UInt64>, from handle: FileHandle) throws -> BatchRecord {
        do {
            return try JSONDecoder().decode(BatchRecord.self, from: JSONFileScanner.read(range, from: handle))
        } catch {
            throw BackupError.invalidFormat
        }
    }

    private static func reEncrypted(_ note: BackupNote, from backupKey: SymmetricKey, to currentKey: SymmetricKey) throws -> NoteRecord {
        let hashChain = try note.hashChain.map { String(decoding: try JSONEncoder().encode($0), as: UTF8.self) }
        var record = NoteRecord(
            id: note.id,
            createdAt: note.createdAt,
            modifiedAt: note.modifiedAt,
            syncedAt: note.syncedAt,
            content: try reEncrypted(note.content, from: backupKey, to: currentKey),
            tags: try reEncryptedTags(note.tags, from: backupKey, to: currentKey),
            attachments: try reEncrypted(note.attachments, from: backupKey, to: currentKey),
            pinned: note.pinned,
            archived: note.archived,
            archivedAt: note.archivedAt,
            locked: note.locked,
            lockedAt: note.lockedAt,
            deleted: note.deleted,
            deletedAt: note.deletedAt,
            version: note.version,
            wordWrap: note.wordWrap,
            syntaxLanguage: note.syntaxLanguage,
            showPreview: note.showPreview,
            color: note.color,
            contentHash: note.contentHash,
            parentHash: note.parentHash,
            hashChain: hashChain,
            needsSync: true
        )
        // Its time in the bin starts here. Without one the purge never sees it,
        // and the date in the file isn't this device's to go by.
        record.localDeletedAt = note.deleted ? Date().iso8601 : nil
        return record
    }

    private static func reEncrypted(_ version: BackupVersion, from backupKey: SymmetricKey, to currentKey: SymmetricKey) throws -> NoteVersionRecord {
        NoteVersionRecord(
            versionKey: version.versionKey,
            noteId: version.noteId,
            version: version.version,
            createdAt: version.createdAt,
            syncedAt: version.syncedAt ?? Date().iso8601,
            content: try reEncrypted(version.content, from: backupKey, to: currentKey),
            tags: try reEncryptedTags(version.tags, from: backupKey, to: currentKey),
            attachments: try reEncrypted(version.attachments, from: backupKey, to: currentKey),
            syntaxLanguage: version.syntaxLanguage,
            wordWrap: version.wordWrap,
            showPreview: version.showPreview,
            color: version.color,
            reason: version.reason ?? "backup-restore",
            contentHash: version.contentHash,
            parentHash: version.parentHash
        )
    }

    private static func reEncrypted(
        _ search: BackupSavedSearch, from backupKey: SymmetricKey, to currentKey: SymmetricKey
    ) throws -> SavedSearchRepository.SavedSearchRecord {
        SavedSearchRepository.SavedSearchRecord(
            id: search.id,
            name: try reEncrypted(search.name, from: backupKey, to: currentKey),
            query: try reEncrypted(search.query, from: backupKey, to: currentKey),
            displayOrder: search.order,
            createdAt: search.createdAt,
            modifiedAt: search.modifiedAt,
            syncedAt: search.syncedAt,
            deleted: search.deleted,
            deletedAt: search.deletedAt,
            version: search.version,
            needsSync: true
        )
    }

    /// Text encrypted with `backupKey`, encrypted with `currentKey` instead.
    private static func reEncrypted(_ stored: String, from backupKey: SymmetricKey, to currentKey: SymmetricKey) throws -> String {
        let plaintext = try CryptoService.decryptText(CryptoService.parseEncryptedJSON(stored), key: backupKey)
        return try CryptoService.serializeEncryptedJSON(CryptoService.encryptText(plaintext, key: currentKey))
    }

    /// Note tags as a backup holds them, in this client's storage form. The web
    /// keeps one blob with every tag in it, and older notes one entry per tag.
    private static func reEncryptedTags(_ stored: [String], from backupKey: SymmetricKey, to currentKey: SymmetricKey) throws -> String {
        var tags: [String] = []
        for entry in stored {
            let text = try CryptoService.decryptText(CryptoService.parseEncryptedJSON(entry), key: backupKey)
            tags.append(contentsOf: parsedTags(text))
        }
        return try CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(tags, key: currentKey))
    }

    private static func parsedTags(_ text: String) -> [String] {
        guard !text.isEmpty else { return [] }
        let json = Data(text.utf8)
        if let tags = try? JSONDecoder().decode([String].self, from: json) { return tags }
        if let tag = try? JSONDecoder().decode(String.self, from: json) { return [tag] }
        return text.split(separator: ",")
            .map { String($0).trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    /// Refs as JSON for the database, with their filenames moved to `currentKey`.
    private static func reEncrypted(_ refs: [AttachmentRef], from backupKey: SymmetricKey, to currentKey: SymmetricKey) throws -> String {
        var moved = refs
        for index in moved.indices {
            let name = AttachmentFilename.display(stored: refs[index].filename, key: backupKey)
            moved[index].filename = try AttachmentFilename.encrypted(name, key: currentKey)
        }
        return String(decoding: try JSONEncoder().encode(moved), as: UTF8.self)
    }

    private static func restoreAttachment(
        _ attachment: BackupAttachment,
        from backupKey: SymmetricKey,
        to currentKey: SymmetricKey,
        repo: AttachmentRepository
    ) throws {
        // Ids are never reused for different contents, so nothing would change,
        // and the server would be sent a blob it already has.
        guard try !repo.hasBlob(id: attachment.id) else { return }
        guard let blob = Data(base64Encoded: attachment.blob) else { throw BackupError.invalidFormat }
        let encrypted = try CryptoService.parseEncryptedJSON(String(decoding: blob, as: UTF8.self))
        let plaintext = try CryptoService.decrypt(encrypted, key: backupKey)
        let reEncrypted = try CryptoService.serializeEncryptedJSON(CryptoService.encrypt(plaintext, key: currentKey))
        try repo.storeBlob(
            id: attachment.id, mimeType: "",
            size: plaintext.count, data: Data(reEncrypted.utf8)
        )
    }

    /// The key the header leads to from `password`, or nil if it leads nowhere.
    ///
    /// Backups of envelope vaults made before the header carried the wrapped key
    /// have an empty salt, and only open with the vault's own master key.
    /// The most PBKDF2 rounds a backup may ask for: seconds of work, not hours.
    /// Every client writes far fewer.
    static let maxBackupIterations = 10_000_000

    private static func keyNamedByHeader(_ encryption: BackupEncryption, password: String) throws -> SymmetricKey? {
        if encryption.envelopeVersion != nil {
            guard let deviceSalt = encryption.deviceSalt.flatMap({ Data(base64Encoded: $0) }),
                  let wrappedJSON = encryption.localWrappedMaster else {
                throw BackupError.invalidSalt
            }
            let deviceKey = try CryptoService.deriveKey(
                password: password,
                salt: deviceSalt,
                iterations: CryptoService.defaultIterations
            )
            let wrapped = try CryptoService.parseEncryptedJSON(wrappedJSON)
            guard let masterKey = try? CryptoService.unwrapMasterKey(wrapped, with: deviceKey) else {
                throw BackupError.wrongPassword
            }
            return SymmetricKey(data: masterKey)
        }

        guard let salt = encryption.salt, !salt.isEmpty,
              let iterations = encryption.iterations, iterations > 0 else {
            return nil
        }
        // The file chooses this, before any password is tried. A value above
        // UInt32's range would crash, and one just below would run for hours.
        // A low one needs no check: `deriveKey` raises it to the minimum, as it
        // always has, so an old backup that names one still opens.
        guard iterations <= maxBackupIterations, let rounds = UInt32(exactly: iterations) else {
            throw BackupError.invalidFormat
        }
        guard let saltData = Data(base64Encoded: salt) else {
            throw BackupError.invalidSalt
        }
        return try CryptoService.deriveKey(password: password, salt: saltData, iterations: rounds)
    }

    // MARK: - JWE Compact Encryption

    /// Encrypt an array of Codable items: JSON → zlib → AES-256-GCM → JWE compact.
    private static func encryptBatch<T: Encodable>(_ items: [T], key: SymmetricKey) throws -> String {
        let jsonData = try JSONEncoder().encode(items)
        let compressed = try compress(jsonData)
        return try jweEncrypt(compressed, key: key)
    }

    /// Each item of a batch, leaving out and counting any that can't be read.
    private static func decryptItems<T: Decodable>(
        _ jwe: String,
        as type: T.Type,
        key: SymmetricKey,
        result: inout RestoreResult
    ) throws -> [T] {
        let items = try JSONDecoder().decode([Lossy<T>].self, from: decryptBatchRaw(jwe, key: key))
        result.skipped += items.count { $0.value == nil }
        return items.compactMap(\.value)
    }

    /// An item of a batch, or nil if it didn't decode.
    private struct Lossy<T: Decodable>: Decodable {
        let value: T?

        init(from decoder: Decoder) throws {
            value = try? decoder.singleValueContainer().decode(T.self)
        }
    }

    /// Decrypt a JWE compact string to raw data (for key verification).
    private static func decryptBatchRaw(_ jwe: String, key: SymmetricKey) throws -> Data {
        let compressed = try jweDecrypt(jwe, key: key)
        return try decompress(compressed)
    }

    /// Create JWE compact serialisation: header.encryptedKey.iv.ciphertext.tag
    /// Using "dir" algorithm (direct key), A256GCM encryption. RFC 7516 makes
    /// the ASCII of the encoded header the authenticated data.
    private static func jweEncrypt(_ data: Data, key: SymmetricKey) throws -> String {
        // Header: {"alg":"dir","enc":"A256GCM"}
        let header = #"{"alg":"dir","enc":"A256GCM"}"#
        let headerB64 = base64urlEncode(Data(header.utf8))

        // Encrypt with AES-256-GCM
        let nonce = AES.GCM.Nonce()
        let sealed = try AES.GCM.seal(data, using: key, nonce: nonce, authenticating: Data(headerB64.utf8))

        let ivB64 = base64urlEncode(Data(nonce))
        let ciphertextB64 = base64urlEncode(sealed.ciphertext)
        let tagB64 = base64urlEncode(sealed.tag)

        // JWE compact: header.encryptedKey.iv.ciphertext.tag
        // "dir" means no encrypted key segment (empty)
        return "\(headerB64)..\(ivB64).\(ciphertextB64).\(tagB64)"
    }

    /// Parse and decrypt JWE compact serialisation.
    private static func jweDecrypt(_ jwe: String, key: SymmetricKey) throws -> Data {
        let parts = jwe.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 5 else {
            throw BackupError.invalidJWE
        }

        // parts[0] = header (we know it's dir/A256GCM)
        // parts[1] = encrypted key (empty for dir)
        // parts[2] = iv
        // parts[3] = ciphertext
        // parts[4] = tag

        guard let ivData = base64urlDecode(String(parts[2])),
              let ciphertext = base64urlDecode(String(parts[3])),
              let tag = base64urlDecode(String(parts[4])) else {
            throw BackupError.invalidJWE
        }

        let nonce = try AES.GCM.Nonce(data: ivData)
        let sealedBox = try AES.GCM.SealedBox(nonce: nonce, ciphertext: ciphertext, tag: tag)
        if let opened = try? AES.GCM.open(sealedBox, using: key, authenticating: Data(parts[0].utf8)) {
            return opened
        }
        // iOS backups made before this format was adopted have no authenticated data.
        return try AES.GCM.open(sealedBox, using: key)
    }

    // MARK: - Compression (zlib)

    /// RFC 1950, as `pako.deflate` writes: a two-byte header, raw DEFLATE
    /// (what Apple's `COMPRESSION_ZLIB` produces), and an Adler-32 checksum.
    private static func compress(_ data: Data) throws -> Data {
        let sourceSize = data.count
        // Allocate buffer (compression may expand worst case, but DEFLATE usually shrinks)
        let destinationBufferSize = sourceSize + sourceSize / 8 + 512
        var destinationBuffer = Data(count: destinationBufferSize)

        let compressedSize = data.withUnsafeBytes { sourcePtr in
            destinationBuffer.withUnsafeMutableBytes { destPtr in
                compression_encode_buffer(
                    destPtr.bindMemory(to: UInt8.self).baseAddress!,
                    destinationBufferSize,
                    sourcePtr.bindMemory(to: UInt8.self).baseAddress!,
                    sourceSize,
                    nil,
                    COMPRESSION_ZLIB
                )
            }
        }
        guard compressedSize > 0 else {
            throw BackupError.compressionFailed
        }

        var result = Data([0x78, 0x9C])
        result.append(destinationBuffer.prefix(compressedSize))
        var checksum = adler32(data).bigEndian
        withUnsafeBytes(of: &checksum) { result.append(contentsOf: $0) }
        return result
    }

    private static func decompress(_ data: Data) throws -> Data {
        // Raw DEFLATE can happen to start like a zlib header, so a stream only
        // counts as zlib if its checksum agrees.
        if data.count > 6, let first = data.first, let second = data.dropFirst().first,
           first & 0x0F == 8, (UInt16(first) << 8 | UInt16(second)) % 31 == 0,
           let inflated = try? inflate(data.dropFirst(2).dropLast(4)) {
            let expected = data.suffix(4).reduce(UInt32(0)) { $0 << 8 | UInt32($1) }
            if adler32(inflated) == expected {
                return inflated
            }
        }
        return try inflate(data)
    }

    /// Inflate raw DEFLATE.
    private static func inflate(_ data: Data) throws -> Data {
        // Start with 4x the compressed size and grow if needed
        var bufferSize = max(data.count * 4, 1024)
        var result = Data(count: bufferSize)

        while true {
            let decompressedSize = data.withUnsafeBytes { sourcePtr in
                result.withUnsafeMutableBytes { destPtr in
                    compression_decode_buffer(
                        destPtr.bindMemory(to: UInt8.self).baseAddress!,
                        bufferSize,
                        sourcePtr.bindMemory(to: UInt8.self).baseAddress!,
                        data.count,
                        nil,
                        COMPRESSION_ZLIB
                    )
                }
            }

            if decompressedSize == 0 {
                throw BackupError.decompressionFailed
            }
            if decompressedSize < bufferSize {
                return result.prefix(decompressedSize)
            }
            // Buffer was too small — double and retry
            bufferSize *= 2
            result = Data(count: bufferSize)
        }
    }

    private static func adler32(_ data: Data) -> UInt32 {
        var a: UInt32 = 1
        var b: UInt32 = 0
        data.withUnsafeBytes { (bytes: UnsafeRawBufferPointer) in
            // 5552 is the most bytes that can't overflow before the reduction.
            var position = 0
            while position < bytes.count {
                let end = min(position + 5552, bytes.count)
                while position < end {
                    a &+= UInt32(bytes[position])
                    b &+= a
                    position += 1
                }
                a %= 65521
                b %= 65521
            }
        }
        return b << 16 | a
    }

    // MARK: - Base64url

    private static func base64urlEncode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    private static func base64urlDecode(_ string: String) -> Data? {
        var base64 = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        // Add padding
        let remainder = base64.count % 4
        if remainder > 0 {
            base64 += String(repeating: "=", count: 4 - remainder)
        }
        return Data(base64Encoded: base64)
    }

    // MARK: - Filename

    static func defaultFilename() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        // The web client only opens .jottery-backup and .json files.
        return "jottery-backup-\(formatter.string(from: Date())).jottery-backup"
    }
}

private func decodedRefs(_ json: String) -> [AttachmentRef] {
    (try? JSONDecoder().decode([AttachmentRef].self, from: Data(json.utf8))) ?? []
}

private extension KeyedDecodingContainer {

    /// Web backups hold an array here. Old iOS backups hold it as JSON in a string.
    func decodeArrayOrJSONString<T: Decodable>(_ type: T.Type, forKey key: Key) throws -> [T]? {
        if let array = try? decodeIfPresent([T].self, forKey: key) {
            return array
        }
        guard let json = try decodeIfPresent(String.self, forKey: key) else {
            return nil
        }
        return try JSONDecoder().decode([T].self, from: Data(json.utf8))
    }

    /// An array of encrypted entries, or an old iOS backup's single encrypted blob.
    func decodeTags(forKey key: Key) throws -> [String] {
        if let tags = try? decodeIfPresent([String].self, forKey: key) {
            return tags
        }
        return try decodeIfPresent(String.self, forKey: key).map { [$0] } ?? []
    }
}

// MARK: - Errors

enum BackupError: LocalizedError, Equatable {
    case noVault
    case invalidFormat
    case unsupportedVersion(String)
    case invalidSalt
    case wrongPassword
    case keyNotInBackup
    /// It holds no notes, so a vault made from it would be empty.
    case nothingToRestore
    case invalidJWE
    case compressionFailed
    case decompressionFailed

    var errorDescription: String? {
        switch self {
        case .noVault: return L.backupErrorNoVault
        case .invalidFormat: return L.backupErrorInvalidFormat
        case .unsupportedVersion(let v): return L.backupErrorUnsupportedVersion(v)
        case .invalidSalt: return L.backupErrorInvalidSalt
        case .wrongPassword: return L.backupErrorWrongPassword
        case .keyNotInBackup: return L.backupErrorKeyNotInBackup
        case .nothingToRestore: return L.backupErrorNothingToRestore
        case .invalidJWE: return L.backupErrorInvalidData
        case .compressionFailed: return L.backupErrorCompressionFailed
        case .decompressionFailed: return L.backupErrorDecompressionFailed
        }
    }
}
