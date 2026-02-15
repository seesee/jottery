import Foundation
import GRDB

/// On-disk note record with encrypted fields. Maps directly to the `notes` SQLite table.
/// Decryption happens in `NoteRepository`, not here.
struct NoteRecord: Codable, FetchableRecord, PersistableRecord, Identifiable {
    static let databaseTableName = "notes"

    // MARK: - Columns

    var id: String
    var createdAt: String          // ISO 8601
    var modifiedAt: String         // ISO 8601
    var syncedAt: String?
    var content: String            // Encrypted JSON: {"ciphertext":"...","iv":"..."}
    var tags: String               // Encrypted JSON: {"ciphertext":"...","iv":"..."}
    var attachments: String        // JSON array of attachment references
    var pinned: Bool
    var archived: Bool
    var archivedAt: String?
    var locked: Bool
    var lockedAt: String?
    var deleted: Bool
    var deletedAt: String?
    var version: Int
    var wordWrap: Bool
    var syntaxLanguage: String
    var showPreview: Bool
    var color: String?
    var contentHash: String?
    var parentHash: String?
    var hashChain: String?         // JSON array of hash strings
    var needsSync: Bool

    // MARK: - Column Mapping

    enum Columns: String, ColumnExpression {
        case id
        case createdAt = "created_at"
        case modifiedAt = "modified_at"
        case syncedAt = "synced_at"
        case content
        case tags
        case attachments
        case pinned
        case archived
        case archivedAt = "archived_at"
        case locked
        case lockedAt = "locked_at"
        case deleted
        case deletedAt = "deleted_at"
        case version
        case wordWrap = "word_wrap"
        case syntaxLanguage = "syntax_language"
        case showPreview = "show_preview"
        case color
        case contentHash = "content_hash"
        case parentHash = "parent_hash"
        case hashChain = "hash_chain"
        case needsSync = "needs_sync"
    }

    // MARK: - Coding Keys (snake_case ↔ camelCase)

    enum CodingKeys: String, CodingKey {
        case id
        case createdAt = "created_at"
        case modifiedAt = "modified_at"
        case syncedAt = "synced_at"
        case content
        case tags
        case attachments
        case pinned
        case archived
        case archivedAt = "archived_at"
        case locked
        case lockedAt = "locked_at"
        case deleted
        case deletedAt = "deleted_at"
        case version
        case wordWrap = "word_wrap"
        case syntaxLanguage = "syntax_language"
        case showPreview = "show_preview"
        case color
        case contentHash = "content_hash"
        case parentHash = "parent_hash"
        case hashChain = "hash_chain"
        case needsSync = "needs_sync"
    }

    // MARK: - Factory

    /// Create a new empty note record with encrypted empty content.
    static func new(encryptedContent: String, encryptedTags: String) -> NoteRecord {
        let now = ISO8601DateFormatter.jottery.string(from: Date())
        return NoteRecord(
            id: CryptoService.generateUUID(),
            createdAt: now,
            modifiedAt: now,
            syncedAt: nil,
            content: encryptedContent,
            tags: encryptedTags,
            attachments: "[]",
            pinned: false,
            archived: false,
            archivedAt: nil,
            locked: false,
            lockedAt: nil,
            deleted: false,
            deletedAt: nil,
            version: 0,
            wordWrap: true,
            syntaxLanguage: "markdown",
            showPreview: false,
            color: nil,
            contentHash: nil,
            parentHash: nil,
            hashChain: nil,
            needsSync: true
        )
    }
}
