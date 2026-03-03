import Compression
import CryptoKit
import Foundation
import GRDB

/// Encrypted backup service — creates and restores JWE v3 backup files
/// compatible with the web client's `backupService.ts`.
///
/// Format:
/// ```json
/// {
///   "version": "3.0",
///   "type": "jottery-encrypted-backup",
///   "createdAt": "ISO 8601",
///   "encryption": { "salt": "base64", "iterations": 600000, "algorithm": "PBKDF2" },
///   "batches": [ { "type": "notes", "index": 0, "count": N, "data": "JWE compact" } ],
///   "summary": { "notes": N, "attachments": N, "versions": N, "savedSearches": N }
/// }
/// ```
///
/// Each batch is: JSON array → DEFLATE compress → AES-256-GCM encrypt → JWE compact.
enum BackupService {

    // MARK: - Constants

    static let noteBatchSize = 250
    static let attachmentBatchSize = 50
    static let versionBatchSize = 250

    // MARK: - Backup Data Structures

    struct BackupFile: Codable {
        let version: String
        let type: String
        let createdAt: String
        let encryption: BackupEncryption
        var batches: [BatchRecord]
        let summary: BackupSummary
    }

    struct BackupEncryption: Codable {
        let salt: String
        let iterations: Int
        let algorithm: String
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
        let tags: String
        let attachments: String
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
        let hashChain: String?

        init(from record: NoteRecord) {
            self.id = record.id
            self.createdAt = record.createdAt
            self.modifiedAt = record.modifiedAt
            self.syncedAt = record.syncedAt
            self.content = record.content
            self.tags = record.tags
            self.attachments = record.attachments
            self.pinned = record.pinned
            self.archived = record.archived
            self.archivedAt = record.archivedAt
            self.locked = record.locked
            self.lockedAt = record.lockedAt
            self.deleted = record.deleted
            self.deletedAt = record.deletedAt
            self.version = record.version
            self.wordWrap = record.wordWrap
            self.syntaxLanguage = record.syntaxLanguage
            self.showPreview = record.showPreview
            self.color = record.color
            self.contentHash = record.contentHash
            self.parentHash = record.parentHash
            self.hashChain = record.hashChain
        }

        func toNoteRecord() -> NoteRecord {
            NoteRecord(
                id: id,
                createdAt: createdAt,
                modifiedAt: modifiedAt,
                syncedAt: syncedAt,
                content: content,
                tags: tags,
                attachments: attachments,
                pinned: pinned,
                archived: archived,
                archivedAt: archivedAt,
                locked: locked,
                lockedAt: lockedAt,
                deleted: deleted,
                deletedAt: deletedAt,
                version: version,
                wordWrap: wordWrap,
                syntaxLanguage: syntaxLanguage,
                showPreview: showPreview,
                color: color,
                contentHash: contentHash,
                parentHash: parentHash,
                hashChain: hashChain,
                needsSync: true
            )
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
        let tags: String
        let attachments: String
        let syntaxLanguage: String?
        let wordWrap: Bool?
        let showPreview: Bool?
        let color: String?
        let reason: String?
        let contentHash: String?
        let parentHash: String?

        init(from record: NoteVersionRecord) {
            self.versionKey = record.versionKey
            self.noteId = record.noteId
            self.version = record.version
            self.createdAt = record.createdAt
            self.syncedAt = record.syncedAt
            self.content = record.content
            self.tags = record.tags
            self.attachments = record.attachments
            self.syntaxLanguage = record.syntaxLanguage
            self.wordWrap = record.wordWrap
            self.showPreview = record.showPreview
            self.color = record.color
            self.reason = record.reason
            self.contentHash = record.contentHash
            self.parentHash = record.parentHash
        }

        func toVersionRecord() -> NoteVersionRecord {
            NoteVersionRecord(
                versionKey: versionKey,
                noteId: noteId,
                version: version,
                createdAt: createdAt,
                syncedAt: syncedAt ?? Date().iso8601,
                content: content,
                tags: tags,
                attachments: attachments,
                syntaxLanguage: syntaxLanguage,
                wordWrap: wordWrap,
                showPreview: showPreview,
                color: color,
                reason: reason ?? "backup-restore",
                contentHash: contentHash,
                parentHash: parentHash
            )
        }
    }

