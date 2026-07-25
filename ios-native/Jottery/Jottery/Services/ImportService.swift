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

    /// Import notes from parsed export data.
    static func importNotes(
        _ export: ExportService.ExportData,
        strategy: Strategy,
        noteRepo: NoteRepository,
        attachmentRepo: AttachmentRepository,
        key: SymmetricKey,
        progress: ((Int, Int) -> Void)? = nil
    ) throws -> ImportResult {
        var result = ImportResult()
        let total = export.notes.count

        for (index, exportNote) in export.notes.enumerated() {
            do {
                let existing = try noteRepo.getRaw(id: exportNote.id)

                if existing != nil {
                    switch strategy {
                    case .skip:
                        result.skipped += 1
                        progress?(index + 1, total)
                        continue
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
                        let encData = try CryptoService.encrypt(rawData, key: key)
                        let encJSON = try CryptoService.serializeEncryptedJSON(encData)
                        let blobData = Data(encJSON.utf8)
                        try attachmentRepo.storeBlob(
                            id: attId,
                            filename: exportAtt.filename,
                            mimeType: exportAtt.mimeType,
                            size: rawData.count,
                            data: blobData
                        )
                        let encFilename = try CryptoService.encryptText(exportAtt.filename, key: key)
                        let encFilenameJSON = try CryptoService.serializeEncryptedJSON(encFilename)
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

            progress?(index + 1, total)
        }

        return result
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
