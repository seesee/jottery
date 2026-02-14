import Foundation

/// In-memory decrypted note — never persisted.
/// Created by `NoteRepository` from a `NoteRecord`.
struct DecryptedNote: Identifiable, Equatable, Hashable {
    let id: String
    var createdAt: Date
    var modifiedAt: Date
    var syncedAt: Date?
    var content: String                // Decrypted plaintext
    var tags: [String]                 // Decrypted tag array
    var attachments: [AttachmentRef]
    var pinned: Bool
    var archived: Bool
    var archivedAt: Date?
    var locked: Bool
    var lockedAt: Date?
    var deleted: Bool
    var deletedAt: Date?
    var version: Int
    var wordWrap: Bool
    var syntaxLanguage: String
    var showPreview: Bool
    var color: String?
    var contentHash: String?
    var parentHash: String?
    var hashChain: [String]
    var needsSync: Bool
    var decryptedAt: Date              // When this was decrypted (cache management)

    /// Auto-generated title: first non-empty line, truncated.
    var title: String {
        let lines = content.split(separator: "\n", omittingEmptySubsequences: true)
        guard let firstLine = lines.first else { return "Untitled" }
        let trimmed = firstLine.trimmingCharacters(in: .whitespaces)
            .replacingOccurrences(of: "^#+\\s*", with: "", options: .regularExpression)
        if trimmed.isEmpty { return "Untitled" }
        return String(trimmed.prefix(80))
    }

    /// Preview snippet: second non-empty line (or continuation of first), truncated.
    var preview: String {
        let lines = content.split(separator: "\n", omittingEmptySubsequences: true)
        guard lines.count > 1 else { return "" }
        let line = lines[1].trimmingCharacters(in: .whitespaces)
        return String(line.prefix(120))
    }

    /// Word count for the note.
    var wordCount: Int {
        content.split(whereSeparator: { $0.isWhitespace || $0.isNewline }).count
    }

    // MARK: - Hashable / Equatable

    func hash(into hasher: inout Hasher) {
        hasher.combine(id)
    }

    static func == (lhs: DecryptedNote, rhs: DecryptedNote) -> Bool {
        lhs.id == rhs.id && lhs.modifiedAt == rhs.modifiedAt && lhs.version == rhs.version
    }
}
