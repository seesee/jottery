import CryptoKit
import Foundation

/// Import notes from JSON matching the web export format.
enum ImportService {

    enum Strategy: String, CaseIterable, Identifiable {
        case skip     // Skip notes that already exist (match by ID)
        case replace  // Overwrite existing notes with imported data
        case merge    // Import all notes with new IDs

        var id: String { rawValue }

        var displayName: String {
            switch self {
            case .skip:    return L.importStrategySkip
            case .replace: return L.importStrategyReplace
            case .merge:   return L.importStrategyMerge
            }
        }

        var description: String {
            switch self {
            case .skip:    return L.importStrategySkipDescription
            case .replace: return L.importStrategyReplaceDescription
            case .merge:   return L.importStrategyMergeDescription
            }
        }
    }

    struct ImportResult {
        var imported: Int = 0
        var skipped: Int = 0
        var errors: [String] = []
    }

    /// Parse a JSON file into ExportData.
    static func parse(_ data: Data) throws -> ExportService.ExportData {
        let decoder = JSONDecoder()
        let export = try decoder.decode(ExportService.ExportData.self, from: data)
        guard export.version == "1.0" else {
            throw ImportError.unsupportedVersion(export.version)
        }
        return export
    }

    /// An export file on disk, indexed so its notes can be read one at a time:
    /// with their attachments they can be far larger than memory.
    struct ImportFile: Sendable {
        let url: URL
        let exportDate: String
        fileprivate let notes: [Range<UInt64>]

        var noteCount: Int { notes.count }
    }

    /// Check a file is an export this version can read, and index it.
    /// Run this off the main actor: it reads the whole file.
    static func open(_ url: URL) throws -> ImportFile {
        let index: JSONFileScanner.Index
        do {
            index = try JSONFileScanner.scan(url, array: "notes")
        } catch JSONFileScanner.ScanError.malformed {
            throw ImportError.invalidFormat
        }
        let decoder = JSONDecoder()
        guard let versionJSON = index.values["version"], let dateJSON = index.values["exportDate"],
              let version = try? decoder.decode(String.self, from: versionJSON),
              let exportDate = try? decoder.decode(String.self, from: dateJSON) else {
            throw ImportError.invalidFormat
        }
        guard version == "1.0" else {
            throw ImportError.unsupportedVersion(version)
        }
        return ImportFile(url: url, exportDate: exportDate, notes: index.elements)
    }

    /// Import the notes of an export file, reading one at a time. Run this off
    /// the main actor. Stops with `CancellationError` if the current task is
    /// cancelled.
    static func importNotes(
        from file: ImportFile,
        strategy: Strategy,
        noteRepo: NoteRepository,
        attachmentRepo: AttachmentRepository,
        key: SymmetricKey,
        progress: (@Sendable (Int, Int) -> Void)? = nil
    ) throws -> ImportResult {
        let handle = try FileHandle(forReadingFrom: file.url)
        defer { try? handle.close() }

        var result = ImportResult()
        for (index, range) in file.notes.enumerated() {
            try Task.checkCancellation()
            autoreleasepool {
                do {
                    let json = try JSONFileScanner.read(range, from: handle)
                    let note = try JSONDecoder().decode(ExportService.ExportNote.self, from: json)
                    importNote(note, strategy: strategy, noteRepo: noteRepo, attachmentRepo: attachmentRepo, key: key, into: &result)
                } catch {
                    result.errors.append(error.localizedDescription)
                }
            }
            progress?(index + 1, file.notes.count)
        }
        return result
    }

    /// Import notes from parsed export data.
    static func importNotes(
        _ export: ExportService.ExportData,
        strategy: Strategy,
        noteRepo: NoteRepository,
        attachmentRepo: AttachmentRepository,
        key: SymmetricKey
    ) throws -> ImportResult {
        var result = ImportResult()
        for exportNote in export.notes {
            importNote(exportNote, strategy: strategy, noteRepo: noteRepo, attachmentRepo: attachmentRepo, key: key, into: &result)
        }
        return result
    }

