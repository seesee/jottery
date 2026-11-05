import Foundation

/// Searches that have been used, newest first, while the vault is unlocked.
///
/// They say what someone was looking for, so they're as private as the notes
/// (jottery-ajpu). This lives only in memory, and goes with the key at lock.
struct RecentSearches: Equatable {
    static let limit = 10

    private(set) var queries: [String] = []

    mutating func record(_ query: String) {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count >= 2 else { return }
        queries.removeAll { $0.caseInsensitiveCompare(trimmed) == .orderedSame }
        queries.insert(trimmed, at: 0)
        queries = Array(queries.prefix(Self.limit))
    }

    mutating func clear() {
        queries = []
    }
}

/// What the search field offers beyond what's been typed.
enum SearchSuggestions {

    static let tagLimit = 8

    struct TagCompletion: Identifiable, Equatable {
        let tag: String
        /// The whole query, with the tag finished.
        let completion: String
        var id: String { tag }
    }

    struct Example: Identifiable, Equatable {
        /// Search syntax, which is the same in every language.
        let query: String
        let label: String
        var id: String { query }
    }

    /// How to search for a tag. `#` ends at a space and matches any tag holding the
    /// word, so a tag with a space in it is found by each of its words.
    static func query(forTag tag: String, marker: String = "#") -> String {
        tag.split(whereSeparator: \.isWhitespace).map { "\(marker)\($0)" }.joined(separator: " ")
    }

    /// Tags for a `#` being typed at the end of `query`. Suggestions take the place
    /// of the results while they're showing, so a finished tag offers nothing.
    static func tagCompletions(for query: String, tags: [String]) -> [TagCompletion] {
        // An odd quote means this is inside a phrase.
        guard query.filter({ $0 == "\"" }).count.isMultiple(of: 2) else { return [] }

        let start = query.lastIndex { $0.isWhitespace || $0 == "|" }.map(query.index(after:)) ?? query.startIndex
        let word = query[start...]
        let marker = word.hasPrefix("-#") ? "-#" : "#"
        guard word.hasPrefix(marker) else { return [] }

        let typed = word.dropFirst(marker.count).lowercased()
        let candidates = tags.filter { !DecryptedNote.isTitleTag($0) }
        guard !candidates.contains(where: { $0.lowercased() == typed }) else { return [] }

        let matching = candidates.filter { typed.isEmpty || $0.lowercased().contains(typed) }
        let startingWith = matching.filter { $0.lowercased().hasPrefix(typed) }
        let containing = matching.filter { !$0.lowercased().hasPrefix(typed) }
        return (startingWith + containing).prefix(tagLimit).map { tag in
            TagCompletion(tag: tag, completion: query[..<start] + Self.query(forTag: tag, marker: marker))
        }
    }

    /// One query for each thing the search can do, ready to run or to change.
    /// Those that can use something real do, so they find something when tapped.
    static func examples(firstTag: String?, today: Date = Date()) -> [Example] {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC") ?? .gmt
        let year = calendar.component(.year, from: today)
        return [
            Example(query: firstTag.map { query(forTag: $0) } ?? "#tag", label: L.searchExampleTag),
            Example(query: "\"exact phrase\"", label: L.searchExamplePhrase),
            Example(query: "word1 | word2", label: L.searchExampleEither),
            Example(query: "-word", label: L.searchExampleExclude),
            Example(query: "dog*", label: L.searchExampleStartsWith),
            Example(query: "has:attachment", label: L.searchExampleAttachment),
            Example(query: "created:>\(year)-01-01", label: L.searchExampleCreated),
            Example(query: "words:>100", label: L.searchExampleWords),
            Example(query: "colour:red", label: L.searchExampleColour),
            Example(query: "archived", label: L.searchExampleArchived),
        ]
    }
}