    // MARK: - Create Backup

    /// Create an encrypted backup of all data.
    static func createBackup(
        noteRepo: NoteRepository,
        versionRepo: VersionRepository,
        attachmentRepo: AttachmentRepository,
        savedSearchRepo: SavedSearchRepository,
        encryptionRepo: EncryptionRepository,
        key: SymmetricKey
    ) throws -> Data {
        guard let metadata = try encryptionRepo.get() else {
            throw BackupError.noVault
        }

        // Gather all note records (raw encrypted, including deleted)
        let allRecords = try noteRepo.db.dbPool.read { db in
            try NoteRecord.fetchAll(db)
        }

        // Gather attachment blobs
        let blobIds = try attachmentRepo.listBlobIds()

        // Gather version snapshots for all notes
        var allVersions: [NoteVersionRecord] = []
        for record in allRecords {
            let versions = try versionRepo.getVersions(noteId: record.id)
            allVersions.append(contentsOf: versions)
        }

        // Gather saved searches (raw encrypted)
        let savedSearchRecords = try savedSearchRepo.db.dbPool.read { db in
            try SavedSearchRepository.SavedSearchRecord
                .filter(Column("deleted") == false)
                .fetchAll(db)
        }
        let _ = savedSearchRecords  // Saved searches included in summary

        // Build batches
        var batches: [BatchRecord] = []

        // Note batches
        let noteItems = allRecords.map { BackupNote(from: $0) }
        let noteBatches = stride(from: 0, to: noteItems.count, by: noteBatchSize).map {
            Array(noteItems[$0..<min($0 + noteBatchSize, noteItems.count)])
        }
        for (index, batch) in noteBatches.enumerated() {
            let jwe = try encryptBatch(batch, key: key)
            batches.append(BatchRecord(type: "notes", index: index, count: batch.count, data: jwe))
        }

        // Attachment batches
        var attachmentItems: [BackupAttachment] = []
        for blobId in blobIds {
            if let blobData = try attachmentRepo.getBlob(id: blobId) {
                attachmentItems.append(BackupAttachment(
                    id: blobId,
                    blob: blobData.base64EncodedString()
                ))
            }
        }
        let attachmentBatches = stride(from: 0, to: attachmentItems.count, by: attachmentBatchSize).map {
            Array(attachmentItems[$0..<min($0 + attachmentBatchSize, attachmentItems.count)])
        }
        for (index, batch) in attachmentBatches.enumerated() {
            let jwe = try encryptBatch(batch, key: key)
            batches.append(BatchRecord(type: "attachments", index: index, count: batch.count, data: jwe))
        }

        // Version batches
        let versionItems = allVersions.map { BackupVersion(from: $0) }
        let versionBatches = stride(from: 0, to: versionItems.count, by: versionBatchSize).map {
            Array(versionItems[$0..<min($0 + versionBatchSize, versionItems.count)])
        }
        for (index, batch) in versionBatches.enumerated() {
            let jwe = try encryptBatch(batch, key: key)
            batches.append(BatchRecord(type: "versions", index: index, count: batch.count, data: jwe))
        }

        let backupFile = BackupFile(
            version: "3.0",
            type: "jottery-encrypted-backup",
            createdAt: Date().iso8601,
            encryption: BackupEncryption(
                salt: metadata.salt,
                iterations: metadata.iterations,
                algorithm: "PBKDF2"
            ),
            batches: batches,
            summary: BackupSummary(
                notes: allRecords.count,
                attachments: attachmentItems.count,
                versions: allVersions.count,
                savedSearches: savedSearchRecords.count
            )
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try encoder.encode(backupFile)
    }

    // MARK: - Restore Backup

    /// Restore from an encrypted backup file. The password is used to derive the key.
    /// Returns the number of notes restored.
    @discardableResult
    static func restoreBackup(
        data: Data,
        password: String,
        noteRepo: NoteRepository,
        versionRepo: VersionRepository,
        attachmentRepo: AttachmentRepository,
        currentKey: SymmetricKey
    ) throws -> Int {
        let backup = try JSONDecoder().decode(BackupFile.self, from: data)

        guard backup.type == "jottery-encrypted-backup" else {
            throw BackupError.invalidFormat
        }
        guard backup.version == "3.0" else {
            throw BackupError.unsupportedVersion(backup.version)
        }

        // Derive key from backup's salt and password
        guard let saltData = Data(base64Encoded: backup.encryption.salt) else {
            throw BackupError.invalidSalt
        }
        let backupKey = CryptoService.deriveKey(
            password: password,
            salt: saltData,
            iterations: UInt32(backup.encryption.iterations)
        )

        // Verify key by trying to decrypt first batch
        guard let firstBatch = backup.batches.first else {
            return 0  // Empty backup
        }
        do {
            let _ = try decryptBatchRaw(firstBatch.data, key: backupKey)
        } catch {
            throw BackupError.wrongPassword
        }

        var notesRestored = 0

        // Process batches
        for batch in backup.batches {
            switch batch.type {
            case "notes":
                let notes: [BackupNote] = try decryptBatch(batch.data, key: backupKey)
                for backupNote in notes {
                    var record = backupNote.toNoteRecord()
                    // Re-encrypt content and tags with the current vault key
                    // (backup stores them encrypted with the backup key)
                    let encContent = try CryptoService.parseEncryptedJSON(record.content)
                    let plainContent = try CryptoService.decryptText(encContent, key: backupKey)
                    let reEncContent = try CryptoService.encryptText(plainContent, key: currentKey)
                    record.content = try CryptoService.serializeEncryptedJSON(reEncContent)

                    let encTags = try CryptoService.parseEncryptedJSON(record.tags)
                    let plainTagsStr = try CryptoService.decryptText(encTags, key: backupKey)
                    let reEncTags = try CryptoService.encryptText(plainTagsStr, key: currentKey)
                    record.tags = try CryptoService.serializeEncryptedJSON(reEncTags)

                    // Re-encrypt attachment filenames
                    if let data = record.attachments.data(using: .utf8),
                       var refs = try? JSONDecoder().decode([AttachmentRef].self, from: data) {
                        for i in refs.indices {
                            if let enc = try? CryptoService.parseEncryptedJSON(refs[i].filename),
                               let plain = try? CryptoService.decryptText(enc, key: backupKey) {
                                let reEnc = try CryptoService.encryptText(plain, key: currentKey)
                                refs[i].filename = try CryptoService.serializeEncryptedJSON(reEnc)
                            }
                        }
                        let refsJSON = try JSONEncoder().encode(refs)
                        record.attachments = String(data: refsJSON, encoding: .utf8) ?? "[]"
                    }

                    record.needsSync = true
                    try noteRepo.insertOrReplace(record)
                    notesRestored += 1
                }

            case "attachments":
                let attachments: [BackupAttachment] = try decryptBatch(batch.data, key: backupKey)
                for att in attachments {
                    guard let blobData = Data(base64Encoded: att.blob) else { continue }
                    do {
                        // Re-encrypt blob data with current key
                        let blobStr = String(data: blobData, encoding: .utf8) ?? ""
                        if !blobStr.isEmpty,
                           let enc = try? CryptoService.parseEncryptedJSON(blobStr) {
                            let plain = try CryptoService.decrypt(enc, key: backupKey)
                            let reEnc = try CryptoService.encrypt(plain, key: currentKey)
                            let reEncJSON = try CryptoService.serializeEncryptedJSON(reEnc)
                            try attachmentRepo.storeBlob(
                                id: att.id, filename: "", mimeType: "",
                                size: plain.count, data: Data(reEncJSON.utf8)
                            )
                        } else {
                            // Store as-is if not encrypted (shouldn't happen but be safe)
                            try attachmentRepo.storeBlob(
                                id: att.id, filename: "", mimeType: "",
                                size: blobData.count, data: blobData
                            )
                        }
                    } catch {
                        print("[Backup] Skipping attachment blob \(att.id): \(error)")
                    }
                }

            case "versions":
                let versions: [BackupVersion] = try decryptBatch(batch.data, key: backupKey)
                for ver in versions {
                    var record = ver.toVersionRecord()
                    // Re-encrypt content and tags
                    do {
                        let encC = try CryptoService.parseEncryptedJSON(record.content)
                        let plainC = try CryptoService.decryptText(encC, key: backupKey)
                        let reEncC = try CryptoService.encryptText(plainC, key: currentKey)
                        record.content = try CryptoService.serializeEncryptedJSON(reEncC)

                        let encT = try CryptoService.parseEncryptedJSON(record.tags)
                        let plainT = try CryptoService.decryptText(encT, key: backupKey)
                        let reEncT = try CryptoService.encryptText(plainT, key: currentKey)
                        record.tags = try CryptoService.serializeEncryptedJSON(reEncT)
                    } catch {
                        // Skip versions that fail to re-encrypt
                        continue
                    }
                    try versionRepo.insertOrReplace(record)
                }

            default:
                break
            }
        }

        return notesRestored
    }

    // MARK: - JWE Compact Encryption

    /// Encrypt an array of Codable items: JSON → DEFLATE → AES-256-GCM → JWE compact.
    private static func encryptBatch<T: Encodable>(_ items: [T], key: SymmetricKey) throws -> String {
        let jsonData = try JSONEncoder().encode(items)
        let compressed = try compress(jsonData)
        return try jweEncrypt(compressed, key: key)
    }

    /// Decrypt a JWE compact string to an array of Codable items.
    private static func decryptBatch<T: Decodable>(_ jwe: String, key: SymmetricKey) throws -> [T] {
        let compressed = try jweDecrypt(jwe, key: key)
        let jsonData = try decompress(compressed)
        return try JSONDecoder().decode([T].self, from: jsonData)
    }

    /// Decrypt a JWE compact string to raw data (for key verification).
    private static func decryptBatchRaw(_ jwe: String, key: SymmetricKey) throws -> Data {
        let compressed = try jweDecrypt(jwe, key: key)
        return try decompress(compressed)
    }

    /// Create JWE compact serialisation: header.encryptedKey.iv.ciphertext.tag
    /// Using "dir" algorithm (direct key), A256GCM encryption.
    private static func jweEncrypt(_ data: Data, key: SymmetricKey) throws -> String {
        // Header: {"alg":"dir","enc":"A256GCM"}
        let header = #"{"alg":"dir","enc":"A256GCM"}"#
        let headerB64 = base64urlEncode(Data(header.utf8))

        // Encrypt with AES-256-GCM
        let nonce = AES.GCM.Nonce()
        let sealed = try AES.GCM.seal(data, using: key, nonce: nonce)

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

        // parts[0] = header (ignored — we know it's dir/A256GCM)
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
        return try AES.GCM.open(sealedBox, using: key)
    }

    // MARK: - Compression (DEFLATE)

    private static func compress(_ data: Data) throws -> Data {
        let sourceSize = data.count
        // Allocate buffer (compression may expand worst case, but DEFLATE usually shrinks)
        let destinationBufferSize = sourceSize + 512
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
        return destinationBuffer.prefix(compressedSize)
    }

    private static func decompress(_ data: Data) throws -> Data {
        // Start with 4x the compressed size and grow if needed
        var bufferSize = data.count * 4
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
        return "jottery-backup-\(formatter.string(from: Date())).jottery"
    }
}

// MARK: - Errors

enum BackupError: LocalizedError {
    case noVault
    case invalidFormat
    case unsupportedVersion(String)
    case invalidSalt
    case wrongPassword
    case invalidJWE
    case compressionFailed
    case decompressionFailed

    var errorDescription: String? {
        switch self {
        case .noVault: return "No vault found"
        case .invalidFormat: return "Invalid backup file format"
        case .unsupportedVersion(let v): return "Unsupported backup version: \(v)"
        case .invalidSalt: return "Invalid encryption salt in backup"
        case .wrongPassword: return "Incorrect password for this backup"
        case .invalidJWE: return "Invalid encrypted data in backup"
        case .compressionFailed: return "Failed to compress backup data"
        case .decompressionFailed: return "Failed to decompress backup data"
        }
    }
}