    private static func importNote(
        _ exportNote: ExportService.ExportNote,
        strategy: Strategy,
        noteRepo: NoteRepository,
        attachmentRepo: AttachmentRepository,
        key: SymmetricKey,
        into result: inout ImportResult
    ) {
        do {
            let existing = try noteRepo.getRaw(id: exportNote.id)

            if existing != nil {
                switch strategy {
                case .skip:
                    result.skipped += 1
                    return
                case .replace:
                    try noteRepo.hardDelete(id: exportNote.id)
                case .merge:
                    // Will create with new ID below
                    break
                }
            }

            // Create the note
            let useNewId = (strategy == .merge && existing != nil)
            let noteId = useNewId ? UUID().uuidString : exportNote.id

            // Encrypt content and tags
            let encContent = try CryptoService.encryptText(exportNote.content, key: key)
            let encTags = try CryptoService.encryptStringArray(exportNote.tags, key: key)
            let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
            let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)

            // Build attachment refs (re-encrypt blobs)
            var attachmentRefs: [AttachmentRef] = []
            for exportAtt in exportNote.attachments {
                let attId = UUID().uuidString
                if let rawData = Data(base64Encoded: exportAtt.data) {
                    let filename = AttachmentFilename.safe(exportAtt.filename)
                    let encData = try CryptoService.encrypt(rawData, key: key)
                    let encJSON = try CryptoService.serializeEncryptedJSON(encData)
                    let blobData = Data(encJSON.utf8)
                    try attachmentRepo.storeBlob(
                        id: attId,
                        filename: filename,
                        mimeType: exportAtt.mimeType,
                        size: rawData.count,
                        data: blobData
                    )
                    let encFilenameJSON = try AttachmentFilename.encrypted(filename, key: key)
                    attachmentRefs.append(AttachmentRef(
                        id: attId,
                        filename: encFilenameJSON,
                        mimeType: exportAtt.mimeType,
                        size: rawData.count,
                        data: attId
                    ))
                }
            }

            let attachmentsStr = String(
                data: try JSONEncoder().encode(attachmentRefs),
                encoding: .utf8
            ) ?? "[]"

            let now = Date().iso8601
            var record = NoteRecord(
                id: noteId,
                createdAt: exportNote.createdAt,
                modifiedAt: exportNote.modifiedAt,
                syncedAt: nil,
                content: contentJSON,
                tags: tagsJSON,
                attachments: attachmentsStr,
                pinned: exportNote.pinned,
                archived: exportNote.archived,
                archivedAt: exportNote.archived ? now : nil,
                locked: exportNote.locked,
                lockedAt: exportNote.locked ? now : nil,
                deleted: false,
                deletedAt: nil,
                version: 0,
                wordWrap: exportNote.wordWrap ?? true,
                syntaxLanguage: exportNote.syntaxLanguage ?? "markdown",
                showPreview: exportNote.showPreview ?? false,
                color: exportNote.color,
                contentHash: nil,
                parentHash: nil,
                hashChain: nil,
                needsSync: true
            )

            // Compute content hash
            let syncTags = try exportNote.tags.map { tag -> String in
                try CryptoService.encryptTagForSync(tag, key: key)
            }
            let contentHash = HashChainService.computeContentHash(
                encryptedContent: contentJSON,
                encryptedTags: syncTags,
                attachmentIds: attachmentRefs.map(\.id).sorted(),
                pinned: record.pinned,
                archived: record.archived,
                locked: record.locked,
                syntaxLanguage: record.syntaxLanguage,
                wordWrap: record.wordWrap,
                showPreview: record.showPreview,
                color: record.color
            )
            record.contentHash = contentHash
            let hashChain = HashChainService.updateHashChain(currentHash: contentHash, parentChain: [])
            record.hashChain = String(data: try JSONEncoder().encode(hashChain), encoding: .utf8)

            try noteRepo.insertOrReplace(record)
            result.imported += 1
        } catch {
            result.errors.append("\(exportNote.id): \(error.localizedDescription)")
        }
    }

    enum ImportError: LocalizedError {
        case unsupportedVersion(String)
        case invalidFormat

        var errorDescription: String? {
            switch self {
            case .unsupportedVersion(let v): return "Unsupported export version: \(v)"
            case .invalidFormat: return "Invalid export file format"
            }
        }
    }
}
