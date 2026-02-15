import Foundation
import GRDB

/// On-disk version snapshot. Maps directly to the `note_versions` SQLite table.
struct NoteVersionRecord: Codable, FetchableRecord, PersistableRecord, Identifiable {
    static let databaseTableName = "note_versions"

    // MARK: - Columns

    var versionKey: String         // "{noteId}:{version}" — PRIMARY KEY
    var noteId: String
    var version: Int
    var createdAt: String          // ISO 8601
    var syncedAt: String           // ISO 8601
    var content: String            // Encrypted JSON
    var tags: String               // Encrypted JSON (storage format — single blob)
    var attachments: String        // JSON array
    var syntaxLanguage: String?
    var wordWrap: Bool?
    var showPreview: Bool?
    var color: String?
    var reason: String             // "sync" | "manual-sync"
    var contentHash: String?
    var parentHash: String?

    var id: String { versionKey }

    // MARK: - Column Mapping

    enum Columns: String, ColumnExpression {
        case versionKey = "version_key"
        case noteId = "note_id"
        case version
        case createdAt = "created_at"
        case syncedAt = "synced_at"
        case content
        case tags
        case attachments
        case syntaxLanguage = "syntax_language"
        case wordWrap = "word_wrap"
        case showPreview = "show_preview"
        case color
        case reason
        case contentHash = "content_hash"
        case parentHash = "parent_hash"
    }

    // MARK: - Coding Keys

    enum CodingKeys: String, CodingKey {
        case versionKey = "version_key"
        case noteId = "note_id"
        case version
        case createdAt = "created_at"
        case syncedAt = "synced_at"
        case content
        case tags
        case attachments
        case syntaxLanguage = "syntax_language"
        case wordWrap = "word_wrap"
        case showPreview = "show_preview"
        case color
        case reason
        case contentHash = "content_hash"
        case parentHash = "parent_hash"
    }

    // MARK: - Factory

    /// Create a version snapshot from a NoteRecord.
    static func from(_ note: NoteRecord, reason: String) -> NoteVersionRecord {
        let now = ISO8601DateFormatter.jottery.string(from: Date())
        return NoteVersionRecord(
            versionKey: "\(note.id):\(note.version)",
            noteId: note.id,
            version: note.version,
            createdAt: note.modifiedAt,
            syncedAt: now,
            content: note.content,
            tags: note.tags,
            attachments: note.attachments,
            syntaxLanguage: note.syntaxLanguage,
            wordWrap: note.wordWrap,
            showPreview: note.showPreview,
            color: note.color,
            reason: reason,
            contentHash: note.contentHash,
            parentHash: note.parentHash
        )
    }
}
