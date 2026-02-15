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
