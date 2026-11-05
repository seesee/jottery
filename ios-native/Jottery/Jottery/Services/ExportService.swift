import CryptoKit
import Foundation

/// Produces JSON matching the web client's export format.
enum ExportService {

    // MARK: - Export Models

    struct ExportData: Codable {
        let version: String
        let exportDate: String
        let notes: [ExportNote]
    }

    struct ExportNote: Codable {
        let id: String
        let createdAt: String
        let modifiedAt: String
        let content: String
        let tags: [String]
        let attachments: [ExportAttachment]
        let pinned: Bool
        let archived: Bool
        let locked: Bool
        let wordWrap: Bool?
        let syntaxLanguage: String?
        let showPreview: Bool?
        let color: String?

        init(
            id: String,
            createdAt: String,
            modifiedAt: String,
            content: String,
            tags: [String],
            attachments: [ExportAttachment],
            pinned: Bool,
            archived: Bool,
            locked: Bool,
            wordWrap: Bool?,
            syntaxLanguage: String?,
            showPreview: Bool?,
            color: String?
        ) {
            self.id = id
            self.createdAt = createdAt
            self.modifiedAt = modifiedAt
            self.content = content
            self.tags = tags
            self.attachments = attachments
            self.pinned = pinned
            self.archived = archived
            self.locked = locked
            self.wordWrap = wordWrap
            self.syntaxLanguage = syntaxLanguage
            self.showPreview = showPreview
            self.color = color
        }

        /// The documented cross-platform export format (CLAUDE.md) omits keys
        /// whose value is the default for a note — `archived`, `locked`,
        /// `pinned`, `color`, `syntaxLanguage`, `wordWrap`, `showPreview` and
        /// even `attachments` may all be absent (see demo-generation's
        /// jottery-demo-notes-*.json for real examples). A plain synthesized
        /// decode would throw `keyNotFound` on the first missing key and abort
        /// the entire import, so default every genuinely-optional field here.
        /// Export writing is unaffected — `writeExport` always populates every
        /// field via the memberwise initializer above.
        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            id = try container.decode(String.self, forKey: .id)
            createdAt = try container.decode(String.self, forKey: .createdAt)
            modifiedAt = try container.decode(String.self, forKey: .modifiedAt)
            content = try container.decode(String.self, forKey: .content)
            // Deliberately lenient — the documented format requires tags, but a missing key on one note must not abort the whole import.
            tags = try container.decodeIfPresent([String].self, forKey: .tags) ?? []
            attachments = try container.decodeIfPresent([ExportAttachment].self, forKey: .attachments) ?? []
            pinned = try container.decodeIfPresent(Bool.self, forKey: .pinned) ?? false
            archived = try container.decodeIfPresent(Bool.self, forKey: .archived) ?? false
            locked = try container.decodeIfPresent(Bool.self, forKey: .locked) ?? false
            wordWrap = try container.decodeIfPresent(Bool.self, forKey: .wordWrap)
            syntaxLanguage = try container.decodeIfPresent(String.self, forKey: .syntaxLanguage)
            showPreview = try container.decodeIfPresent(Bool.self, forKey: .showPreview)
            color = try container.decodeIfPresent(String.self, forKey: .color)
        }
    }

    struct ExportAttachment: Codable {
        let filename: String
        let mimeType: String
        let data: String  // Base64 encoded
    }

    // MARK: - Export

    /// Write `notes` to `url` as JSON matching the web format, one note at a
    /// time: with their attachments they can be far larger than memory. Run
    /// this off the main actor. Stops with `CancellationError` if the current
    /// task is cancelled.
    static func writeExport(
        notes: [DecryptedNote],
        to url: URL,
        attachmentRepo: AttachmentRepository?,
        key: SymmetricKey,
        progress: (@Sendable (_ completed: Int, _ total: Int) -> Void)? = nil
    ) throws {
        let handle = try FileHandle(forWritingTo: url)
        defer { try? handle.close() }
        try handle.truncate(atOffset: 0)

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

        try handle.write(contentsOf: Data("{\n\"version\": \"1.0\",\n\"exportDate\": \"\(Date().iso8601)\",\n\"notes\": [\n".utf8))
        for (index, note) in notes.enumerated() {
            try Task.checkCancellation()
            try autoreleasepool {
                let exportNote = ExportNote(
                    id: note.id,
                    createdAt: note.createdAt.iso8601,
                    modifiedAt: note.modifiedAt.iso8601,
                    content: note.content,
                    tags: note.tags,
                    attachments: exportAttachments(note.attachments, repo: attachmentRepo, key: key),
                    pinned: note.pinned,
                    archived: note.archived,
                    locked: note.locked,
                    wordWrap: note.wordWrap,
                    syntaxLanguage: note.syntaxLanguage,
                    showPreview: note.showPreview,
                    color: note.color
                )
                if index > 0 {
                    try handle.write(contentsOf: Data(",\n".utf8))
                }
                try handle.write(contentsOf: encoder.encode(exportNote))
            }
            progress?(index + 1, notes.count)
        }
        try handle.write(contentsOf: Data("\n]\n}\n".utf8))
    }

    /// `writeExport` into a new temporary file, for the share sheet. The caller
    /// removes the file when it's finished with it.
    static func exportToTemporaryFile(
        notes: [DecryptedNote],
        attachmentRepo: AttachmentRepository?,
        key: SymmetricKey,
        files: TemporaryFiles = .shared,
        progress: (@Sendable (_ completed: Int, _ total: Int) -> Void)? = nil
    ) async throws -> URL {
        let url = try files.createFile(named: defaultFilename())
        do {
            try await offMainActor {
                try writeExport(notes: notes, to: url, attachmentRepo: attachmentRepo, key: key, progress: progress)
            }
            return url
        } catch {
            files.remove(url)
            throw error
        }
    }

    // MARK: - Private

    /// An attachment that fails to decrypt is left out. Its ciphertext isn't
    /// the file, so it has no place in an export of decrypted data.
    private static func exportAttachments(
        _ refs: [AttachmentRef],
        repo: AttachmentRepository?,
        key: SymmetricKey
    ) -> [ExportAttachment] {
        guard let repo else { return [] }
        return refs.compactMap { ref -> ExportAttachment? in
            guard let blob = try? repo.getBlob(id: ref.id),
                  let encrypted = try? CryptoService.parseEncryptedJSON(String(decoding: blob, as: UTF8.self)),
                  let decrypted = try? CryptoService.decrypt(encrypted, key: key) else {
                Log.debug("[Export] Leaving out attachment \(ref.id): it couldn't be decrypted")
                return nil
            }
            return ExportAttachment(
                filename: ref.filename,
                mimeType: ref.mimeType,
                data: decrypted.base64EncodedString()
            )
        }
    }

    // MARK: - Filename

    static func defaultFilename() -> String {
        let date = ISO8601DateFormatter.jottery.string(from: Date()).prefix(10)
        return "jottery-export-\(date).json"
    }
}
