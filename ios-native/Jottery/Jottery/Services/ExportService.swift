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
        /// Export writing is unaffected — `exportAll` always populates every
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

    /// Export all active notes as JSON matching the web format.
    static func exportAll(
        notes: [DecryptedNote],
        attachmentRepo: AttachmentRepository?,
        key: SymmetricKey
    ) throws -> Data {
        let exportNotes = notes.map { note in
            ExportNote(
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
        }

        let export = ExportData(
            version: "1.0",
            exportDate: Date().iso8601,
            notes: exportNotes
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return try encoder.encode(export)
    }

    /// Export a subset of notes by IDs.
    static func exportSelected(
        notes: [DecryptedNote],
        ids: Set<String>,
        attachmentRepo: AttachmentRepository?,
        key: SymmetricKey
    ) throws -> Data {
        let selected = notes.filter { ids.contains($0.id) }
        return try exportAll(notes: selected, attachmentRepo: attachmentRepo, key: key)
    }

    // MARK: - Private

    private static func exportAttachments(
        _ refs: [AttachmentRef],
        repo: AttachmentRepository?,
        key: SymmetricKey
    ) -> [ExportAttachment] {
        guard let repo else { return [] }
        return refs.compactMap { ref -> ExportAttachment? in
            guard let blob = try? repo.getBlob(id: ref.id) else { return nil }
            // Attempt to decrypt the blob
            if let encJSON = String(data: blob, encoding: .utf8),
               let enc = try? CryptoService.parseEncryptedJSON(encJSON),
               let decrypted = try? CryptoService.decrypt(enc, key: key) {
                return ExportAttachment(
                    filename: ref.filename,
                    mimeType: ref.mimeType,
                    data: decrypted.base64EncodedString()
                )
            }
            // Fallback: raw base64
            return ExportAttachment(
                filename: ref.filename,
                mimeType: ref.mimeType,
                data: blob.base64EncodedString()
            )
        }
    }

    // MARK: - Filename

    static func defaultFilename() -> String {
        let date = ISO8601DateFormatter.jottery.string(from: Date()).prefix(10)
        return "jottery-export-\(date).json"
    }
}
