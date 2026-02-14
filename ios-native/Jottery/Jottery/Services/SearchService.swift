import Foundation

/// In-memory search over decrypted notes.
enum SearchService {

    /// Filter notes by a search query string.
    /// Supports basic text search, tag filtering with `#tagname`,
    /// and negation with `-term`.
    static func filter(notes: [DecryptedNote], query: String) -> [DecryptedNote] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return notes }

        let tokens = tokenise(trimmed)

        return notes.filter { note in
            matchesAllTokens(note: note, tokens: tokens)
        }
    }

    /// Sort notes by the given order, pinned notes always first.
    static func sort(notes: [DecryptedNote], order: SortOrder) -> [DecryptedNote] {
        notes.sorted { a, b in
            // Pinned first
            if a.pinned != b.pinned { return a.pinned }

            switch order {
            case .recent:
                return a.modifiedAt > b.modifiedAt
            case .oldest:
                return a.modifiedAt < b.modifiedAt
            case .alpha:
                return a.title.localizedCaseInsensitiveCompare(b.title) == .orderedAscending
            case .created:
                return a.createdAt > b.createdAt
            }
        }
    }

    // MARK: - Private

    private enum SearchToken {
        case tag(String)          // #tagname
        case exclude(String)      // -term
        case text(String)         // plain text
    }

    private static func tokenise(_ query: String) -> [SearchToken] {
        var tokens: [SearchToken] = []
        let parts = query.split(separator: " ", omittingEmptySubsequences: true)

        for part in parts {
            let str = String(part)
            if str.hasPrefix("#") && str.count > 1 {
                tokens.append(.tag(String(str.dropFirst()).lowercased()))
            } else if str.hasPrefix("-") && str.count > 1 {
                tokens.append(.exclude(String(str.dropFirst()).lowercased()))
            } else {
                tokens.append(.text(str.lowercased()))
            }
        }

        return tokens
    }

    private static func matchesAllTokens(note: DecryptedNote, tokens: [SearchToken]) -> Bool {
        let lowerContent = note.content.lowercased()
        let lowerTags = note.tags.map { $0.lowercased() }

        for token in tokens {
            switch token {
            case .tag(let tag):
                if !lowerTags.contains(tag) { return false }
            case .exclude(let term):
                if lowerContent.contains(term) { return false }
            case .text(let term):
                if !lowerContent.contains(term) && !lowerTags.contains(where: { $0.contains(term) }) {
                    return false
                }
            }
        }
        return true
    }
}
